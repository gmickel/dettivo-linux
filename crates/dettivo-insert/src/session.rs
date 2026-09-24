//! What the desktop session offers: the displays, the compositor, the
//! helper sockets and binaries the backends and probes check before they
//! run. Built from the environment once per call so a session that gains
//! a display is seen without a restart; built by hand in tests.

use std::path::{Path, PathBuf};

/// The QA mock (`DETTIVO_MOCK_INSERT`): where the mock backend records
/// what it "typed".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mock {
    /// The file the mock backend appends inserted text to.
    pub inserted_file: PathBuf,
}

/// The session facts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Session {
    /// `WAYLAND_DISPLAY`.
    pub wayland_display: Option<String>,
    /// `DISPLAY`.
    pub x11_display: Option<String>,
    /// `HYPRLAND_INSTANCE_SIGNATURE`.
    pub hyprland_signature: Option<String>,
    /// `XDG_RUNTIME_DIR`.
    pub runtime_dir: Option<PathBuf>,
    /// `XDG_CURRENT_DESKTOP`, first entry.
    pub desktop: Option<String>,
    /// The ydotool socket when it exists.
    pub ydotool_socket: Option<PathBuf>,
    /// `ydotool` is on `PATH`.
    pub has_ydotool: bool,
    /// `xdotool` is on `PATH`.
    pub has_xdotool: bool,
    /// The session bus is reachable (a portal needs one).
    pub has_session_bus: bool,
    /// The QA mock, when `DETTIVO_MOCK_INSERT` is on.
    pub mock: Option<Mock>,
    /// `DETTIVO_MOCK_A11Y`: the focus probe answers with a fixed target.
    pub mock_focus: bool,
}

fn on_path(tool: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(tool).is_file()))
        .unwrap_or(false)
}

impl Session {
    /// Reads the process environment. `mock` names the QA mock file when
    /// the daemon runs with `DETTIVO_MOCK_INSERT`.
    pub fn from_env(mock: Option<Mock>, mock_focus: bool) -> Self {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let runtime_dir = var("XDG_RUNTIME_DIR").map(PathBuf::from);
        let ydotool_socket = var("YDOTOOL_SOCKET")
            .map(PathBuf::from)
            .or_else(|| {
                runtime_dir
                    .as_ref()
                    .map(|r| r.join(".ydotool_socket"))
                    .or_else(|| {
                        // SAFETY: getuid has no preconditions and cannot fail.
                        #[allow(unsafe_code)]
                        let uid = unsafe { libc::getuid() };
                        Some(PathBuf::from(format!("/run/user/{uid}/.ydotool_socket")))
                    })
            })
            .filter(|p| p.exists());
        let has_session_bus = var("DBUS_SESSION_BUS_ADDRESS").is_some()
            || runtime_dir.as_ref().is_some_and(|r| r.join("bus").exists());
        Self {
            wayland_display: var("WAYLAND_DISPLAY"),
            x11_display: var("DISPLAY"),
            hyprland_signature: var("HYPRLAND_INSTANCE_SIGNATURE"),
            desktop: var("XDG_CURRENT_DESKTOP")
                .map(|d| d.split(':').next().unwrap_or(&d).to_string()),
            ydotool_socket,
            has_ydotool: on_path("ydotool"),
            has_xdotool: on_path("xdotool"),
            has_session_bus,
            runtime_dir,
            mock,
            mock_focus,
        }
    }

    /// The Hyprland IPC socket when this is a Hyprland session.
    pub fn hyprland_socket(&self) -> Option<PathBuf> {
        let sig = self.hyprland_signature.as_ref()?;
        let runtime = self.runtime_dir.as_deref().unwrap_or(Path::new("/tmp"));
        let modern = runtime.join("hypr").join(sig).join(".socket.sock");
        if modern.exists() {
            return Some(modern);
        }
        let legacy = Path::new("/tmp/hypr").join(sig).join(".socket.sock");
        legacy.exists().then_some(legacy)
    }

    /// True when a Wayland display is reachable.
    pub fn is_wayland(&self) -> bool {
        self.wayland_display.is_some()
    }

    /// True when an X11 display (XWayland included) is reachable.
    pub fn is_x11(&self) -> bool {
        self.x11_display.is_some()
    }
}
