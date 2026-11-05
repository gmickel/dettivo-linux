//! `Session::while_recording` (ADR 0083): the hold key's lock notice runs
//! only while the take records, and never after the take began to end, so
//! a `recording` notice cannot follow the take's terminal state.

mod support;

use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Instant;

use dettivo_session::machine::Session;
use dettivo_session::{Level, Publisher, State, StateChange};
use support::{Recorder, policy, wait_idle};

/// Tries the notice from inside each state the take publishes, as a
/// concurrent hold key would when it races the take's end.
#[derive(Default)]
struct Probe {
    session: OnceLock<Weak<Session>>,
    ran: Mutex<Vec<(State, bool)>>,
}

impl Publisher for Probe {
    fn state(&self, change: &StateChange) {
        let Some(session) = self.session.get().and_then(Weak::upgrade) else {
            return;
        };
        let ran = session.while_recording(&change.job_id, || ()).is_some();
        self.ran.lock().unwrap().push((change.state, ran));
    }
    fn level(&self, _: Level) {}
}

#[test]
fn the_notice_runs_while_the_take_records_and_never_once_it_ends() {
    let dir = tempfile::tempdir().unwrap();
    let probe = Arc::new(Probe::default());
    let session = Arc::new(Session::new(
        dir.path().join("sessions"),
        probe.clone(),
        Arc::new(Recorder::default()),
    ));
    probe.session.set(Arc::downgrade(&session)).ok();
    let feed = support::Feed::new();
    let snap = session
        .start(
            policy(dir.path(), false),
            support::FakeEngine::new("x"),
            feed.clone(),
            None,
            Instant::now(),
        )
        .unwrap();
    feed.speak(100);
    assert_eq!(session.while_recording(&snap.job_id, || 7), Some(7));
    assert_eq!(session.while_recording("job_dict_99", || 7), None);

    session.cancel().unwrap();
    wait_idle(&session);
    assert_eq!(session.while_recording(&snap.job_id, || 7), None);
    // Recording itself ran it; the cancelled and idle states the ending
    // take published did not, though its slot was still taken.
    assert_eq!(
        *probe.ran.lock().unwrap(),
        [
            (State::Recording, true),
            (State::Cancelled, false),
            (State::Idle, false)
        ]
    );
}
