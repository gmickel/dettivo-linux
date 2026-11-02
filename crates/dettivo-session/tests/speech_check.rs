//! fn-90 through the session (ADR 0097): the speech check runs while the
//! take is captured and never holds the capture up. A check far slower
//! than the capture still leaves every sample in the take, and what it
//! heard plus what the release added is the take, in order (R3). A check
//! that fails leaves the release to judge the whole take (R1). A tap
//! never reaches the detector, and a take that ends on its own, by a
//! cancel or a lost device, ends its check before the next take can start.

mod support;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dettivo_audio::Event;
use dettivo_session::SessionError;
use dettivo_speech::EngineError;
use dettivo_speech::gate::{SpeechStream, VoiceDetector};
use support::{policy, rig};

/// A voice pass whose stream takes `slow` per piece and keeps every sample
/// it was given; `broken` fails the first piece.
struct Heard {
    slow: Duration,
    broken: bool,
    speech_ms: u64,
    samples: Arc<Mutex<Vec<i16>>>,
    pieces: Arc<AtomicUsize>,
    whole: Arc<AtomicUsize>,
}

struct HeardStream {
    slow: Duration,
    broken: bool,
    speech_ms: u64,
    samples: Arc<Mutex<Vec<i16>>>,
    pieces: Arc<AtomicUsize>,
}

impl VoiceDetector for Heard {
    fn speech_ms(&self, _: &[i16]) -> Result<u64, EngineError> {
        self.whole.fetch_add(1, Ordering::Relaxed);
        Ok(self.speech_ms)
    }

    fn stream(&self) -> Option<Box<dyn SpeechStream>> {
        Some(Box::new(HeardStream {
            slow: self.slow,
            broken: self.broken,
            speech_ms: self.speech_ms,
            samples: self.samples.clone(),
            pieces: self.pieces.clone(),
        }))
    }
}

impl SpeechStream for HeardStream {
    fn feed(&mut self, pcm: &[i16]) -> Result<(), EngineError> {
        if self.broken {
            return Err(EngineError::Crashed("gone".into()));
        }
        std::thread::sleep(self.slow);
        self.pieces.fetch_add(1, Ordering::Relaxed);
        self.samples.lock().unwrap().extend_from_slice(pcm);
        Ok(())
    }

    fn finish(&mut self, rest: &[i16]) -> Result<u64, EngineError> {
        self.samples.lock().unwrap().extend_from_slice(rest);
        Ok(self.speech_ms)
    }
}

fn heard(slow: Duration, broken: bool, speech_ms: u64) -> Arc<Heard> {
    Arc::new(Heard {
        slow,
        broken,
        speech_ms,
        samples: Arc::default(),
        pieces: Arc::default(),
        whole: Arc::default(),
    })
}

/// Ten seconds of 20 ms chunks, each one distinct, so order is checked.
fn take() -> Vec<Vec<i16>> {
    (0..500)
        .map(|c: i16| {
            (0..320)
                .map(|i| if i % 16 < 8 { 2_000 + c } else { -2_000 - c })
                .collect()
        })
        .collect()
}

#[test]
fn a_check_slower_than_the_capture_drops_no_sample() {
    let (dir, _rec, session, engine, feed) = rig("ok");
    // 200 ms a piece: a check that ran each 20 ms chunk alone would need
    // 100 s for this 10 s take.
    let vad = heard(Duration::from_millis(200), false, 900);
    session.set_vad(Some(vad.clone()));
    session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            feed.clone(),
            None,
            Instant::now(),
        )
        .unwrap();
    let chunks = take();
    for chunk in &chunks {
        feed.send(Event::pcm(chunk.clone()));
    }
    std::thread::sleep(Duration::from_millis(50));
    let stopped = Instant::now();
    assert_eq!(session.stop().unwrap().text, "Ok");
    let all: Vec<i16> = chunks.concat();
    assert_eq!(
        engine.last_samples.load(Ordering::Relaxed),
        all.len(),
        "every sample reached the take"
    );
    assert_eq!(
        *vad.samples.lock().unwrap(),
        all,
        "the check heard the take, in order"
    );
    assert_eq!(vad.whole.load(Ordering::Relaxed), 0, "no whole-take pass");
    assert!(
        vad.pieces.load(Ordering::Relaxed) < 10,
        "the backlog runs in pieces, not chunk by chunk"
    );
    // The release waits for at most the piece the check is running.
    assert!(
        stopped.elapsed() < Duration::from_secs(2),
        "{:?}",
        stopped.elapsed()
    );
}

#[test]
fn a_failed_check_leaves_the_release_to_judge_the_whole_take() {
    for (speech_ms, outcome) in [(0, Err(SessionError::NoSpeech)), (600, Ok("Ok"))] {
        let (dir, _rec, session, engine, feed) = rig("ok");
        let vad = heard(Duration::ZERO, true, speech_ms);
        session.set_vad(Some(vad.clone()));
        session
            .start(
                policy(dir.path(), false),
                engine.clone(),
                feed.clone(),
                None,
                Instant::now(),
            )
            .unwrap();
        feed.speak(400);
        std::thread::sleep(Duration::from_millis(50));
        let ended = session.stop();
        match outcome {
            Ok(text) => assert_eq!(ended.unwrap().text, text),
            Err(e) => assert_eq!(ended.unwrap_err(), e),
        }
        assert_eq!(vad.whole.load(Ordering::Relaxed), 1);
    }
}

/// A voice pass that numbers its streams and logs when each piece starts
/// and ends; a piece takes 300 ms.
#[derive(Default)]
struct Logged {
    streams: AtomicUsize,
    log: Arc<Mutex<Vec<(usize, &'static str)>>>,
}

struct LoggedStream {
    id: usize,
    log: Arc<Mutex<Vec<(usize, &'static str)>>>,
}

impl VoiceDetector for Logged {
    fn speech_ms(&self, _: &[i16]) -> Result<u64, EngineError> {
        Ok(600)
    }

