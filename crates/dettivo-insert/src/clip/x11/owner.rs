//! The owner side: one window per process that serves what Dettivo
//! offers, and the restore that takes the selection back only while it
//! is still Dettivo's.

use std::sync::{Arc, Mutex, OnceLock};

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt as _, EventMask, PropMode, SELECTION_NOTIFY_EVENT,
    SelectionNotifyEvent, SelectionRequestEvent, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{CURRENT_TIME, NONE};

use super::{Endpoint, err, intern};
use crate::clip::{ClipError, Restored};
/// One target the owner window serves, as the property it writes.
pub(super) struct Offer {
    pub(super) target: Atom,
    pub(super) type_: Atom,
    pub(super) format: u8,
    pub(super) bytes: Vec<u8>,
}

/// What the owner window serves.
#[derive(Default)]
struct Served {
    generation: u64,
    offers: Vec<Offer>,
}

/// The process's owner window and the thread answering for it.
pub(super) struct Owner {
    endpoint: Endpoint,
    window: Window,
    served: Mutex<Served>,
}

static OWNER: OnceLock<Result<Arc<Owner>, ClipError>> = OnceLock::new();

pub(super) fn owner() -> Result<Arc<Owner>, ClipError> {
    OWNER
        .get_or_init(|| {
            let endpoint = Endpoint::open()?;
            let owner = Arc::new(Owner {
                window: endpoint.window()?,
                endpoint,
                served: Mutex::new(Served::default()),
            });
            let serving = owner.clone();
            std::thread::Builder::new()
                .name("x11-clipboard".into())
                .spawn(move || serving.run())
                .map_err(|e| err("thread")(&e))?;
            Ok(owner)
        })
        .clone()
}

impl Owner {
    fn run(&self) {
        while let Ok(event) = self.endpoint.conn.wait_for_event() {
            if let Event::SelectionRequest(request) = event
                && let Err(e) = self.answer(&request)
            {
                tracing::warn!(error = %e, "x11 clipboard: request not answered");
            }
        }
    }

