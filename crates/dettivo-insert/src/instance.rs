//! Which Hyprland instance answers for this session (ADR 0099). The
//! environment's `HYPRLAND_INSTANCE_SIGNATURE` is a hint, not the truth:
//! a compositor that restarted has a new signature while the daemon's
//! environment (and the systemd user environment it came from) keeps the
//! old one. When the instance in use refuses the connection, the
//! instances under `$XDG_RUNTIME_DIR/hypr/` whose `hyprland.lock` names
//! this session's Wayland display are tried newest first, and the first
//! that accepts is the one in use from then on. An instance of another
//! display, or outside this user's runtime directory, is never tried; that
//! holds for the environment's instance too, unless its lock names no
//! display (an older Hyprland), which keeps the hint as it is.

use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
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

/// The display socket an instance's `hyprland.lock` names: Hyprland writes
/// its pid, then its Wayland display's name, a socket under the runtime
/// directory. `None` when the lock is unreadable or has no display line.
fn lock_display(runtime: &Path, lock: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(lock).ok()?;
    let name = text.lines().nth(1)?.trim();
    (!name.is_empty()).then(|| runtime.join(name))
}

/// True when the instance's lock names a display other than the session's.
/// A lock without a display line cannot say, so the instance is not ruled
/// out.
fn of_another_display(session: &Session, signature: &str) -> bool {
    let (Some(runtime), Some(display)) = (&session.runtime_dir, session.wayland_socket()) else {
        return false;
    };
    let lock = runtime.join("hypr").join(signature).join("hyprland.lock");
    lock_display(runtime, &lock).is_some_and(|named| named != display)
}

/// The instances under the runtime directory whose display socket is the
/// session's, other than `skip`, newest first by their lock file.
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
            if lock_display(runtime, &lock).as_ref() != Some(&display) {
                return None;
            }
            let socket = dir.join(".socket.sock");
            let modified = std::fs::metadata(&lock).and_then(|m| m.modified()).ok()?;
            socket.exists().then_some((modified, signature, socket))
        })
        .collect();
    found.sort_by_key(|a| std::cmp::Reverse(a.0));
    found.into_iter().map(|(_, s, p)| (s, p)).collect()
}

impl Instances {
    /// A connection to the session's Hyprland IPC: the instance in use
    /// (at first the environment's), else the live instance for this
    /// display, found again on every refused connection.
    pub fn connect(&self, session: &Session) -> Result<UnixStream, String> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let previous = state
            .in_use
            .clone()
            .or_else(|| session.hyprland_signature.clone());
        let hint = previous
            .clone()
            .filter(|signature| !of_another_display(session, signature));
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
                    from = %previous.as_deref().unwrap_or("none"),
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

    /// What `insert.target` reports: the instance in use, whether the
    /// environment's signature names another one, and whether that one
    /// belongs to another display. `None` outside Hyprland.
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
            environment_other_display: environment
                .as_deref()
                .is_some_and(|signature| of_another_display(session, signature)),
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
            assert!(!report.environment_other_display);

            let _live = HyprlandStandIn::start(dir.path(), "live", "wl-0").unwrap();
            assert!(instances.connect(&session).is_ok(), "absolute: {absolute}");
            assert_eq!(
                instances.report(&session).unwrap().signature.as_deref(),
                Some("live")
            );
        }
    }

    /// An older Hyprland's lock names no display, so the environment's
    /// instance cannot be checked and is used as before.
    #[test]
    fn an_environment_instance_whose_lock_names_no_display_is_kept() {
        let dir = tempfile::Builder::new()
            .prefix("dtv")
            .tempdir_in("/tmp")
            .unwrap();
        let _old = HyprlandStandIn::start(dir.path(), "old", "wl-0").unwrap();
        std::fs::write(dir.path().join("hypr/old/hyprland.lock"), "1\n").unwrap();
        let session = Session {
            runtime_dir: Some(dir.path().to_path_buf()),
            wayland_display: Some("wl-1".into()),
            hyprland_signature: Some("old".into()),
            ..Session::default()
        };
        let instances = Instances::default();
        assert!(instances.connect(&session).is_ok());
        let report = instances.report(&session).unwrap();
        assert_eq!(report.signature.as_deref(), Some("old"));
        assert!(!report.environment_stale);
        assert!(!report.environment_other_display);
    }

    /// An environment naming another display's live instance is reported
    /// as such, not as a restart.
    #[test]
    fn an_environment_instance_of_another_display_is_reported_as_such() {
        let dir = tempfile::Builder::new()
            .prefix("dtv")
            .tempdir_in("/tmp")
            .unwrap();
        let other = HyprlandStandIn::start(dir.path(), "other", "wl-9").unwrap();
        let _live = HyprlandStandIn::start(dir.path(), "live", "wl-0").unwrap();
        let session = Session {
            runtime_dir: Some(dir.path().to_path_buf()),
            wayland_display: Some("wl-0".into()),
            hyprland_signature: Some("other".into()),
            ..Session::default()
        };
        let instances = Instances::default();
        assert!(instances.connect(&session).is_ok());
        assert_eq!(other.queries(), 0);
        let report = instances.report(&session).unwrap();
        assert_eq!(report.signature.as_deref(), Some("live"));
        assert!(report.environment_stale);
        assert!(report.environment_other_display);
    }
}
