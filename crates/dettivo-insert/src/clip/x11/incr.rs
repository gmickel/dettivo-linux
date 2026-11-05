//! The owner side of ICCCM `INCR`: a restored target too large for one
//! `ChangeProperty` goes to the requestor in chunks, one each time the
//! requestor deletes the property. Each transfer holds its own handle on
//! the bytes, so a newer offer never changes one in flight, and one whose
//! requestor stops reading is dropped after a few seconds.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::ErrorKind;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, ChangeWindowAttributesAux, ConnectionExt as _, EventMask, PropMode, Property, Window,
};
use x11rb::rust_connection::RustConnection;

/// The bytes sent per chunk: within the core request limit, so it fits
/// whether or not the server has BIG-REQUESTS, and a multiple of 4 so a
/// chunk never splits a 16- or 32-bit unit.
const CHUNK: usize = 64 * 1024;

/// How long a requestor may take to delete the property before the
/// transfer is dropped.
const STALL: Duration = Duration::from_secs(5);

struct Transfer {
    type_: Atom,
    format: u8,
    bytes: Arc<[u8]>,
    sent: usize,
    last: Instant,
}

/// The `INCR` transfers in flight, by requestor window and property.
#[derive(Default)]
pub(super) struct Transfers(HashMap<(Window, Atom), Transfer>);

impl Transfers {
    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Starts a transfer: watches the requestor's properties and writes
    /// the `INCR` property. The caller sends the `SelectionNotify`.
    pub(super) fn start(
        &mut self,
        conn: &RustConnection,
        incr: Atom,
        requestor: Window,
        property: Atom,
        (type_, format, bytes): (Atom, u8, Arc<[u8]>),
    ) -> Result<(), String> {
        watch(conn, requestor, EventMask::PROPERTY_CHANGE)?;
        let size = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        conn.change_property(
            PropMode::REPLACE,
            requestor,
            property,
            incr,
            32,
            1,
            &size.to_ne_bytes(),
        )
        .map_err(|e| e.to_string())?;
        self.0.insert(
            (requestor, property),
            Transfer {
                type_,
                format,
                bytes,
                sent: 0,
                last: Instant::now(),
            },
        );
        Ok(())
    }

    /// Advances the transfer `event` belongs to, if any: the next chunk
    /// on a deleted property, or the end of every transfer to a window
    /// that no longer exists.
    pub(super) fn handle(&mut self, conn: &RustConnection, event: &Event) -> Result<(), String> {
        match event {
            Event::PropertyNotify(p) if p.state == Property::DELETE => {
                let key = (p.window, p.atom);
                let Some(t) = self.0.get_mut(&key) else {
                    return Ok(());
                };
                let end = (t.sent + CHUNK).min(t.bytes.len());
                let chunk = &t.bytes[t.sent..end];
                let units = chunk.len() / usize::from(t.format / 8);
                conn.change_property(
                    PropMode::REPLACE,
                    p.window,
                    p.atom,
                    t.type_,
                    t.format,
                    u32::try_from(units).map_err(|e| e.to_string())?,
                    chunk,
                )
                .map_err(|e| e.to_string())?;
                // The empty chunk ends the transfer.
                let done = chunk.is_empty();
                t.sent = end;
                t.last = Instant::now();
                if done {
                    self.drop_transfer(conn, key);
                }
                conn.flush().map_err(|e| e.to_string())
            }
            Event::Error(e) if e.error_kind == ErrorKind::Window => {
                self.0.retain(|(w, _), _| *w != e.bad_value);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Drops every transfer whose requestor has not deleted the property
    /// within [`STALL`].
    pub(super) fn expire(&mut self, conn: &RustConnection) {
        let stalled: Vec<_> = self
            .0
            .iter()
            .filter(|(_, t)| t.last.elapsed() > STALL)
            .map(|(k, _)| *k)
            .collect();
        for key in stalled {
            tracing::warn!("x11 clipboard: an INCR requestor stopped reading");
            self.drop_transfer(conn, key);
        }
        let _ = conn.flush();
    }

    /// Forgets a transfer, and stops watching the requestor once no other
    /// transfer goes to it.
    fn drop_transfer(&mut self, conn: &RustConnection, key: (Window, Atom)) {
        self.0.remove(&key);
        if !self.0.keys().any(|(w, _)| *w == key.0) {
            let _ = watch(conn, key.0, EventMask::NO_EVENT);
        }
    }
}

fn watch(conn: &RustConnection, window: Window, mask: EventMask) -> Result<(), String> {
    conn.change_window_attributes(window, &ChangeWindowAttributesAux::new().event_mask(mask))
        .map(drop)
        .map_err(|e| e.to_string())
}