    fn answer(&self, request: &SelectionRequestEvent) -> Result<(), String> {
        let conn = &self.endpoint.conn;
        let served = self.served.lock().unwrap_or_else(|p| p.into_inner());
        let property = if request.property == NONE {
            request.target
        } else {
            request.property
        };
        let fits = conn.maximum_request_bytes().saturating_sub(64);
        let answered = if request.target == self.endpoint.targets {
            let mut atoms = vec![self.endpoint.targets];
            atoms.extend(served.offers.iter().map(|o| o.target));
            conn.change_property32(
                PropMode::REPLACE,
                request.requestor,
                property,
                AtomEnum::ATOM,
                &atoms,
            )
            .map_err(|e| e.to_string())?;
            property
        } else {
            match served.offers.iter().find(|o| o.target == request.target) {
                Some(o) if o.bytes.len() <= fits => {
                    let units = o.bytes.len() / usize::from(o.format / 8);
                    conn.change_property(
                        PropMode::REPLACE,
                        request.requestor,
                        property,
                        o.type_,
                        o.format,
                        u32::try_from(units).map_err(|e| e.to_string())?,
                        &o.bytes,
                    )
                    .map_err(|e| e.to_string())?;
                    property
                }
                _ => NONE,
            }
        };
        let notify = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: request.time,
            requestor: request.requestor,
            selection: request.selection,
            target: request.target,
            property: answered,
        };
        conn.send_event(false, request.requestor, EventMask::NO_EVENT, notify)
            .map_err(|e| e.to_string())?;
        conn.flush().map_err(|e| e.to_string())
    }

    /// Takes the selection with `offers` (an empty list gives it up).
    fn offer(&self, served: &mut Served, offers: Vec<Offer>) -> Result<(), ClipError> {
        let e = &self.endpoint;
        let window = if offers.is_empty() { NONE } else { self.window };
        served.offers = offers;
        served.generation += 1;
        e.conn
            .set_selection_owner(window, e.clipboard, CURRENT_TIME)
            .map_err(|x| err("owner")(&x))?;
        if self.current()? != window {
            return Err(ClipError(
                "x11 clipboard: the selection was not taken".into(),
            ));
        }
        Ok(())
    }

    /// Who owns the selection now, by the server's answer.
    fn current(&self) -> Result<Window, ClipError> {
        let e = &self.endpoint;
        Ok(e.conn
            .get_selection_owner(e.clipboard)
            .map_err(|x| err("owner")(&x))?
            .reply()
            .map_err(|x| err("owner")(&x))?
            .owner)
    }

    pub(super) fn conn(&self) -> &RustConnection {
        &self.endpoint.conn
    }

    /// Takes the selection with `offers`; the generation they are.
    pub(super) fn set(&self, offers: Vec<Offer>) -> Result<u64, ClipError> {
        let mut served = self.served.lock().unwrap_or_else(|p| p.into_inner());
        self.offer(&mut served, offers)?;
        Ok(served.generation)
    }

    /// Offers `offers` if the selection still holds `generation`: nothing
    /// newer was set here, and the selection is still this window's. The
    /// ownership check and the take run under a server grab, so no other
    /// client can copy in between, and they ask the server rather than
    /// wait for a `SelectionClear` that may still be on its way.
    pub(super) fn restore(
        &self,
        offers: Vec<Offer>,
        generation: u64,
    ) -> Result<Restored, ClipError> {
        let mut served = self.served.lock().unwrap_or_else(|p| p.into_inner());
        if served.generation != generation {
            return Ok(Restored::Superseded);
        }
        let served = &mut *served;
        let conn = &self.endpoint.conn;
        conn.grab_server().map_err(|x| err("grab")(&x))?;
        let taken = match self.current() {
            Ok(now) if now == self.window => {
                self.offer(served, offers).map(|()| Restored::Restored)
            }
            Ok(_) => Ok(Restored::Superseded),
            Err(e) => Err(e),
        };
        let released = conn.ungrab_server().map_err(|x| err("grab")(&x));
        let flushed = conn.flush().map_err(|x| err("grab")(&x));
        let restored = taken?;
        released?;
        flushed?;
        Ok(restored)
    }
}

/// The targets a dictation is offered as. `TEXT` answers as
/// `UTF8_STRING`; `STRING` is ISO-8859-1, so it is offered only when the
/// text has a Latin-1 form, in that form.
pub(super) fn text_offers(conn: &RustConnection, text: &str) -> Result<Vec<Offer>, ClipError> {
    let utf8 = intern(conn, "UTF8_STRING")?;
    let mut offers = Vec::new();
    for (name, type_) in [
        ("text/plain;charset=utf-8", None),
        ("UTF8_STRING", Some(utf8)),
        ("text/plain", None),
        ("TEXT", Some(utf8)),
    ] {
        let target = intern(conn, name)?;
        offers.push(Offer {
            target,
            type_: type_.unwrap_or(target),
            format: 8,
            bytes: text.as_bytes().to_vec(),
        });
    }
    if let Some(latin1) = latin1(text) {
        let string = intern(conn, "STRING")?;
        offers.push(Offer {
            target: string,
            type_: string,
            format: 8,
            bytes: latin1,
        });
    }
    Ok(offers)
}

/// `text` in ISO-8859-1, `None` when a character has no Latin-1 form.
fn latin1(text: &str) -> Option<Vec<u8>> {
    text.chars()
        .map(|c| u8::try_from(u32::from(c)).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::latin1;

    #[test]
    fn string_is_offered_in_latin1_only_when_the_text_has_that_form() {
        assert_eq!(latin1("café"), Some(b"caf\xe9".to_vec()));
        assert_eq!(latin1("naïve – done"), None);
    }
}
