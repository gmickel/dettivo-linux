//! R1 exits: cancel in every state, device loss, an engine crash, a source
//! that fails to open, and the frozen policy with the maximum duration.

#![allow(unused_imports)]

mod support;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dettivo_audio::{EndReason, Event};
use dettivo_proto::runtime::InsertionOutcome;
use dettivo_session::machine::Session;
use dettivo_session::{SessionError, State};
use dettivo_speech::EngineError;
use support::{FakeEngine, Feed, Recorder, policy, rig, wait_idle};

#[test]
fn cancel_works_in_every_state() {
    // Recording.
    let (dir, rec, session, engine, feed) = rig("x");
    session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            feed.clone(),
            None,
        )
        .unwrap();
    feed.speak(100);
    let c = session.cancel().unwrap();
    assert_eq!(c.state, State::Cancelled);
    assert!(session.last().is_none());
    assert_eq!(
        rec.sequence(),
        [State::Recording, State::Cancelled, State::Idle]
    );

    // Transcribing: a slow engine is cancelled through the engine's cancel.
    let dir2 = tempfile::tempdir().unwrap();
    let rec2 = Arc::new(Recorder::default());
    let session2 = Session::new(dir2.path().join("s"), rec2.clone(), rec2.clone());
    let slow = Arc::new(FakeEngine {
        text: "late".into(),
        fail: None,
        slow: Duration::from_secs(5),
        cancelled: AtomicBool::new(false),
        calls: AtomicUsize::new(0),
    });
    let feed2 = Feed::new();
    session2
        .start(
            policy(dir2.path(), false),
            slow.clone(),
            feed2.clone(),
            None,
        )
        .unwrap();
    feed2.speak(100);
    let s2 = Arc::new(session2);
    let stopper = {
        let s = s2.clone();
        std::thread::spawn(move || s.stop())
    };
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(s2.snapshot().unwrap().state, State::Transcribing);
    let c = s2.cancel().unwrap();
    assert_eq!(c.state, State::Cancelled);
    assert_eq!(
        stopper.join().unwrap().unwrap_err(),
        SessionError::Cancelled
    );
    assert!(s2.last().is_none());
    assert_eq!(
        rec2.sequence(),
        [
            State::Recording,
            State::Transcribing,
            State::Cancelled,
            State::Idle
        ]
    );
}

#[test]
fn device_loss_ends_the_session_recoverably() {
    let (dir, rec, session, engine, feed) = rig("x");
    session
        .start(policy(dir.path(), false), engine, feed.clone(), None)
        .unwrap();
    feed.speak(100);
    feed.send(Event::Ended {
        reason: EndReason::DeviceLost("alsa_input.usb".into()),
    });
    wait_idle(&session);
    let err = session.stop().unwrap_err();
    assert_eq!(err, SessionError::NoSession);
    let states = rec.states.lock().unwrap();
    let failed = states.iter().find(|c| c.state == State::Failed).unwrap();
    assert!(
        failed
            .reason
            .as_deref()
            .unwrap()
            .contains("device lost: alsa_input.usb"),
        "{failed:?}"
    );
    assert_eq!(states.last().unwrap().state, State::Idle);
    drop(states);
    // The next session starts cleanly.
    let (dir2, _r, _s, _e, _f) = rig("x");
    assert!(
        session
            .start(
                policy(dir2.path(), false),
                FakeEngine::new("y"),
                Feed::new(),
                None
            )
            .is_ok()
    );
    session.cancel().unwrap();
}

#[test]
fn an_engine_crash_fails_the_session_and_the_next_one_runs() {
    let (dir, rec, session, _engine, feed) = rig("x");
    let crashing = Arc::new(FakeEngine {
        text: String::new(),
        fail: Some(EngineError::Crashed("segfault MARKER_TRANSCRIPT".into())),
        slow: Duration::ZERO,
        cancelled: AtomicBool::new(false),
        calls: AtomicUsize::new(0),
    });
    session
        .start(policy(dir.path(), false), crashing, feed.clone(), None)
        .unwrap();
    feed.speak(100);
    let err = session.stop().unwrap_err();
    match err {
        SessionError::Failed(reason) => {
            assert!(reason.starts_with("engine:"), "{reason}");
            assert!(!reason.contains("MARKER"), "crash detail leaked: {reason}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        rec.sequence(),
        [
            State::Recording,
            State::Transcribing,
            State::Failed,
            State::Idle
        ]
    );
    let good = FakeEngine::new("fine");
    session
        .start(policy(dir.path(), false), good, feed.clone(), None)
        .unwrap();
    feed.speak(100);
    assert_eq!(session.stop().unwrap().text, "Fine");
}

#[test]
fn a_source_that_fails_to_open_reports_audio_and_frees_the_slot() {
    let (dir, rec, session, engine, _feed) = rig("x");
    let broken = Arc::new(Feed {
        tx: Mutex::new(None),
        handle: Mutex::new(None),
        stopped: Arc::new(AtomicBool::new(false)),
        fail_open: true,
    });
    let err = session
        .start(policy(dir.path(), false), engine.clone(), broken, None)
        .unwrap_err();
    assert_eq!(err, SessionError::Audio("no microphone".into()));
    assert!(session.snapshot().is_none());
    assert_eq!(rec.sequence(), [State::Failed, State::Idle]);
    assert!(
        session
            .start(policy(dir.path(), false), engine, Feed::new(), None)
            .is_ok()
    );
    session.cancel().unwrap();
}

#[test]
fn the_policy_is_frozen_at_start_and_the_take_stops_at_the_maximum() {
    let (dir, _rec, session, engine, feed) = rig("late words");
    let mut p = policy(dir.path(), false);
    p.max_duration = Duration::from_millis(300);
    let snap = session.start(p, engine, feed.clone(), None).unwrap();
    assert_eq!(snap.policy.model, "tiny");
    feed.speak(200);
    std::thread::sleep(Duration::from_millis(400));
    // The maximum passed: the session finished on its own.
    wait_idle(&session);
    let t = session.last().unwrap();
    assert_eq!(t.text, "Late words");
    assert_eq!(t.policy.model, "tiny");
}
