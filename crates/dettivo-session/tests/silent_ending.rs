//! fn-93: a take whose last stretch before the release holds no speech by
//! the gate's rules ends at the release; one that ends on speech keeps the
//! whole 200 ms tail (ADR 0093). The microphones deliver 20 ms chunks at
//! real time: a tone is speech, digital silence fails the level stage, and
//! a hum passes the level stage, so only the voice check can call it silent.

mod support;

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dettivo_audio::{EndReason, Event};
use dettivo_session::source::{Source, SourceFactory};
use dettivo_session::{Archive, POST_RELEASE_TAIL, Transcript};
use dettivo_speech::EngineError;
use dettivo_speech::gate::{SpeechStream, VAD_WINDOW, VoiceDetector};
use support::{policy, rig, tone};

const RELEASE_MS: u64 = 1_000;

/// What the microphone captures between the speech.
#[derive(Clone, Copy)]
enum Floor {
    /// Digital silence: under the level stage's floor.
    Silent,
    /// A hum at about -44 dBFS: over it.
    Hum,
}

/// Speech (a tone) from `speech.0` to `speech.1` ms after `t0`, the floor
/// otherwise; chunk `i` arrives at `t0 + 20 ms × (i + 1)` until the stop.
fn microphone(t0: Instant, speech: (u64, u64), floor: Floor) -> Source {
    let (tx, events) = mpsc::channel::<Event>();
    let stopped = Arc::new(AtomicBool::new(false));
    let (flag, ack) = (stopped.clone(), tx.clone());
    std::thread::spawn(move || {
        for i in 0..150 {
            let due = t0 + Duration::from_millis(20 * (i + 1));
            std::thread::sleep(due.saturating_duration_since(Instant::now()));
            if flag.load(Ordering::SeqCst) {
                return;
            }
            let ms = 20 * i;
            let pcm = match floor {
                _ if ms >= speech.0 && ms < speech.1 => tone(320),
                Floor::Silent => vec![0; 320],
                Floor::Hum => (0..320)
                    .map(|i| if i % 16 < 8 { 200 } else { -200 })
                    .collect(),
            };
            if tx.send(Event::pcm(pcm)).is_err() {
                return;
            }
        }
    });
    Source::Channel {
        events,
        on_stop: Arc::new(move || {
            if !stopped.swap(true, Ordering::SeqCst) {
                let _ = ack.send(Event::Ended {
                    reason: EndReason::Stopped,
                });
            }
        }),
    }
}

struct Once(Mutex<Option<Source>>);

impl SourceFactory for Once {
    fn open(&self, _: u64) -> Result<Source, String> {
        self.0
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| "opened twice".into())
    }
}

#[derive(Default)]
struct Takes(Mutex<Vec<Vec<i16>>>);

impl Archive for Takes {
    fn archive(&self, _: &Transcript, take_dir: &Path) -> Result<(), String> {
        let samples = dettivo_audio::mock::read_fixture(&take_dir.join("microphone.wav"))
            .map_err(|e| e.to_string())?;
        self.0.lock().unwrap().push(samples);
        Ok(())
    }
}

/// A voice check that hears the tone as voice: it answers where the last
/// window holding a tone sample ends, or nothing when `mute`.
struct Voiced {
    mute: bool,
    whole: Arc<AtomicUsize>,
}

struct VoicedStream {
    mute: bool,
    samples: Vec<i16>,
}

impl VoiceDetector for Voiced {
    fn speech_ms(&self, _: &[i16]) -> Result<u64, EngineError> {
        self.whole.fetch_add(1, Ordering::Relaxed);
        Ok(500)
    }

    fn stream(&self) -> Option<Box<dyn SpeechStream>> {
        Some(Box::new(VoicedStream {
            mute: self.mute,
            samples: Vec::new(),
        }))
    }
}

impl SpeechStream for VoicedStream {
    fn feed(&mut self, pcm: &[i16]) -> Result<(), EngineError> {
        self.samples.extend_from_slice(pcm);
        Ok(())
    }

    fn feed_voiced(&mut self, pcm: &[i16]) -> Result<Option<u64>, EngineError> {
        self.feed(pcm)?;
        let last = self.samples.iter().rposition(|s| s.unsigned_abs() > 1_000);
        let voiced = last.map_or(0, |i| (i / VAD_WINDOW + 1) * VAD_WINDOW);
        Ok((!self.mute).then_some(voiced as u64))
    }

    fn finish(&mut self, _: &[i16]) -> Result<u64, EngineError> {
        Ok(500)
    }
}