    fn stream(&self) -> Option<Box<dyn SpeechStream>> {
        let id = self.streams.fetch_add(1, Ordering::Relaxed);
        self.log.lock().unwrap().push((id, "stream"));
        Some(Box::new(LoggedStream {
            id,
            log: self.log.clone(),
        }))
    }
}

impl SpeechStream for LoggedStream {
    fn feed(&mut self, _: &[i16]) -> Result<(), EngineError> {
        self.log.lock().unwrap().push((self.id, "piece"));
        std::thread::sleep(Duration::from_millis(300));
        self.log.lock().unwrap().push((self.id, "piece done"));
        Ok(())
    }

    fn finish(&mut self, _: &[i16]) -> Result<u64, EngineError> {
        self.log.lock().unwrap().push((self.id, "finish"));
        Ok(600)
    }
}

/// The detector's process holds one take's state, so a cancelled take's
/// check never runs a piece once the next take can start.
#[test]
fn a_cancelled_take_check_ends_before_the_next_take_starts() {
    let (dir, _rec, session, engine, feed) = rig("ok");
    let vad = Arc::new(Logged::default());
    session.set_vad(Some(vad.clone()));
    session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            feed.clone(),
            None,
            Instant::now(),
        )
        .unwrap();
    feed.speak(400);
    std::thread::sleep(Duration::from_millis(50));
    session.cancel().unwrap();
    support::wait_idle(&session);
    session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            feed.clone(),
            None,
            Instant::now(),
        )
        .unwrap();
    feed.speak(400);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(session.stop().unwrap().text, "Ok");
    let log = vad.log.lock().unwrap().clone();
    let first_of_next = log.iter().position(|(id, _)| *id == 1).unwrap();
    assert!(
        log[first_of_next..].iter().all(|(id, _)| *id == 1),
        "the cancelled take's check ran into the next take: {log:?}"
    );
}

/// The take after a lost device starts only once the lost take's check
/// ended, so its piece never lands in the next take's stream.
#[test]
fn a_lost_device_take_check_ends_before_the_next_take_starts() {
    let (dir, _rec, session, engine, feed) = rig("ok");
    let vad = Arc::new(Logged::default());
    session.set_vad(Some(vad.clone()));
    session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            feed.clone(),
            None,
            Instant::now(),
        )
        .unwrap();
    feed.speak(400);
    std::thread::sleep(Duration::from_millis(50));
    feed.send(Event::Ended {
        reason: dettivo_audio::EndReason::DeviceLost("gone".into()),
    });
    support::wait_idle(&session);
    session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            feed.clone(),
            None,
            Instant::now(),
        )
        .unwrap();
    feed.speak(400);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(session.stop().unwrap().text, "Ok");
    let log = vad.log.lock().unwrap().clone();
    let first_of_next = log.iter().position(|(id, _)| *id == 1).unwrap();
    assert!(
        log[first_of_next..].iter().all(|(id, _)| *id == 1),
        "the lost take's check ran into the next take: {log:?}"
    );
}

/// ADR 0093: a press under the minimum hold reaches no voice pass, even
/// when the detector streams.
#[test]
fn a_tap_never_reaches_the_speech_check() {
    for discard in [false, true] {
        let (dir, _rec, session, engine, feed) = rig("never used");
        let vad = Arc::new(Logged::default());
        session.set_vad(Some(vad.clone()));
        let mut p = policy(dir.path(), false);
        p.min_hold = Duration::from_millis(300);
        let pressed = Instant::now();
        let job = session
            .start(p, engine.clone(), feed.clone(), None, pressed)
            .unwrap()
            .job_id;
        feed.speak(100);
        let released = pressed + Duration::from_millis(120);
        std::thread::sleep(released.saturating_duration_since(Instant::now()));
        if discard {
            session.discard_tap(&job).unwrap();
        } else {
            assert_eq!(
                session.stop_expected(None, released).unwrap_err(),
                SessionError::Cancelled
            );
        }
        assert!(
            vad.log.lock().unwrap().is_empty(),
            "the tap reached the detector: {:?}",
            vad.log.lock().unwrap()
        );
        assert!(
            pressed.elapsed() < Duration::from_millis(300),
            "the tap waited on a check piece: {:?}",
            pressed.elapsed()
        );
    }
}

/// The audio captured before the press qualified as a take reaches the
/// check once it does, ahead of the rest and in order.
#[test]
fn the_check_hears_the_audio_from_before_the_minimum_hold() {
    let (dir, _rec, session, engine, feed) = rig("ok");
    let vad = heard(Duration::ZERO, false, 900);
    session.set_vad(Some(vad.clone()));
    let mut p = policy(dir.path(), false);
    p.min_hold = Duration::from_millis(300);
    session
        .start(p, engine.clone(), feed.clone(), None, Instant::now())
        .unwrap();
    let chunks = take();
    let (early, late) = chunks[..40].split_at(20);
    for chunk in early {
        feed.send(Event::pcm(chunk.clone()));
    }
    std::thread::sleep(Duration::from_millis(150));
    assert!(
        vad.samples.lock().unwrap().is_empty(),
        "the check ran before the minimum hold"
    );
    std::thread::sleep(Duration::from_millis(250));
    for chunk in late {
        feed.send(Event::pcm(chunk.clone()));
    }
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(session.stop().unwrap().text, "Ok");
    assert_eq!(
        *vad.samples.lock().unwrap(),
        chunks[..40].concat(),
        "the check heard the whole take, in order"
    );
    assert_eq!(vad.whole.load(Ordering::Relaxed), 0, "no whole-take pass");
}
