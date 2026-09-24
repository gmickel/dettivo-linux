//! The X11 probe: `_NET_ACTIVE_WINDOW` on the root, then `_NET_WM_PID`,
//! `WM_CLASS` and the title of that window. Serves XWayland under any
//! Wayland compositor without a focus protocol, and plain X sessions
//! (the CI drive under Xvfb with openbox).

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, AtomEnum, ConnectionExt as _};
use x11rb::rust_connection::RustConnection;

use super::{FocusProbe, RawWindow};

/// The probe.
pub struct X11Probe {
    /// `DISPLAY`.
    pub display: String,
    /// True when the X server is XWayland (a Wayland display exists too).
    pub xwayland: bool,
}

fn atom(conn: &RustConnection, name: &str) -> Result<xproto::Atom, String> {
    conn.intern_atom(false, name.as_bytes())
        .map_err(|e| e.to_string())?
        .reply()
        .map(|r| r.atom)
        .map_err(|e| format!("intern {name}: {e}"))
}

fn property(
    conn: &RustConnection,
    window: xproto::Window,
    name: &str,
    kind: impl Into<xproto::Atom>,
) -> Result<Option<Vec<u8>>, String> {
    let prop = atom(conn, name)?;
    let reply = conn
        .get_property(false, window, prop, kind, 0, 4096)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| format!("get {name}: {e}"))?;
    Ok((reply.value_len > 0).then_some(reply.value))
}

/// Splits `WM_CLASS` (`instance\0class\0`) into the app id the chain
/// uses: the class, falling back to the instance.
pub fn app_id_from_wm_class(value: &[u8]) -> String {
    let mut parts = value
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned());
    let instance = parts.next().unwrap_or_default();
    parts.next().unwrap_or(instance)
}

impl FocusProbe for X11Probe {
    fn name(&self) -> &'static str {
        "x11"
    }

    fn focused(&self) -> Result<Option<RawWindow>, String> {
        let (conn, screen) = RustConnection::connect(Some(&self.display))
            .map_err(|e| format!("x11 {}: {e}", self.display))?;
        let root = conn.setup().roots[screen].root;
        let active = property(&conn, root, "_NET_ACTIVE_WINDOW", AtomEnum::WINDOW)?
            .and_then(|v| {
                v.get(..4)
                    .map(|b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
            })
            .filter(|w| *w != 0);
        // Without a window manager nothing sets _NET_ACTIVE_WINDOW; the
        // server's input focus, climbed to the top-level that carries the
        // client properties, names the window a drive focused directly.
        let window = match active {
            Some(w) => w,
            None => match input_focus_toplevel(&conn, root)? {
                Some(w) => w,
                None => return Ok(None),
            },
        };
        let pid = property(&conn, window, "_NET_WM_PID", AtomEnum::CARDINAL)?
            .and_then(|v| {
                v.get(..4)
                    .map(|b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
            })
            .filter(|p| *p != 0);
        let app_id = property(&conn, window, "WM_CLASS", AtomEnum::STRING)?
            .map(|v| app_id_from_wm_class(&v))
            .unwrap_or_default();
        let utf8 = atom(&conn, "UTF8_STRING")?;
        let title = property(&conn, window, "_NET_WM_NAME", utf8)?
            .or(property(&conn, window, "WM_NAME", AtomEnum::STRING)?)
            .map(|v| String::from_utf8_lossy(&v).into_owned())
            .unwrap_or_default();
        Ok(Some(RawWindow {
            app_id,
            pid,
            title,
            xwayland: self.xwayland,
            window: Some(format!("{window:#x}")),
        }))
    }
}

/// The window holding the input focus, climbed to the nearest ancestor
/// that carries `_NET_WM_PID` or `WM_CLASS`; `None` when the focus is
/// the root, `PointerRoot` or unset.
fn input_focus_toplevel(conn: &RustConnection, root: u32) -> Result<Option<u32>, String> {
    let focus = conn
        .get_input_focus()
        .map_err(|e| format!("x11 focus: {e}"))?
        .reply()
        .map_err(|e| format!("x11 focus: {e}"))?
        .focus;
    // 0 is None, 1 is PointerRoot.
    let mut window = focus;
    if window <= 1 || window == root {
        return Ok(None);
    }
    for _ in 0..32 {
        let named = property(conn, window, "_NET_WM_PID", AtomEnum::CARDINAL)?.is_some()
            || property(conn, window, "WM_CLASS", AtomEnum::STRING)?.is_some();
        if named {
            return Ok(Some(window));
        }
        let tree = conn
            .query_tree(window)
            .map_err(|e| format!("x11 tree: {e}"))?
            .reply()
            .map_err(|e| format!("x11 tree: {e}"))?;
        if tree.parent == root || tree.parent == 0 {
            return Ok(None);
        }
        window = tree.parent;
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wm_class_prefers_the_class_over_the_instance() {
        assert_eq!(
            app_id_from_wm_class(b"dettivo-insert-target\0dettivo-insert-target\0"),
            "dettivo-insert-target"
        );
        assert_eq!(app_id_from_wm_class(b"foot\0"), "foot");
        assert_eq!(app_id_from_wm_class(b""), "");
    }
}
