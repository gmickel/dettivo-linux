//! X11 helpers for the fallback driver: the window that belongs to a pid
//! and the keysym-to-keycode map XTEST typing needs.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, ConnectionExt as _};

use super::DriverError;
use super::atspi::AtspiDriver;

impl AtspiDriver {
    /// The part of `window` that is on the screen, in the window's own
    /// coordinates. `GetImage` refuses a rectangle that reaches past the
    /// screen (a `Match` error), and a window can be larger than a headless
    /// display or placed partly off it, so a screenshot takes what is
    /// visible instead of failing.
    pub(super) fn visible_rect(
        &mut self,
        window: xproto::Window,
    ) -> Result<(i16, i16, u16, u16), DriverError> {
        let (conn, screen) = self.x11()?;
        let root = &conn.setup().roots[*screen];
        let (screen_w, screen_h) = (
            i32::from(root.width_in_pixels),
            i32::from(root.height_in_pixels),
        );
        let root = root.root;
        let geometry = conn
            .get_geometry(window)
            .map_err(|e| DriverError::Failed(e.to_string()))?
            .reply()
            .map_err(|e| DriverError::Failed(format!("geometry: {e}")))?;
        let origin = conn
            .translate_coordinates(window, root, 0, 0)
            .map_err(|e| DriverError::Failed(e.to_string()))?
            .reply()
            .map_err(|e| DriverError::Failed(format!("translate_coordinates: {e}")))?;
        let (wx, wy) = (i32::from(origin.dst_x), i32::from(origin.dst_y));
        let (w, h) = (i32::from(geometry.width), i32::from(geometry.height));
        let x0 = (-wx).clamp(0, w);
        let y0 = (-wy).clamp(0, h);
        let x1 = (screen_w - wx).clamp(0, w);
        let y1 = (screen_h - wy).clamp(0, h);
        if x1 <= x0 || y1 <= y0 {
            return Err(DriverError::Failed(format!(
                "window {window} is off the {screen_w}x{screen_h} screen"
            )));
        }
        Ok((
            i16::try_from(x0).unwrap_or(i16::MAX),
            i16::try_from(y0).unwrap_or(i16::MAX),
            u16::try_from(x1 - x0).unwrap_or(u16::MAX),
            u16::try_from(y1 - y0).unwrap_or(u16::MAX),
        ))
    }

    /// The first top-level X11 window whose `_NET_WM_PID` is `pid`.
    pub(super) fn window_for_pid(&mut self, pid: u32) -> Result<Option<u64>, DriverError> {
        let (conn, screen) = self.x11()?;
        let root = conn.setup().roots[*screen].root;
        let atom = conn
            .intern_atom(false, b"_NET_WM_PID")
            .map_err(|e| DriverError::Failed(e.to_string()))?
            .reply()
            .map_err(|e| DriverError::Failed(format!("intern_atom: {e}")))?
            .atom;
        let tree = conn
            .query_tree(root)
            .map_err(|e| DriverError::Failed(e.to_string()))?
            .reply()
            .map_err(|e| DriverError::Failed(format!("query_tree: {e}")))?;
        let mut stack: Vec<xproto::Window> = tree.children;
        while let Some(window) = stack.pop() {
            if let Ok(reply) = conn
                .get_property(false, window, atom, xproto::AtomEnum::CARDINAL, 0, 1)
                .map_err(|e| DriverError::Failed(e.to_string()))?
                .reply()
            {
                if reply.value32().and_then(|mut v| v.next()) == Some(pid) {
                    if let Ok(attrs) = conn
                        .get_window_attributes(window)
                        .map_err(|e| DriverError::Failed(e.to_string()))?
                        .reply()
                    {
                        if attrs.map_state == xproto::MapState::VIEWABLE {
                            return Ok(Some(u64::from(window)));
                        }
                    }
                }
            }
            if let Ok(sub) = conn
                .query_tree(window)
                .map_err(|e| DriverError::Failed(e.to_string()))?
                .reply()
            {
                stack.extend(sub.children);
            }
        }
        Ok(None)
    }

    pub(super) fn keymap(&mut self) -> Result<Keymap, DriverError> {
        let (conn, _) = self.x11()?;
        let setup = conn.setup();
        let (min, max) = (setup.min_keycode, setup.max_keycode);
        let mapping = conn
            .get_keyboard_mapping(min, max - min + 1)
            .map_err(|e| DriverError::Failed(e.to_string()))?
            .reply()
            .map_err(|e| DriverError::Failed(format!("keyboard mapping: {e}")))?;
        let per = usize::from(mapping.keysyms_per_keycode);
        let mut entries = Vec::new();
        let mut shift = 50u8;
        for (i, syms) in mapping.keysyms.chunks(per).enumerate() {
            let keycode = min + i as u8;
            if syms.first() == Some(&0xFFE1) {
                shift = keycode;
            }
            if let Some(&plain) = syms.first() {
                entries.push((plain, keycode, false));
            }
            if let Some(&shifted) = syms.get(1) {
                entries.push((shifted, keycode, true));
            }
        }
        Ok(Keymap { entries, shift })
    }
}

/// Keysym to keycode lookup for printable ASCII.
pub(super) struct Keymap {
    pub(super) entries: Vec<(u32, u8, bool)>,
    pub(super) shift: u8,
}

impl Keymap {
    /// The keycode for a keysym and whether Shift is needed, the plain
    /// level first.
    pub(super) fn lookup_sym(&self, keysym: u32) -> Option<(u8, bool)> {
        self.entries
            .iter()
            .find(|(sym, _, shifted)| *sym == keysym && !*shifted)
            .or_else(|| self.entries.iter().find(|(sym, _, _)| *sym == keysym))
            .map(|(_, code, shifted)| (*code, *shifted))
    }

    pub(super) fn lookup(&self, ch: char) -> Option<(u8, bool)> {
        let keysym: u32 = match ch {
            '\n' => 0xFF0D,
            '\t' => 0xFF09,
            ' '..='~' => ch as u32,
            _ => return None,
        };
        self.entries
            .iter()
            .find(|(sym, _, shifted)| *sym == keysym && !*shifted)
            .or_else(|| self.entries.iter().find(|(sym, _, _)| *sym == keysym))
            .map(|(_, code, shifted)| (*code, *shifted))
    }
}
