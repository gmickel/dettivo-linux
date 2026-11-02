//! fn-90 through the session (ADR 0097): the speech check runs while the
//! take is captured and never holds the capture up. A check far slower
//! than the capture still leaves every sample in the take, and what it
//! heard plus what the release added is the take, in order (R3). A check
//! that fails leaves the release to judge the whole take (R1).

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
