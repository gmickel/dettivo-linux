//! R1 and R5 on the state machine: the full session, audio retention and
//! silence, concurrent starts, and the refusals.

#![allow(unused_imports)]

mod support;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;

use dettivo_audio::{EndReason, Event};
use dettivo_proto::runtime::InsertionOutcome;
use dettivo_session::machine::Session;
use dettivo_session::{SessionError, SessionTarget, State};
use dettivo_speech::EngineError;
use support::{FakeEngine, Feed, Recorder, policy, rig, wait_idle};

#[test]
fn a_full_session_runs_every_state_and_keeps_the_transcript() {
    let (dir, rec, session, engine, feed) = rig("teh quick fox comma done period");
    let started = session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            feed.clone(),
            None,
        )
        .unwrap();
    assert_eq!(started.state, State::Recording);
    assert_eq!(started.job_id, "job_dict_1");
    feed.speak(400);
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(session.snapshot().unwrap().state, State::Recording);
    let t = session.stop().unwrap();
    assert_eq!(t.text, "The quick fox, done.");
    assert!(t.duration_ms >= 380, "{}", t.duration_ms);
    assert!(!t.silent);
    assert_eq!(
        t.insertion.as_ref().unwrap().outcome,
        InsertionOutcome::Inserted
    );
    assert_eq!(
        rec.inserted.lock().unwrap().as_slice(),
        ["The quick fox, done."]
    );
    // The completion carries the split of the path after the stop, and the
    // same block rides on the completion change for the event stream.
    let timings = t.timings.expect("timings on a completed transcript");
    assert!(timings.capture_ms < 5_000, "{timings:?}");
    assert_eq!(
        rec.completion_timings(),
        Some(timings),
        "the completion change carries the transcript's timings"
    );
    assert_eq!(
        rec.sequence(),
        [
            State::Recording,
            State::Transcribing,
            State::Inserting,
            State::Idle
        ]
    );
    assert!(rec.levels.load(Ordering::Relaxed) >= 1);
    assert!(feed.stopped.load(Ordering::Relaxed));
    assert!(session.snapshot().is_none());
    assert_eq!(session.last().unwrap().id.len(), 36);
    assert!(
        !dir.path()
            .join("sessions/job_dict_1/microphone.wav")
            .exists(),
        "take discarded"
    );
    assert!(
        !dir.path()
            .join("sessions/job_dict_1/transcript.json")
            .exists()
    );
    let again = session.reinsert_last().unwrap();
    assert_eq!(rec.inserted.lock().unwrap().len(), 2);
    assert_eq!(again.text, t.text);
}

/// dictation/F18: a voiced take shorter than the meter window carries no
/// level event; its silence is read from the samples and it reaches the
/// engine. A level event over silent samples does not make them speech.
#[test]
fn silence_is_read_from_the_samples_not_the_meter() {
    let (dir, _rec, session, engine, feed) = rig("short take");
    session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            feed.clone(),
            None,
        )
        .unwrap();
    // 20 ms at 0.09 of full scale, well over the 0.01 threshold, and no
    // level event at all.
    feed.send(Event::Pcm(vec![3000i16; 320]));
    std::thread::sleep(Duration::from_millis(50));
    let t = session.stop().unwrap();
    assert!(!t.silent);
    assert_eq!(t.text, "Short take");
    assert_eq!(engine.calls.load(Ordering::Relaxed), 1);

    let (dir, _rec, session, engine, feed) = rig("never used");
    session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            feed.clone(),
            None,
        )
        .unwrap();
    feed.send(Event::Pcm(vec![0i16; 320]));
    feed.send(Event::Level {
        rms: 0.5,
        peak: 0.9,
    });
    std::thread::sleep(Duration::from_millis(50));
    let t = session.stop().unwrap();
    assert!(t.silent);
    assert_eq!(engine.calls.load(Ordering::Relaxed), 0);
}

#[test]
fn unarchived_staging_is_discarded_and_silence_skips_the_engine() {
    let (dir, _rec, session, engine, feed) = rig("never used");
    session
        .start(policy(dir.path(), true), engine.clone(), feed.clone(), None)
        .unwrap();
    for _ in 0..10 {
        feed.send(Event::Pcm(vec![0i16; 320]));
    }
    feed.send(Event::Level {
        rms: 0.0,
        peak: 0.001,
    });
    std::thread::sleep(Duration::from_millis(50));
    let t = session.stop().unwrap();
    assert!(t.silent);
    assert_eq!(t.text, "");
    assert_eq!(
        engine.calls.load(Ordering::Relaxed),
        0,
        "silence never reaches the engine"
    );
    assert!(
        !dir.path()
            .join("sessions/job_dict_1/microphone.wav")
            .exists(),
        "history alone owns retention"
    );
}

