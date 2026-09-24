//! Focus probes: who owns the focused window right now. One per
//! environment (Hyprland IPC, X11 EWMH under XWayland or a plain X
//! session, the QA mock); the chain compares the answer with the guards
//! a client captured at hotkey time before any backend runs.

pub mod hyprland;
pub mod x11;

use dettivo_proto::methods::insert::Target;

use crate::session::Session;
use crate::settings::Settings;

/// The focused window as a probe reports it, before policy is applied.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawWindow {
    /// The Wayland app id or X11 window class.
    pub app_id: String,
    /// The owning process id when known.
    pub pid: Option<u32>,
    /// The window title; hashed before it leaves the daemon.
    pub title: String,
    /// True for an X client under XWayland.
    pub xwayland: bool,
    /// The compositor's identity of the window (a Hyprland address, an
    /// X11 window id), stable for the window's life and different for
    /// two windows of one process; never leaves the daemon.
    pub window: Option<String>,
}

/// A way to find the focused window.
pub trait FocusProbe {
    /// The probe's name for `insert.target` (`hyprland`, `x11`, `mock`).
    fn name(&self) -> &'static str;
    /// The focused window, `None` when nothing is focused.
    fn focused(&self) -> Result<Option<RawWindow>, String>;
}

/// The QA mock probe: a fixed GNOME Text Editor window, pid 4141.
pub struct MockProbe;

/// The app id the mock probe reports.
pub const MOCK_APP_ID: &str = "org.gnome.TextEditor";
/// The pid the mock probe reports.
pub const MOCK_PID: u32 = 4141;

impl FocusProbe for MockProbe {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn focused(&self) -> Result<Option<RawWindow>, String> {
        Ok(Some(RawWindow {
            app_id: MOCK_APP_ID.to_string(),
            pid: Some(MOCK_PID),
            title: "mock".to_string(),
            xwayland: false,
            window: Some("mock-window".to_string()),
        }))
    }
}

/// The probe for this session: the mock when QA asks for it, Hyprland
/// IPC on Hyprland, X11 when a display is reachable, none otherwise.
pub fn for_session(session: &Session) -> Option<Box<dyn FocusProbe>> {
    if session.mock_focus || session.mock.is_some() {
        return Some(Box::new(MockProbe));
    }
    if let Some(socket) = session.hyprland_socket() {
        return Some(Box::new(hyprland::HyprlandProbe { socket }));
    }
    if let Some(display) = &session.x11_display {
        return Some(Box::new(x11::X11Probe {
            display: display.clone(),
            xwayland: session.is_wayland(),
        }));
    }
    None
}

/// FNV-1a over the title: stable, short, never reversible into the title
/// by a reader of the result.
pub fn title_hash(title: &str) -> String {
    if title == "mock" {
        return "mock".to_string();
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in title.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Applies the settings to a raw window: the self check and the hash.
pub fn target_from(raw: &RawWindow, settings: &Settings) -> Target {
    Target {
        app_id: raw.app_id.clone(),
        pid: raw.pid,
        title_hash: title_hash(&raw.title),
        is_dettivo: settings.is_self(&raw.app_id),
        xwayland: raw.xwayland,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_carry_a_hash_not_the_title_and_flag_dettivo_windows() {
        let settings = Settings::default();
        let raw = RawWindow {
            app_id: "dettivo-osd".into(),
            pid: Some(7),
            title: "secret document.txt".into(),
            xwayland: true,
            window: Some("0x1".into()),
        };
        let target = target_from(&raw, &settings);
        assert!(target.is_dettivo && target.xwayland);
        assert_ne!(target.title_hash, raw.title);
        assert!(!target.title_hash.contains("secret"));
        assert_eq!(target.title_hash, title_hash("secret document.txt"));
        let other = target_from(
            &RawWindow {
                app_id: "foot".into(),
                ..RawWindow::default()
            },
            &settings,
        );
        assert!(!other.is_dettivo && !other.xwayland);
    }

    #[test]
    fn mock_probe_is_chosen_for_mock_sessions() {
        let session = Session {
            mock_focus: true,
            ..Session::default()
        };
        let probe = for_session(&session).unwrap();
        assert_eq!(probe.name(), "mock");
        assert_eq!(probe.focused().unwrap().unwrap().app_id, MOCK_APP_ID);
        assert!(for_session(&Session::default()).is_none());
    }
}
