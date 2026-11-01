//! fn-85 through the session: the first key of a take waits out the
//! modifier settle after the stop (R2, ADR 0090), and a printing stop
//! hands nothing to the inserter and records `printed` (R4).

mod support;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dettivo_proto::runtime::{InsertionMethod, InsertionOutcome, InsertionResult, TargetApp};
use dettivo_session::machine::Session;
use dettivo_session::{Inserter, MODIFIER_SETTLE, SessionTarget, no_context_pack};
use support::{FakeEngine, Feed, Recorder, policy};

/// An inserter that notes when it was called.
#[derive(Default)]
struct Clock(Mutex<Vec<Instant>>);

impl Inserter for Clock {
    fn insert(&self, _: &str, _: Option<&SessionTarget>) -> InsertionResult {
        self.0.lock().unwrap().push(Instant::now());
        InsertionResult {
            outcome: InsertionOutcome::Inserted,
            method: InsertionMethod::Paste,
            target_app: TargetApp {
                bundle_id: "test".into(),
                name: "test".into(),
            },
            context_pack: no_context_pack(),
            reason: None,
            backend: None,
        }
    }
}

/// An engine that answers at once still leaves the inserter waiting
/// until the settle has passed since the stop; a slow engine adds
/// nothing on top of its own time.
#[test]
fn the_first_key_waits_out_the_modifier_settle_after_the_stop() {
    for slow in [Duration::ZERO, Duration::from_millis(400)] {
        let dir = tempfile::tempdir().unwrap();
        let clock = Arc::new(Clock::default());
        let session = Session::new(
            dir.path().join("sessions"),
            Arc::new(Recorder::default()),
            clock.clone(),
        );
        let feed = Feed::new();
        let mut engine = FakeEngine::new("hello");
        Arc::get_mut(&mut engine).unwrap().slow = slow;
        session
            .start(policy(dir.path(), false), engine, feed.clone(), None)
            .unwrap();
        feed.speak(400);
        std::thread::sleep(Duration::from_millis(50));
        let stopped = Instant::now();
        assert_eq!(session.stop().unwrap().text, "Hello");
        let inserted = clock.0.lock().unwrap()[0];
        let waited = inserted.duration_since(stopped);
        assert!(waited >= MODIFIER_SETTLE, "slow={slow:?}: {waited:?}");
        assert!(
            waited < slow.max(MODIFIER_SETTLE) + Duration::from_millis(200),
            "slow={slow:?}: the settle overlaps the transcription: {waited:?}"
        );
    }
}

#[test]
fn a_printing_stop_inserts_nothing_and_records_printed() {
    let (dir, rec, session, engine, feed) = support::rig("hello");
    session
        .start(policy(dir.path(), false), engine, feed.clone(), None)
        .unwrap();
    feed.speak(400);
    std::thread::sleep(Duration::from_millis(50));
    let done = session.stop_print().unwrap();
    assert_eq!(done.text, "Hello");
    let insertion = done.insertion.unwrap();
    assert_eq!(insertion.outcome, InsertionOutcome::Printed);
    assert_eq!(insertion.method, InsertionMethod::Printed);
    assert!(rec.inserted.lock().unwrap().is_empty());
}
