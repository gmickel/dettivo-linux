//! fn-81: a take runs from the pre-roll before its press to the tail
//! after its release, and a tap never becomes a take (ADR 0092). The
//! microphones here deliver 20 ms chunks at real time.

mod support;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dettivo_audio::{EndReason, Event};
use dettivo_session::ready::{PRE_ROLL, ReadyFeed};
use dettivo_session::source::{Source, SourceFactory};
use dettivo_session::{Archive, POST_RELEASE_TAIL, SessionError, State, TAP, Transcript};
use support::{policy, rig, silence_reason, tone};

/// A microphone that speaks (a tone) from `speech.0` to `speech.1` ms
/// after `t0` and is silent otherwise, chunk `i` arriving at
/// `t0 + 20 ms × (i + 1)`, until its stop or `until_ms`.
fn microphone(t0: Instant, speech: (u64, u64), until_ms: u64) -> Source {
    let (tx, events) = mpsc::channel::<Event>();
    let stopped = Arc::new(AtomicBool::new(false));
    let (flag, ack) = (stopped.clone(), tx.clone());
    std::thread::spawn(move || {
        for i in 0..until_ms / 20 {
            let due = t0 + Duration::from_millis(20 * (i + 1));
            std::thread::sleep(due.saturating_duration_since(Instant::now()));
            if flag.load(Ordering::SeqCst) {
                return;
            }
            let ms = 20 * i;
            let pcm = if ms >= speech.0 && ms < speech.1 {
                tone(320)
            } else {
                vec![0; 320]
            };
            if tx.send(Event::Pcm(pcm)).is_err() {
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

/// Opens one prepared source.
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

/// Keeps the samples of every take file the archive is handed.
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

fn sleep_until(t: Instant) {
    std::thread::sleep(t.saturating_duration_since(Instant::now()));
}

fn leading_silence(samples: &[i16]) -> usize {
    samples.iter().take_while(|&&s| s == 0).count()
}

/// R1 and R5: speech that began 100 ms before the press is in the take,
/// which starts 450 ms before the press; the file written is that window
/// alone, and the ring wrote nothing while it filled.
#[test]
fn a_take_from_the_ring_keeps_the_first_word_and_writes_only_its_window() {
    let (dir, _, mut session, engine, _) = rig("first word");
    let takes = Arc::new(Takes::default());
    session.set_archive(takes.clone());
    let t0 = Instant::now();
    let feed = ReadyFeed::open(microphone(t0, (1_000, 5_000), 5_000)).unwrap();
    let pressed = t0 + Duration::from_millis(1_100);
    sleep_until(pressed);
    assert!(
        !dir.path().join("sessions").exists(),
        "the ring writes nothing"
    );
    let take = feed.take(pressed - PRE_ROLL).expect("the feed is idle");
    session
        .start(
            policy(dir.path(), true),
            engine,
            Arc::new(Once(Mutex::new(Some(take)))),
            None,
            pressed,
        )
        .unwrap();
    let released = pressed + Duration::from_millis(500);
    sleep_until(released);
    assert_eq!(
        session.stop_expected(None, released).unwrap().text,
        "First word"
    );
    feed.close();
    let written = takes.0.lock().unwrap().pop().expect("one take file");
    let silent_ms = leading_silence(&written) / 16;
    assert!(
        (330..=370).contains(&silent_ms),
        "the take opens {silent_ms} ms before the speech that began 100 ms before the press"
    );
    let ms = written.len() / 16;
    assert!(
        (930..=1_000).contains(&ms),
        "{ms} ms: the pre-roll and the hold, never the ring's whole second"
    );
}

/// R2: a release 150 ms before the last word ends keeps the word, and
/// the tail stops within a chunk of its 200 ms.
#[test]
fn the_release_tail_keeps_the_last_word() {
    let (dir, _, mut session, engine, _) = rig("last word");
    let takes = Arc::new(Takes::default());
    session.set_archive(takes.clone());
    let t0 = Instant::now();
    let source = microphone(t0, (0, 840), 3_000);
    let mut p = policy(dir.path(), true);
    p.release_tail = POST_RELEASE_TAIL;
    session
        .start(
            p,
            engine,
            Arc::new(Once(Mutex::new(Some(source)))),
            None,
            t0,
        )
        .unwrap();
    let released = t0 + Duration::from_millis(690);
    sleep_until(released);
    session.stop_expected(None, released).unwrap();
    let written = takes.0.lock().unwrap().pop().expect("one take file");
    let speech_ms = written.iter().filter(|&&s| s != 0).count() / 16;
    assert_eq!(speech_ms, 840, "the whole last word");
    let ms = written.len() / 16;
    assert!((880..=940).contains(&ms), "{ms} ms: the hold and its tail");
}

/// R3: a release under the minimum hold is a tap: no engine, no take
/// kept, `cancelled` with reason `tap`, by `stop` and by the hold key's
/// discard alike.
#[test]
fn a_press_shorter_than_the_minimum_hold_is_a_tap() {
    for discard in [false, true] {
        let (dir, rec, mut session, engine, feed) = rig("never used");
        let takes = Arc::new(Takes::default());
        session.set_archive(takes.clone());
        let mut p = policy(dir.path(), true);
        p.min_hold = Duration::from_millis(300);
        let pressed = Instant::now();
        let job = session
            .start(p, engine.clone(), feed.clone(), None, pressed)
            .unwrap()
            .job_id;
        feed.speak(100);
        let released = pressed + Duration::from_millis(120);
        sleep_until(released);
        if discard {
            session.discard_tap(&job).unwrap();
        } else {
            assert_eq!(
                session.stop_expected(None, released).unwrap_err(),
                SessionError::Cancelled
            );
        }
        assert_eq!(engine.calls.load(Ordering::Relaxed), 0);
        assert!(takes.0.lock().unwrap().is_empty());
        assert!(session.last().is_none());
        assert_eq!(silence_reason(&rec).as_deref(), Some(TAP));
        assert_eq!(
            rec.sequence(),
            [State::Recording, State::Cancelled, State::Idle]
        );
        assert!(!dir.path().join("sessions").join(&job).exists());
    }
}

/// A hold of the minimum or longer is a take.
#[test]
fn a_hold_of_the_minimum_is_a_take() {
    let (dir, _, session, engine, feed) = rig("held");
    let mut p = policy(dir.path(), false);
    p.min_hold = Duration::from_millis(300);
    let pressed = Instant::now() - Duration::from_millis(300);
    session
        .start(p, engine, feed.clone(), None, pressed)
        .unwrap();
    feed.speak(200);
    assert_eq!(
        session
            .stop_expected(None, pressed + Duration::from_millis(300))
            .unwrap()
            .text,
        "Held"
    );
}
