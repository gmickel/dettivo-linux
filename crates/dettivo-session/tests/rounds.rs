//! fn-92 through the session (ADR 0102): a Parakeet take on a GPU is
//! decoded in rounds while the key is held, and the release decodes only
//! its tail. Capture never waits on a round and every sample reaches the
//! take and the rounds, in order (R4); a take without rounds is decoded
//! whole, as before (R3).

mod support;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dettivo_audio::Event;
use dettivo_engine_proto::{Backend, RecognizeResult, Segment, Word};
use dettivo_session::{ReleaseDecode, SessionTimings};
use dettivo_speech::{Capabilities, EngineError, PreloadSource, RecognizeRequest, SttEngine};
use support::{policy, rig};

/// A Parakeet engine on `backend` that takes `slow` per request, keeps
/// every request's samples, and names each second of a clip by the chunk
/// it came from: `s<n>` for the take's n-th second.
struct Rounds {
    slow: Duration,
    backend: Backend,
    fail: AtomicBool,
    /// A padded request that starts after the take's start (the release's
    /// tail) comes back empty, as Parakeet v3 does for some (ADR 0081).
    empty_padded_tail: AtomicBool,
    requests: Mutex<Vec<Vec<i16>>>,
}

fn engine(slow: Duration, backend: Backend) -> Arc<Rounds> {
    Arc::new(Rounds {
        slow,
        backend,
        fail: AtomicBool::new(false),
        empty_padded_tail: AtomicBool::new(false),
        requests: Mutex::default(),
    })
}

/// The chunk a sample came from (see [`take`]).
fn chunk_of(sample: i16) -> u64 {
    (sample.unsigned_abs() as u64).saturating_sub(2_000)
}

impl SttEngine for Rounds {
    fn provider(&self) -> &str {
        "parakeet"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_timestamps: true,
            supports_streaming: false,
            supported_languages: vec!["en".into()],
            max_duration_seconds: 300,
            supports_custom_vocabulary: false,
            supports_model_deletion: true,
        }
    }
    fn preload(&self, _: PreloadSource) -> Result<(), EngineError> {
        Ok(())
    }
    fn recognize(
        &self,
        request: RecognizeRequest,
        _: Duration,
    ) -> Result<RecognizeResult, EngineError> {
        std::thread::sleep(self.slow);
        self.requests.lock().unwrap().push(request.pcm.clone());
        if self.fail.load(Ordering::Relaxed) {
            return Err(EngineError::Busy);
        }
        let pcm = &request.pcm;
        if self.empty_padded_tail.load(Ordering::Relaxed)
            && chunk_of(pcm[0]) > 0
            && pcm.last() == Some(&0)
        {
            return Ok(RecognizeResult {
                text: String::new(),
                language: request.language,
                segments: Vec::new(),
                duration_ms: pcm.len() as u64 / 16,
                backend: self.backend,
            });
        }
        // A word in the middle of every whole second that holds audio.
        let words: Vec<Word> = (0..pcm.len() / 16_000)
            .filter(|s| pcm[s * 16_000 + 8_000] != 0)
            .map(|s| {
                let second = chunk_of(pcm[s * 16_000 + 8_000]) / 50;
                Word {
                    start_ms: s as u64 * 1000 + 300,
                    end_ms: s as u64 * 1000 + 700,
                    text: format!("s{second}"),
                    confidence: 1.0,
                }
            })
            .collect();
        let text = words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        Ok(RecognizeResult {
            text: text.clone(),
            language: request.language,
            segments: vec![Segment {
                start_ms: 0,
                end_ms: pcm.len() as u64 / 16,
                text,
                words,
            }],
            duration_ms: pcm.len() as u64 / 16,
            backend: self.backend,
        })
    }
    fn cancel(&self) {}
    fn backend(&self) -> Option<Backend> {
        Some(self.backend)
    }
}

/// `seconds` of 20 ms chunks, each one distinct, so order is checked:
/// chunk c holds the level 2000 + c.
fn take(seconds: usize) -> Vec<Vec<i16>> {
    (0..seconds * 50)
        .map(|c| {
            let level = 2_000 + c as i16;
            (0..320)
                .map(|i| if i % 16 < 8 { level } else { -level })
                .collect()
        })
        .collect()
}

