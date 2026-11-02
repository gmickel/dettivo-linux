//! The X11 `CLIPBOARD` selection on x11rb. A snapshot reads `TARGETS`
//! and then each target, following `INCR` transfers, on a connection of
//! its own; each target is read through a requestor window of its own, so
//! a reply or chunk that arrives late can never land in another target's
//! answer. What Dettivo offers is served by one owner window per process,
//! so a restore can offer every target the snapshot held, with the type
//! and format each was answered in. A restore asks the server, under a
//! grab, whether the selection is still Dettivo's before taking it back.
//! One target larger than the server's request limit is not served (no
//! `INCR` on the owner side) and the requestor gets a refusal.

use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, Property, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::{CURRENT_TIME, NONE};

use super::{Clip, ClipError, ClipboardSnapshot, Generation, Restored, X11Form, admit, is_meta};

mod owner;

use owner::{Offer, owner, text_offers};

/// The X11 clipboard.
pub struct X11Clip;

fn err(what: &str) -> impl Fn(&dyn std::fmt::Display) -> ClipError + '_ {
    move |e| ClipError(format!("x11 clipboard {what}: {e}"))
}

/// One target's answer: the property's type, format and bytes.
struct Reply {
    type_: Atom,
    format: u8,
    value: Vec<u8>,
}

/// A connection that makes hidden windows for selection events.
struct Endpoint {
    conn: RustConnection,
    root: Window,
    clipboard: Atom,
    targets: Atom,
}

impl Endpoint {
    fn open() -> Result<Self, ClipError> {
        let (conn, screen) = x11rb::connect(None).map_err(|e| err("connect")(&e))?;
        let root = conn.setup().roots[screen].root;
        let clipboard = intern(&conn, "CLIPBOARD")?;
        let targets = intern(&conn, "TARGETS")?;
        Ok(Self {
            conn,
            root,
            clipboard,
            targets,
        })
    }

    fn window(&self) -> Result<Window, ClipError> {
        let window = self.conn.generate_id().map_err(|e| err("window")(&e))?;
        self.conn
            .create_window(
                0,
                window,
                self.root,
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
        Ok(window)
    }

    /// The selection converted to `target` within `limit` bytes, `None`
    /// when the owner (or no owner) refused it. The requestor window is
    /// destroyed afterwards: an owner still sending `INCR` chunks or a
    /// late reply then fails instead of feeding the next target.
    fn convert(&self, target: Atom, until: Instant, limit: usize) -> Result<Option<Reply>, String> {
        let window = self.window().map_err(|e| e.0)?;
        let answer = self.convert_on(window, target, until, limit);
        let _ = self.conn.destroy_window(window);
        let _ = self.conn.flush();
        answer
    }

    fn convert_on(
        &self,
        window: Window,
        target: Atom,
        until: Instant,
        limit: usize,
    ) -> Result<Option<Reply>, String> {
        let property = intern(&self.conn, "DETTIVO_CLIPBOARD").map_err(|e| e.0)?;
        let incr = intern(&self.conn, "INCR").map_err(|e| e.0)?;
        self.conn
            .convert_selection(window, self.clipboard, target, property, CURRENT_TIME)
            .map_err(|e| e.to_string())?;
        self.conn.flush().map_err(|e| e.to_string())?;
        let notify = loop {
            match self.next_event(until)? {
                Event::SelectionNotify(n)
                    if n.requestor == window
                        && n.selection == self.clipboard
                        && n.target == target =>
                {
                    break n;
                }
                _ => {}
            }
        };
        if notify.property == NONE {
            return Ok(None);
        }
        let first = self.take(window, property, limit)?;
        if first.type_ != incr {
            return Ok(Some(first));
        }
        // INCR: each new value of the property is the next chunk; an
        // empty one ends the transfer.
        let mut out = Reply {
            type_: target,
            format: 8,
            value: Vec::new(),
        };
        loop {
            match self.next_event(until)? {
                Event::PropertyNotify(p)
                    if p.window == window
                        && p.atom == property
                        && p.state == Property::NEW_VALUE =>
                {
                    let chunk = self.take(window, property, limit - out.value.len())?;
                    if chunk.value.is_empty() {
                        return Ok(Some(out));
                    }
                    if out.value.is_empty() {
                        out.type_ = chunk.type_;
                        out.format = chunk.format;
                    }
                    out.value.extend_from_slice(&chunk.value);
                }
                _ => {}
            }
        }
    }

    /// Reads and deletes `property` on `window`, asking for no more than
    /// `limit` bytes: a larger value is refused before it is transferred,
    /// and left in place so an `INCR` owner sends nothing more.
    fn take(&self, window: Window, property: Atom, limit: usize) -> Result<Reply, String> {
        let words = u32::try_from(limit.div_ceil(4)).unwrap_or(u32::MAX);
        let reply = self
            .conn
            .get_property(true, window, property, AtomEnum::ANY, 0, words)
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| e.to_string())?;
        if reply.bytes_after > 0 || reply.value.len() > limit {
            return Err("over the snapshot budget".into());
        }
        Ok(Reply {
            type_: reply.type_,
            format: reply.format,
            value: reply.value,
        })
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

    fn name(&self, atom: Atom) -> Option<String> {
        let reply = self.conn.get_atom_name(atom).ok()?.reply().ok()?;
        Some(String::from_utf8_lossy(&reply.name).into_owned())
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

impl Clip for X11Clip {
    fn snapshot(
        &self,
        budget: usize,
        deadline: Duration,
    ) -> Result<Option<ClipboardSnapshot>, ClipError> {
        let until = Instant::now() + deadline;
        let reader = Endpoint::open()?;
        let targets = match reader.convert(reader.targets, until, budget) {
            Ok(Some(reply)) => reply.value,
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
            let Some(name) = reader.name(atom) else {
                continue;
            };
            if is_meta(&name) {
                continue;
            }
            offered = true;
            match reader.convert(atom, until, left) {
                Ok(Some(reply)) => {
                    let form = (reply.format != 0)
                        .then(|| reader.name(reply.type_))
                        .flatten()
                        .map(|type_name| X11Form {
                            type_name,
                            format: reply.format,
                        });
                    if admit(&mut snap, &mut left, name.clone(), reply.value)
                        && let Some(form) = form
                    {
                        snap.forms.insert(name, form);
                    }
                }
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
        let offers = text_offers(owner.conn(), text)?;
        owner.set(offers).map(Generation)
    }

    fn restore(
        &self,
        snap: ClipboardSnapshot,
        if_generation: Generation,
    ) -> Result<Restored, ClipError> {
        let owner = owner()?;
        let conn = owner.conn();
        let mut offers = Vec::new();
        for (name, bytes) in snap.offers {
            let (type_name, format) = snap
                .forms
                .get(&name)
                .map_or((name.as_str(), 8), |f| (f.type_name.as_str(), f.format));
            if !matches!(format, 8 | 16 | 32) || bytes.len() % usize::from(format / 8) != 0 {
                tracing::warn!(mime = %name, format, "clipboard type not restored: malformed");
                continue;
            }
            offers.push(Offer {
                target: intern(conn, &name)?,
                type_: intern(conn, type_name)?,
                format,
                bytes,
            });
        }
        owner.restore(offers, if_generation.0)
    }
}
