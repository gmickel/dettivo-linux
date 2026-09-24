//! The target guards (FR-I8): the app id and pid a client captured at
//! hotkey time, and the window identity a dictation session recorded
//! when its key went down, compared with the probed window before any
//! backend runs. Pure decisions, so every rule has a unit test.

use dettivo_proto::methods::insert::Target;

use crate::chain::Refusal;

/// The guards a client captured at hotkey time.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Guards<'a> {
    /// Expected app id (`expected_target_bundle_id`).
    pub app_id: Option<&'a str>,
    /// Expected pid (`expected_target_pid`).
    pub pid: Option<&'a str>,
    /// The window itself, as the probe identified it when a dictation
    /// session started: two windows of one process pass the app id and
    /// the pid, only the original passes this. Never on the wire.
    pub window: Option<&'a str>,
}

impl Guards<'_> {
    /// True when nothing is guarded.
    pub fn is_empty(&self) -> bool {
        self.app_id.is_none() && self.pid.is_none() && self.window.is_none()
    }
}

/// Checks the guards against the probed target (FR-I8) and the probed
/// window identity. A guard with no probed target cannot be verified and
/// is a conflict.
pub fn check_guards(
    guards: &Guards<'_>,
    target: Option<&Target>,
    window: Option<&str>,
) -> Result<(), Refusal> {
    if guards.is_empty() {
        return Ok(());
    }
    let Some(target) = target else {
        return Err(Refusal::Conflict(
            "Frontmost target cannot be verified in this session".into(),
        ));
    };
    if let Some(expected) = guards.app_id {
        if !expected.eq_ignore_ascii_case(&target.app_id) {
            return Err(Refusal::Conflict("Frontmost app mismatch".into()));
        }
    }
    if let Some(expected) = guards.pid {
        let matches = target
            .pid
            .is_some_and(|pid| expected.trim() == pid.to_string());
        if !matches {
            return Err(Refusal::Conflict("Frontmost PID mismatch".into()));
        }
    }
    if let Some(expected) = guards.window {
        if window != Some(expected) {
            return Err(Refusal::Conflict("Frontmost window mismatch".into()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(app: &str, pid: u32) -> Target {
        Target {
            app_id: app.into(),
            pid: Some(pid),
            title_hash: "h".into(),
            is_dettivo: false,
            xwayland: false,
        }
    }

    #[test]
    fn guards_mismatch_is_a_conflict_before_any_backend_runs() {
        let t = target("foot", 42);
        assert_eq!(check_guards(&Guards::default(), None, None), Ok(()));
        assert_eq!(
            check_guards(
                &Guards {
                    app_id: Some("Foot"),
                    pid: Some("42"),
                    window: None,
                },
                Some(&t),
                Some("0x1")
            ),
            Ok(())
        );
        assert_eq!(
            check_guards(
                &Guards {
                    app_id: Some("kitty"),
                    pid: None,
                    window: None,
                },
                Some(&t),
                None
            ),
            Err(Refusal::Conflict("Frontmost app mismatch".into()))
        );
        assert_eq!(
            check_guards(
                &Guards {
                    app_id: None,
                    pid: Some("4242"),
                    window: None,
                },
                Some(&t),
                None
            ),
            Err(Refusal::Conflict("Frontmost PID mismatch".into()))
        );
        assert!(matches!(
            check_guards(
                &Guards {
                    app_id: Some("foot"),
                    pid: None,
                    window: None,
                },
                None,
                None
            ),
            Err(Refusal::Conflict(_))
        ));
    }

    /// Two windows of one process share the app id and the pid; only the
    /// window the session captured passes its identity, and a probe that
    /// cannot name the window any more (the origin closed) is a conflict.
    #[test]
    fn the_window_guard_tells_two_windows_of_one_process_apart() {
        let t = target("foot", 42);
        let guards = Guards {
            app_id: Some("foot"),
            pid: Some("42"),
            window: Some("0x1"),
        };
        assert_eq!(check_guards(&guards, Some(&t), Some("0x1")), Ok(()));
        assert_eq!(
            check_guards(&guards, Some(&t), Some("0x2")),
            Err(Refusal::Conflict("Frontmost window mismatch".into()))
        );
        assert_eq!(
            check_guards(&guards, Some(&t), None),
            Err(Refusal::Conflict("Frontmost window mismatch".into()))
        );
        let window_only = Guards {
            window: Some("0x1"),
            ..Guards::default()
        };
        assert!(!window_only.is_empty());
        assert!(check_guards(&window_only, None, None).is_err());
    }
}