/// The text the raw pipeline makes of `s0 s1 ...`: its first letter capitalised.
fn words(seconds: usize) -> String {
    let text = (0..seconds)
        .map(|s| format!("s{s}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!("S{}", &text[1..])
}

fn timings(rec: &support::Recorder) -> SessionTimings {
    rec.completion_timings().expect("a completed take")
}

/// The samples a request carried, without the silence padding added.
fn unpadded(pcm: &[i16]) -> &[i16] {
    let end = pcm.iter().rposition(|s| *s != 0).map_or(0, |i| i + 1);
    &pcm[..end]
}

#[test]
fn capture_never_waits_on_a_round_and_drops_no_sample() {
    let (dir, rec, session, _, feed) = rig("unused");
    // A round takes 700 ms, longer than the audio fed while it runs.
    let engine = engine(Duration::from_millis(700), Backend::Vulkan);
    session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            feed.clone(),
            None,
            Instant::now(),
        )
        .unwrap();
    let chunks = take(12);
    let all: Vec<i16> = chunks.concat();
    // Four seconds at once, then the rest while the first round runs.
    let fed = Instant::now();
    for chunk in &chunks[..200] {
        feed.send(Event::pcm(chunk.clone()));
    }
    std::thread::sleep(Duration::from_millis(1_700));
    for chunk in &chunks[200..] {
        feed.send(Event::pcm(chunk.clone()));
    }
    assert!(
        fed.elapsed() < Duration::from_millis(1_900),
        "capture never waited"
    );
    std::thread::sleep(Duration::from_millis(1_600));
    let text = session.stop().unwrap().text;
    assert_eq!(text, words(12), "every second once, in order");
    let requests = engine.requests.lock().unwrap();
    let sizes: Vec<usize> = requests.iter().map(|r| unpadded(r).len()).collect();
    assert!(requests.len() >= 3, "two rounds and the tail: {sizes:?}");
    // Each round decoded the take from its start, in order, with every
    // sample captured by then; the last one held all of them.
    for round in &requests[..requests.len() - 1] {
        let heard = unpadded(round);
        assert_eq!(heard, &all[..heard.len()]);
    }
    assert_eq!(unpadded(&requests[requests.len() - 2]), &all[..]);
    // The release decoded the tail only.
    let tail = unpadded(&requests[requests.len() - 1]);
    assert!(tail.len() < all.len(), "{sizes:?}");
    assert_eq!(tail, &all[all.len() - tail.len()..]);
    assert_eq!(timings(&rec).release_decode, Some(ReleaseDecode::Tail));
}

/// A take with no rounds: on the CPU, shorter than one cadence, or with
/// every round failed. Each is decoded whole, in one request.
#[test]
fn a_take_without_rounds_is_decoded_whole() {
    for (backend, seconds, fail_rounds) in [
        (Backend::Cpu, 4, false),
        (Backend::Vulkan, 1, false),
        (Backend::Vulkan, 4, true),
    ] {
        let (dir, rec, session, _, feed) = rig("unused");
        let engine = engine(Duration::ZERO, backend);
        engine.fail.store(fail_rounds, Ordering::Relaxed);
        session
            .start(
                policy(dir.path(), false),
                engine.clone(),
                feed.clone(),
                None,
                Instant::now(),
            )
            .unwrap();
        let chunks = take(seconds);
        for chunk in &chunks {
            feed.send(Event::pcm(chunk.clone()));
        }
        std::thread::sleep(Duration::from_millis(1_000 * seconds as u64));
        let rounds = engine.requests.lock().unwrap().len();
        engine.fail.store(false, Ordering::Relaxed);
        let text = session.stop().unwrap().text;
        assert_eq!(text, words(seconds), "{backend:?} {seconds} s");
        let requests = engine.requests.lock().unwrap();
        assert_eq!(requests.len(), rounds + 1, "one release request");
        assert_eq!(unpadded(requests.last().unwrap()), &chunks.concat()[..]);
        // Rounds that all failed fell back, and History says so.
        let expected = fail_rounds.then_some(ReleaseDecode::Whole);
        assert_eq!(timings(&rec).release_decode, expected);
    }
}

/// An empty padded tail runs once more as captured, so the take's last
/// words are kept.
#[test]
fn an_empty_padded_tail_runs_once_more_as_captured() {
    let (dir, rec, session, _, feed) = rig("unused");
    let engine = engine(Duration::ZERO, Backend::Vulkan);
    engine.empty_padded_tail.store(true, Ordering::Relaxed);
    session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            feed.clone(),
            None,
            Instant::now(),
        )
        .unwrap();
    for chunk in &take(8) {
        feed.send(Event::pcm(chunk.clone()));
    }
    std::thread::sleep(Duration::from_millis(1_700));
    assert_eq!(session.stop().unwrap().text, words(8));
    let requests = engine.requests.lock().unwrap();
    let last = requests.last().unwrap();
    assert_ne!(last.last(), Some(&0), "the retry carries no padding");
    assert_eq!(timings(&rec).release_decode, Some(ReleaseDecode::Tail));
}