#[test]
fn concurrent_starts_let_exactly_one_win() {
    let (dir, _rec, session, engine, feed) = rig("x");
    let session = Arc::new(session);
    let p = policy(dir.path(), false);
    let mut handles = Vec::new();
    for _ in 0..4 {
        let (s, p, e, f) = (session.clone(), p.clone(), engine.clone(), feed.clone());
        handles.push(std::thread::spawn(move || s.start(p, e, f, None).is_ok()));
    }
    let wins: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(wins.iter().filter(|w| **w).count(), 1, "{wins:?}");
    assert_eq!(
        session.start(p, engine, feed.clone(), None).unwrap_err(),
        SessionError::Active
    );
    session.cancel().unwrap();
    wait_idle(&session);
}

/// A source whose open waits until the test lets it through, so a stop
/// can land while the slot is reserved and the worker does not exist yet.
struct GatedFeed {
    inner: Arc<Feed>,
    entered: Arc<Barrier>,
    proceed: Arc<Barrier>,
}

impl dettivo_session::source::SourceFactory for GatedFeed {
    fn open(&self, level: u64) -> Result<dettivo_session::source::Source, String> {
        self.entered.wait();
        self.proceed.wait();
        self.inner.open(level)
    }
}

/// dictation/F11 (fn-43): a take's result belongs to that take. A stop
/// that lands while the source is still opening waits for the take
/// instead of answering `NoSession`, two stoppers of one take both get
/// its result, and the next start neither sees the old take's result
/// nor loses its own.
#[test]
fn every_stopper_of_a_take_gets_that_takes_result() {
    let (dir, _rec, session, engine, feed) = rig("hello world");
    let session = Arc::new(session);
    let entered = Arc::new(Barrier::new(2));
    let proceed = Arc::new(Barrier::new(2));
    let gated = Arc::new(GatedFeed {
        inner: feed.clone(),
        entered: entered.clone(),
        proceed: proceed.clone(),
    });
    let starter = {
        let (s, p, e) = (session.clone(), policy(dir.path(), false), engine.clone());
        std::thread::spawn(move || s.start(p, e, gated, None))
    };
    entered.wait();
    // The slot is reserved, the source is opening: two stoppers arrive.
    let stoppers: Vec<_> = (0..2)
        .map(|_| {
            let s = session.clone();
            std::thread::spawn(move || s.stop())
        })
        .collect();
    std::thread::sleep(Duration::from_millis(100));
    proceed.wait();
    assert!(starter.join().unwrap().is_ok());
    let results: Vec<_> = stoppers
        .into_iter()
        .map(|h| h.join().unwrap().map(|t| t.text))
        .collect();
    for r in &results {
        assert_ne!(*r, Err(SessionError::NoSession), "{results:?}");
    }
    assert_eq!(
        results[0], results[1],
        "both stoppers get the take's result"
    );
    wait_idle(&session);
    // The next take starts at once and its own stop answers for it.
    let next = session
        .start(policy(dir.path(), false), engine, Feed::new(), None)
        .unwrap();
    assert_eq!(next.state, State::Recording);
    assert_ne!(session.stop().map(|t| t.text), Err(SessionError::NoSession));
    wait_idle(&session);
}

#[test]
fn stop_and_cancel_without_a_session_are_not_found_and_modes_are_gated() {
    let (dir, _rec, session, engine, feed) = rig("x");
    assert_eq!(session.stop().unwrap_err(), SessionError::NoSession);
    assert_eq!(session.cancel().unwrap_err(), SessionError::NoSession);
    assert_eq!(
        session.reinsert_last().unwrap_err(),
        SessionError::NoTranscript
    );
    let mut p = policy(dir.path(), false);
    p.mode = "meeting".into();
    assert_eq!(
        session
            .start(p.clone(), engine.clone(), feed.clone(), None)
            .unwrap_err(),
        SessionError::ModeUnknown("meeting".into())
    );
    p.mode = "raw".into();
    p.model_path = dir.path().join("absent.bin");
    match session.start(p, engine, feed, None).unwrap_err() {
        SessionError::ModelMissing {
            model,
            recommended_action,
        } => {
            assert_eq!(model, "fake/tiny");
            assert!(recommended_action.contains("dettivo speech download --model tiny"));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_target_captured_at_start_reaches_the_snapshot_and_the_inserter() {
    let (dir, rec, session, engine, feed) = rig("hello");
    let target = SessionTarget {
        app_id: Some("foot".into()),
        pid: Some(4242),
        window: Some("0x1".into()),
        unverified: false,
    };
    let started = session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            feed.clone(),
            Some(target.clone()),
        )
        .unwrap();
    assert_eq!(started.target.as_ref(), Some(&target));
    assert_eq!(session.snapshot().unwrap().target, Some(target.clone()));
    feed.speak(200);
    let t = session.stop().unwrap();
    assert_eq!(t.target, Some(target.clone()));
    assert_eq!(rec.targets.lock().unwrap().as_slice(), [Some(target)]);
    // A re-insert carries no guard: the user has moved on.
    session.reinsert_last().unwrap();
    assert_eq!(rec.targets.lock().unwrap().last().unwrap(), &None);
}