/// What one take kept.
struct Take {
    /// Its length, in ms.
    ms: usize,
    /// The ms of speech (tone) in it.
    speech_ms: usize,
    capture_ms: u64,
}

/// One take released at [`RELEASE_MS`] with the daemon's tail.
fn take(speech: (u64, u64), floor: Floor, vad: Option<Arc<dyn VoiceDetector>>) -> Take {
    let (dir, rec, mut session, engine, _) = rig("done");
    let takes = Arc::new(Takes::default());
    session.set_archive(takes.clone());
    session.set_vad(vad);
    let t0 = Instant::now();
    let mut p = policy(dir.path(), true);
    p.release_tail = POST_RELEASE_TAIL;
    let source = microphone(t0, speech, floor);
    session
        .start(
            p,
            engine,
            Arc::new(Once(Mutex::new(Some(source)))),
            None,
            t0,
        )
        .unwrap();
    let released = t0 + Duration::from_millis(RELEASE_MS);
    std::thread::sleep(released.saturating_duration_since(Instant::now()));
    assert_eq!(session.stop_expected(None, released).unwrap().text, "Done");
    let written = takes.0.lock().unwrap().pop().expect("one take file");
    Take {
        ms: written.len() / 16,
        speech_ms: written.iter().filter(|s| s.unsigned_abs() > 1_000).count() / 16,
        capture_ms: rec.completion_timings().unwrap().capture_ms,
    }
}

fn voiced(mute: bool) -> (Arc<AtomicUsize>, Arc<dyn VoiceDetector>) {
    let whole = Arc::new(AtomicUsize::new(0));
    let vad = Arc::new(Voiced {
        mute,
        whole: whole.clone(),
    });
    (whole, vad)
}

fn assert_tail(t: &Take, what: &str) {
    assert!(
        (1_180..=1_240).contains(&t.ms),
        "{what}: {} ms, the hold and the whole 200 ms tail",
        t.ms
    );
    assert!(t.capture_ms >= 190, "{what}: capture {} ms", t.capture_ms);
}

fn assert_early(t: &Take, what: &str) {
    assert!(
        (1_000..=1_060).contains(&t.ms),
        "{what}: {} ms, ending at the release",
        t.ms
    );
    assert!(t.capture_ms < 120, "{what}: capture {} ms", t.capture_ms);
}

/// R1: a take whose last stretch is under the silence floor ends at the
/// release; one that ends on speech keeps the whole tail.
#[test]
fn a_silent_ending_ends_at_the_release_and_a_spoken_one_keeps_its_tail() {
    let silent = take((0, 600), Floor::Silent, None);
    assert_early(&silent, "silent ending");
    assert_eq!(silent.speech_ms, 600);
    let spoken = take((0, 3_000), Floor::Silent, None);
    assert_tail(&spoken, "speech ending");
    assert_eq!(spoken.speech_ms, spoken.ms, "speech to the tail's end");
}

/// R1: over a hum the level stage hears sound, so the voice check decides:
/// no voice in the last stretch ends the take at the release without a
/// whole-take pass, a voice keeps the tail, and a check that cannot say
/// keeps it too, as does a detector that only judges whole takes.
#[test]
fn over_a_hum_the_voice_check_decides() {
    let (whole, vad) = voiced(false);
    assert_early(
        &take((0, 600), Floor::Hum, Some(vad)),
        "no voice at the end",
    );
    assert_eq!(
        whole.load(Ordering::Relaxed),
        0,
        "the check heard the whole take"
    );
    let (_, vad) = voiced(false);
    assert_tail(
        &take((0, 940), Floor::Hum, Some(vad)),
        "voice 60 ms before the release",
    );
    let (_, vad) = voiced(true);
    assert_tail(
        &take((0, 600), Floor::Hum, Some(vad)),
        "a check that cannot say",
    );
    assert_tail(&take((0, 600), Floor::Hum, None), "no voice check");
}

/// R2: a last word ending 60 to 140 ms before the release (in whole
/// chunks) keeps every sample and the whole tail, by the level stage and
/// by the voice check alike: the 150 ms are counted back from the release,
/// never from the window boundary after it.
#[test]
fn a_last_word_ending_just_before_the_release_is_kept() {
    for before in [60, 100, 140] {
        let word = RELEASE_MS - before;
        for floor in [Floor::Silent, Floor::Hum] {
            let (_, vad) = voiced(false);
            let t = take((0, word), floor, Some(vad));
            assert_eq!(
                t.speech_ms as u64, word,
                "{before} ms before: the whole word"
            );
            assert_tail(&t, &format!("a word ending {before} ms before"));
        }
    }
}
