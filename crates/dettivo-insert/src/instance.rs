//! Which Hyprland instance answers for this session (ADR 0099). The
//! environment's `HYPRLAND_INSTANCE_SIGNATURE` is a hint, not the truth:
//! a compositor that restarted has a new signature while the daemon's
//! environment (and the systemd user environment it came from) keeps the
//! old one. When the instance in use refuses the connection, the
//! instances under `$XDG_RUNTIME_DIR/hypr/` whose `hyprland.lock` names
//! this session's Wayland display are tried newest first, and the first
//! that accepts is the one in use from then on. An instance of another
//! display, or outside this user's runtime directory, is never tried.

use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

use dettivo_proto::methods::insert::CompositorInstance;

use crate::session::Session;

#[derive(Default)]
struct State {
    /// The signature of the instance that last accepted a connection.
    in_use: Option<String>,
    /// No instance answered at the last try (logged once).
    lost: bool,
}

/// The instance in use, shared by every probe of one insertion service.
#[derive(Default)]
pub struct Instances {
    state: Mutex<State>,
}

/// The instances under the runtime directory whose display socket is the
/// session's,
/// other than `skip`, newest first by their lock file.
fn live_candidates(session: &Session, skip: Option<&str>) -> Vec<(String, PathBuf)> {
    let (Some(runtime), Some(display)) = (&session.runtime_dir, session.wayland_socket()) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(runtime.join("hypr")) else {
        return Vec::new();
    };
    let mut found: Vec<(SystemTime, String, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let signature = entry.file_name().into_string().ok()?;
            if Some(signature.as_str()) == skip {
                return None;
            }
            let dir = entry.path();
            let lock = dir.join("hyprland.lock");
            let text = std::fs::read_to_string(&lock).ok()?;
            // Hyprland writes its pid, then its Wayland display's name,
            // which names a socket under the runtime directory.
            if text.lines().nth(1).map(|name| runtime.join(name.trim())) != Some(display.clone()) {
                return None;
            }
            let socket = dir.join(".socket.sock");
            let modified = std::fs::metadata(&lock).and_then(|m| m.modified()).ok()?;
            socket.exists().then_some((modified, signature, socket))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().map(|(_, s, p)| (s, p)).collect()
}

impl Instances {
    /// A connection to the session's Hyprland IPC: the instance in use
    /// (at first the environment's), else the live instance for this
    /// display, found again on every refused connection.
    pub fn connect(&self, session: &Session) -> Result<UnixStream, String> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let hint = state
            .in_use
            .clone()
            .or_else(|| session.hyprland_signature.clone());
        let mut refused = None;
        if let Some(path) = hint.as_deref().and_then(|s| session.hyprland_socket(s)) {
            match UnixStream::connect(&path) {
                Ok(stream) => {
                    state.in_use = hint;
                    state.lost = false;
                    return Ok(stream);
                }
                Err(e) => refused = Some(format!("hyprland socket {}: {e}", path.display())),
            }
        }
        for (signature, socket) in live_candidates(session, hint.as_deref()) {
            if let Ok(stream) = UnixStream::connect(&socket) {
                tracing::info!(
                    from = hint.as_deref().unwrap_or("none"),
                    to = %signature,
                    "hyprland: switched to the live instance"
                );
                state.in_use = Some(signature);
                state.lost = false;
                return Ok(stream);
            }
        }
        let why = refused
            .unwrap_or_else(|| "no Hyprland instance answers for this Wayland display".to_string());
        if !state.lost {
            tracing::warn!(reason = %why, "hyprland: no instance answers; the focused window cannot be verified");
            state.lost = true;
        }
        state.in_use = None;
        Err(why)
    }

    /// What `insert.target` reports: the instance in use and whether the
    /// environment's signature names another one. `None` outside Hyprland.
    pub fn report(&self, session: &Session) -> Option<CompositorInstance> {
        let in_use = self
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .in_use
            .clone();
        let environment = session.hyprland_signature.clone();
        if in_use.is_none() && environment.is_none() {
            return None;
        }
        Some(CompositorInstance {
            environment_stale: environment.is_some() && environment != in_use,
            signature: in_use,
            environment_signature: environment,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stand_in::HyprlandStandIn;

    /// The boundary: a live instance of another display is never
    /// connected to, and with none of this display live the probe fails
    /// (the take goes to the clipboard) and the report says so. The
    /// display matches whether `WAYLAND_DISPLAY` is a name or the socket's
    /// path.
    #[test]
    fn only_an_instance_of_this_display_is_ever_tried() {
        for absolute in [false, true] {
            let dir = tempfile::Builder::new()
                .prefix("dtv")
                .tempdir_in("/tmp")
                .unwrap();
            let display = if absolute {
                dir.path().join("wl-0").to_string_lossy().into_owned()
            } else {
                "wl-0".to_string()
            };
            let session = Session {
                runtime_dir: Some(dir.path().to_path_buf()),
                wayland_display: Some(display),
                hyprland_signature: Some("dead".into()),
                ..Session::default()
            };
            let mut dead = HyprlandStandIn::start(dir.path(), "dead", "wl-0").unwrap();
            dead.crash();
            let other = HyprlandStandIn::start(dir.path(), "other", "wl-9").unwrap();
            let instances = Instances::default();
            assert!(instances.connect(&session).is_err());
            assert!(instances.connect(&session).is_err());
            assert_eq!(other.queries(), 0);
            let report = instances.report(&session).unwrap();
            assert_eq!(report.signature, None);
            assert!(report.environment_stale);

            let _live = HyprlandStandIn::start(dir.path(), "live", "wl-0").unwrap();
            assert!(instances.connect(&session).is_ok(), "absolute: {absolute}");
            assert_eq!(
                instances.report(&session).unwrap().signature.as_deref(),
                Some("live")
            );
        }
    }
}
