//! What may run only while a take records: the hold key's lock notice
//! (ADR 0083). It runs under the session's lock, so the take cannot begin
//! to end meanwhile, and not at all once the take's end began, so a
//! notice never follows the take's terminal state.

use std::sync::Mutex;

use super::{Active, Session};
use crate::State;

impl Session {
    /// Runs `f` while the take `job_id` records and has not begun to end;
    /// `None` when it does not. `f` must not block: the session waits.
    pub fn while_recording<R>(&self, job_id: &str, f: impl FnOnce() -> R) -> Option<R> {
        let g = self.active.lock().unwrap_or_else(|p| p.into_inner());
        let a = g.as_ref()?;
        (a.snapshot.job_id == job_id && a.snapshot.state == State::Recording && !a.ending).then(f)
    }
}

/// Marks the take as ending, under the lock and before its terminal state
/// is published, so nothing [`Session::while_recording`] runs follows it.
pub(crate) fn mark_ending(active: &Mutex<Option<Active>>) {
    if let Some(a) = active.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
        a.ending = true;
    }
}
