//! The X11 `CLIPBOARD` selection on x11rb. A snapshot reads `TARGETS`
//! and then each target, following `INCR` transfers, on a connection of
//! its own. What Dettivo offers is served by one owner window per
//! process, so a restore can offer every target the snapshot held; a
//! `SelectionClear` tells it another client took the selection. One
//! target larger than the server's request limit is not served (no `INCR`
//! on the owner side) and the requestor gets a refusal.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, Property,
    SELECTION_NOTIFY_EVENT, SelectionNotifyEvent, SelectionRequestEvent, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{CURRENT_TIME, NONE};

use super::{Clip, ClipError, ClipboardSnapshot, Generation, Restored, TEXT_TYPES, admit, is_meta};

/// The X11 clipboard.
pub struct X11Clip;

fn err(what: &str) -> impl Fn(&dyn std::fmt::Display) -> ClipError + '_ {
    move |e| ClipError(format!("x11 clipboard {what}: {e}"))
}

/// A connection with a hidden window that receives selection events.
struct Endpoint {
    conn: RustConnection,
    window: Window,
    clipboard: Atom,
    targets: Atom,
}

impl Endpoint {
    fn open() -> Result<Self, ClipError> {
        let (conn, screen) = x11rb::connect(None).map_err(|e| err("connect")(&e))?;
        let root = conn.setup().roots[screen].root;
        let window = conn.generate_id().map_err(|e| err("window")(&e))?;
        conn.create_window(
            0,
            window,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_ONLY,
            0,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .map_err(|e| err("window")(&e))?;
        let clipboard = intern(&conn, "CLIPBOARD")?;
        let targets = intern(&conn, "TARGETS")?;
        Ok(Self {
            conn,
            window,
            clipboard,
            targets,
        })
    }

    /// The selection converted to `target`, `None` when the owner (or no
    /// owner) refused it.
    fn convert(
        &self,
        target: Atom,
        until: Instant,
        limit: usize,
    ) -> Result<Option<Vec<u8>>, String> {
        let property = intern(&self.conn, "DETTIVO_CLIPBOARD").map_err(|e| e.0)?;
        let incr = intern(&self.conn, "INCR").map_err(|e| e.0)?;
        self.conn
            .convert_selection(self.window, self.clipboard, target, property, CURRENT_TIME)
            .map_err(|e| e.to_string())?;
        self.conn.flush().map_err(|e| e.to_string())?;
        let notify = loop {
            match self.next_event(until)? {
                Event::SelectionNotify(n) if n.requestor == self.window => break n,
                _ => {}
            }
        };
        if notify.property == NONE {
            return Ok(None);
        }
        let first = self.take(property)?;
        if first.type_ != incr {
            return Ok(Some(first.value));
        }
        // INCR: each new value of the property is the next chunk; an
        // empty one ends the transfer.
        let mut out = Vec::new();
        loop {
            match self.next_event(until)? {
                Event::PropertyNotify(p)
                    if p.atom == property && p.state == Property::NEW_VALUE =>
                {
                    let chunk = self.take(property)?.value;
                    if chunk.is_empty() {
                        return Ok(Some(out));
                    }
                    if out.len() + chunk.len() > limit {
                        return Err("over the snapshot budget".into());
                    }
                    out.extend_from_slice(&chunk);
                }
                _ => {}
            }
        }
    }

    /// Reads and deletes `property` on the window.
    fn take(&self, property: Atom) -> Result<x11rb::protocol::xproto::GetPropertyReply, String> {
        self.conn
            .get_property(true, self.window, property, AtomEnum::ANY, 0, u32::MAX / 4)
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| e.to_string())
    }

