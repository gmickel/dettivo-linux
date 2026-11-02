//! fn-85 through the session: the first key of a take waits out the
//! modifier settle after the stop (R2, ADR 0090), and a printing stop
//! hands nothing to the inserter and records `printed` (R4).

mod support;

use std::sync::{Arc, Mutex, mpsc};
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
            .start(
                policy(dir.path(), false),
                engine,
                feed.clone(),
                None,
                Instant::now(),
            )
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
        .start(
            policy(dir.path(), false),
            engine,
            feed.clone(),
            None,
            Instant::now(),
        )
        .unwrap();
    feed.speak(400);
    std::thread::sleep(Duration::from_millis(50));
    let done = session.stop_print(Instant::now()).unwrap();
    assert_eq!(done.text, "Hello");
    let insertion = done.insertion.unwrap();
    assert_eq!(insertion.outcome, InsertionOutcome::Printed);
    assert_eq!(insertion.method, InsertionMethod::Printed);
    assert!(rec.inserted.lock().unwrap().is_empty());
}

/// An inserter that reports each call and holds it until released, so
/// a test can act while the worker is committed to inserting.
struct Held {
    entered: Mutex<mpsc::Sender<()>>,
    release: Mutex<mpsc::Receiver<()>>,
    rec: Recorder,
}

impl Inserter for Held {
    fn insert(&self, text: &str, target: Option<&SessionTarget>) -> InsertionResult {
        self.entered.lock().unwrap().send(()).unwrap();
        self.release.lock().unwrap().recv().unwrap();
        self.rec.insert(text, target)
    }
}

/// A printing stop and the worker's commitment race for the take: a
/// print that arrives while the engine works wins for every stopper, and
/// one that arrives once the text is committed to the inserter is
/// refused instead of reporting a print that never happened.
#[test]
fn a_print_counts_only_until_the_text_is_committed() {
    for committed in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let (entered_tx, entered) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let held = Arc::new(Held {
            entered: Mutex::new(entered_tx),
            release: Mutex::new(release_rx),
            rec: Recorder::default(),
        });
        let session = Arc::new(Session::new(
            dir.path().join("sessions"),
            Arc::new(Recorder::default()),
            held.clone(),
        ));
        let feed = Feed::new();
        let mut engine = FakeEngine::new("hello");
        // While the engine works for 400 ms the route is still open.
        Arc::get_mut(&mut engine).unwrap().slow = if committed {
            Duration::ZERO
        } else {
            Duration::from_millis(400)
        };
        session
            .start(
                policy(dir.path(), false),
                engine,
                feed.clone(),
                None,
                Instant::now(),
            )
            .unwrap();
        feed.speak(400);
        std::thread::sleep(Duration::from_millis(50));
        let s = session.clone();
        let plain = std::thread::spawn(move || s.stop());
        if committed {
            // The worker is inside the inserter: the text is committed.
            entered.recv_timeout(Duration::from_secs(5)).unwrap();
        } else {
            std::thread::sleep(Duration::from_millis(100));
        }
        let print = session.stop_print(Instant::now());
        release.send(()).unwrap();
        let plain = plain.join().unwrap().unwrap();
        let outcome = plain.insertion.unwrap().outcome;
        if committed {
            assert_eq!(
                print.unwrap_err(),
                dettivo_session::SessionError::Delivering
            );
            assert_eq!(outcome, InsertionOutcome::Inserted);
            assert_eq!(*held.rec.inserted.lock().unwrap(), ["Hello"]);
        } else {
            assert_eq!(print.unwrap().text, "Hello");
            assert_eq!(outcome, InsertionOutcome::Printed);
            assert!(entered.try_recv().is_err(), "nothing reached the inserter");
            assert!(held.rec.inserted.lock().unwrap().is_empty());
        }
    }
}
