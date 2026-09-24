//! The self-target allowance (ADR 0024): the chain refuses to type into
//! one of Dettivo's own windows (`target_is_self`), and the first-run Try
//! it step is the one place that wants exactly that, into its own field.
//! The app arms the allowance on its connection while the step is on
//! screen; it covers the app's window ids and never the pill's, and it
//! ends the moment the app disarms it or its connection closes, so a
//! dictation started later from any binding lands in the desktop again.

use std::sync::Mutex;

/// The app ids an armed allowance covers: the app window's, never the
/// pill's or the sheet's.
pub const APP_IDS: &[&str] = &["dettivo", "dettivo-app"];

/// The process name a peer must carry to arm the allowance outside QA
/// mode.
pub const APP_COMM: &str = "dettivo-app";

/// The allowance: at most one connection holds it.
#[derive(Debug, Default)]
pub struct SelfTargetAllowance {
    armed_by: Mutex<Option<u64>>,
}

/// Why a peer may not arm the allowance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotTheApp {
    /// What the peer looked like.
    pub detail: String,
}

impl SelfTargetAllowance {
    /// Arms the allowance for `conn` when the peer is the app (its
    /// process name is `dettivo-app`) or the daemon runs in QA mode.
    pub fn arm(&self, conn: u64, peer_pid: Option<u32>, qa_mode: bool) -> Result<(), NotTheApp> {
        if !qa_mode {
            let comm = peer_pid.and_then(|pid| {
                std::fs::read_to_string(format!("/proc/{pid}/comm"))
                    .ok()
                    .map(|c| c.trim().to_string())
            });
            if comm.as_deref() != Some(APP_COMM) {
                return Err(NotTheApp {
                    detail: match (peer_pid, comm) {
                        (Some(pid), Some(comm)) => format!("peer pid {pid} is {comm}"),
                        (Some(pid), None) => format!("peer pid {pid} is unreadable"),
                        (None, _) => "the peer's pid is unknown".to_string(),
                    },
                });
            }
        }
        *self.armed_by.lock().unwrap_or_else(|p| p.into_inner()) = Some(conn);
        tracing::info!(conn, "self-target allowance armed");
        Ok(())
    }

    /// Disarms it when `conn` holds it.
    pub fn disarm(&self, conn: u64) {
        let mut g = self.armed_by.lock().unwrap_or_else(|p| p.into_inner());
        if *g == Some(conn) {
            *g = None;
            tracing::info!(conn, "self-target allowance disarmed");
        }
    }

    /// True while any connection holds it.
    pub fn armed(&self) -> bool {
        self.armed_by
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_allowance_is_held_by_one_connection_and_ends_with_it() {
        let a = SelfTargetAllowance::default();
        assert!(!a.armed());
        // This test process is not dettivo-app, so outside QA mode the arm
        // is refused with the peer named.
        let refused = a.arm(1, Some(std::process::id()), false).unwrap_err();
        assert!(refused.detail.contains("peer pid"), "{}", refused.detail);
        assert!(!a.armed());
        a.arm(1, None, true).unwrap();
        assert!(a.armed());
        a.disarm(2);
        assert!(a.armed(), "another connection cannot disarm it");
        a.disarm(1);
        assert!(!a.armed());
        let unknown = a.arm(3, None, false).unwrap_err();
        assert_eq!(unknown.detail, "the peer's pid is unknown");
    }
}