    fn next_event(&self, until: Instant) -> Result<Event, String> {
        loop {
            if let Some(event) = self.conn.poll_for_event().map_err(|e| e.to_string())? {
                return Ok(event);
            }
            if Instant::now() >= until {
                return Err("the owner did not answer in time".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn intern(conn: &RustConnection, name: &str) -> Result<Atom, ClipError> {
    Ok(conn
        .intern_atom(false, name.as_bytes())
        .map_err(|e| err("atom")(&e))?
        .reply()
        .map_err(|e| err("atom")(&e))?
        .atom)
}

/// What the owner window serves.
#[derive(Default)]
struct Served {
    generation: u64,
    /// The selection is still ours.
    live: bool,
    offers: Vec<(Atom, Vec<u8>)>,
}

/// The process's owner window and the thread answering for it.
struct Owner {
    endpoint: Endpoint,
    served: Mutex<Served>,
}

static OWNER: OnceLock<Result<Arc<Owner>, ClipError>> = OnceLock::new();

fn owner() -> Result<Arc<Owner>, ClipError> {
    OWNER
        .get_or_init(|| {
            let owner = Arc::new(Owner {
                endpoint: Endpoint::open()?,
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
            match event {
                Event::SelectionRequest(request) => {
                    if let Err(e) = self.answer(&request) {
                        tracing::warn!(error = %e, "x11 clipboard: request not answered");
                    }
                }
                Event::SelectionClear(clear) if clear.selection == self.endpoint.clipboard => {
                    self.served.lock().unwrap_or_else(|p| p.into_inner()).live = false;
                }
                _ => {}
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
            atoms.extend(served.offers.iter().map(|(a, _)| *a));
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
            match served.offers.iter().find(|(a, _)| *a == request.target) {
                Some((target, bytes)) if bytes.len() <= fits => {
                    conn.change_property8(
                        PropMode::REPLACE,
                        request.requestor,
                        property,
                        *target,
                        bytes,
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
    fn offer(&self, served: &mut Served, offers: Vec<(Atom, Vec<u8>)>) -> Result<(), ClipError> {
        let e = &self.endpoint;
        let window = if offers.is_empty() { NONE } else { e.window };
        served.offers = offers;
        served.generation += 1;
        served.live = window != NONE;
        e.conn
            .set_selection_owner(window, e.clipboard, CURRENT_TIME)
            .map_err(|x| err("owner")(&x))?;
        let now = e
            .conn
            .get_selection_owner(e.clipboard)
            .map_err(|x| err("owner")(&x))?
            .reply()
            .map_err(|x| err("owner")(&x))?
            .owner;
        if now != window {
            served.live = false;
            return Err(ClipError(
                "x11 clipboard: the selection was not taken".into(),
            ));
        }
        Ok(())
    }
}

impl Clip for X11Clip {
    fn snapshot(
        &self,
        budget: usize,
        deadline: Duration,
    ) -> Result<Option<ClipboardSnapshot>, ClipError> {
        let until = Instant::now() + deadline;
        let reader = Endpoint::open()?;
        let targets = match reader.convert(reader.targets, until, budget) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return Ok(None),
            Err(e) => return Err(ClipError(format!("x11 clipboard targets: {e}"))),
        };
        let mut snap = ClipboardSnapshot::default();
        let mut left = budget;
        let mut offered = false;
        for atom in targets
            .chunks_exact(4)
            .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
        {
            let name = match reader.conn.get_atom_name(atom).map(|c| c.reply()) {
                Ok(Ok(reply)) => String::from_utf8_lossy(&reply.name).into_owned(),
                _ => continue,
            };
            if is_meta(&name) {
                continue;
            }
            offered = true;
            match reader.convert(atom, until, left) {
                Ok(Some(bytes)) => admit(&mut snap, &mut left, name, bytes),
                Ok(None) => {}
                Err(reason) => {
                    tracing::warn!(mime = %name, reason = %reason, "clipboard type left out of the snapshot")
                }
            }
        }
        match (offered, snap.offers.is_empty()) {
            (false, _) => Ok(None),
            (true, true) => Err(ClipError("no clipboard type could be read".into())),
            (true, false) => Ok(Some(snap)),
        }
    }

    fn set_text(&self, text: &str) -> Result<Generation, ClipError> {
        let owner = owner()?;
        let mut offers = Vec::new();
        for name in TEXT_TYPES {
            offers.push((
                intern(&owner.endpoint.conn, name)?,
                text.as_bytes().to_vec(),
            ));
        }
        let mut served = owner.served.lock().unwrap_or_else(|p| p.into_inner());
        owner.offer(&mut served, offers)?;
        Ok(Generation(served.generation))
    }

    fn restore(
        &self,
        snap: ClipboardSnapshot,
        if_generation: Generation,
    ) -> Result<Restored, ClipError> {
        let owner = owner()?;
        let mut offers = Vec::new();
        for (name, bytes) in snap.offers {
            offers.push((intern(&owner.endpoint.conn, &name)?, bytes));
        }
        let mut served = owner.served.lock().unwrap_or_else(|p| p.into_inner());
        if served.generation != if_generation.0 || !served.live {
            return Ok(Restored::Superseded);
        }
        owner.offer(&mut served, offers)?;
        Ok(Restored::Restored)
    }
}
