//! A stop acknowledges the final accepted PCM; silence in the queue is not
//! an end. The take runs through its release tail and ends at the source's
//! end; nothing the source sends after that joins it.

mod support;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use dettivo_audio::{EndReason, Event};
use dettivo_session::source::{Source, SourceFactory};
use dettivo_session::{SessionError, TAIL_STALL_GRACE};
use support::{policy, rig, tone};

/// Delivers `before` while recording and, when it acknowledges the stop,
/// one 20 ms chunk of speech captured before the release but delivered
/// while the stop drains (a PipeWire quantum in flight).
struct StopSource {
    acknowledge: bool,
    before: Vec<i16>,
}

impl SourceFactory for StopSource {
    fn open(&self, _: u64) -> Result<Source, String> {
        let (tx, events) = mpsc::channel();
        let acknowledge = self.acknowledge;
        if !self.before.is_empty() {
            let _ = tx.send(Event::pcm(self.before.clone()));
        }
        // The stop request and the worker both stop the source: only the
        // first acknowledges, as a real capture does.
        let stopped = AtomicBool::new(false);
        let in_flight = Instant::now();
        Ok(Source::Channel {
            events,
            on_stop: Arc::new(move || {
                if acknowledge && !stopped.swap(true, Ordering::SeqCst) {
                    let tx = tx.clone();
                    std::thread::spawn(move || {
                        let _ = tx.send(Event::Pcm {
                            pcm: tone(320),
                            at: in_flight,
                        });
                        let _ = tx.send(Event::Ended {
                            reason: EndReason::Stopped,
                        });
                    });
                }
            }),
        })
    }
}

#[test]
fn stop_includes_the_last_chunk_delivered_during_source_shutdown() {
    let (dir, _, session, engine, _) = rig("last words");
    session
        .start(
            policy(dir.path(), false),
            engine,
            Arc::new(StopSource {
                acknowledge: true,
                before: tone(1_600),
            }),
            None,
            Instant::now(),
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    let result = session.stop().unwrap();
    assert_eq!(result.duration_ms, 120);
    assert_eq!(result.text, "Last words");
}

/// Audio a source sends after its end never joins the take: the worker
/// stops the source once the release tail is covered, and the take ends
/// at the source's `Ended`.
#[test]
fn audio_a_source_sends_after_its_end_never_counts() {
    let (dir, _, session, engine, _) = rig("never used");
    let source = move |_: u64| {
        let (tx, events) = mpsc::channel();
        let _ = tx.send(Event::pcm(vec![0; 1_600]));
        Ok(Source::Channel {
            events,
            on_stop: Arc::new(move || {
                let _ = tx.send(Event::Ended {
                    reason: EndReason::Stopped,
                });
                let _ = tx.send(Event::pcm(tone(1_600)));
            }),
        })
    };
    session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            Arc::new(Opener(Box::new(source))),
            None,
            Instant::now(),
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(session.stop().unwrap_err(), SessionError::Cancelled);
    assert_eq!(engine.calls.load(std::sync::atomic::Ordering::Relaxed), 0);
}

/// A source factory from a closure.
struct Opener(Box<dyn Fn(u64) -> Result<Source, String> + Send + Sync>);

impl SourceFactory for Opener {
    fn open(&self, level_interval_ms: u64) -> Result<Source, String> {
        (self.0)(level_interval_ms)
    }
}

#[test]
fn a_source_that_never_acknowledges_stop_fails_within_a_bound() {
    let (dir, _, session, engine, _) = rig("uncommitted");
    session
        .start(
            policy(dir.path(), false),
            engine,
            Arc::new(StopSource {
                acknowledge: false,
                before: Vec::new(),
            }),
            None,
            Instant::now(),
        )
        .unwrap();
    let start = Instant::now();
    let error = session.stop().unwrap_err();
    assert!(
        matches!(error, SessionError::Failed(ref message) if message.contains("capture stop acknowledgement")),
        "{error:?}"
    );
    assert!(start.elapsed() < Duration::from_secs(5));
    assert!(session.snapshot().is_none());
    assert!(session.last().is_none());
}

/// fn-79 R1: speech the source delivered before the stop counts even when
/// the worker had not read it yet; nothing waits for the worker here.
#[test]
fn speech_queued_before_the_stop_opens_the_gate() {
    let (dir, _, session, engine, feed) = rig("queued words");
    session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            feed.clone(),
            None,
            Instant::now(),
        )
        .unwrap();
    feed.speak(200);
    let result = session.stop().unwrap();
    assert_eq!(result.text, "Queued words");
    assert_eq!(result.duration_ms, 200);
}

/// ADR 0092: a source that stops delivering during the release tail
/// closes the take after the stall grace, not at the tail's end, and a
/// sound after that never opens the gate.
#[test]
fn a_stalled_tail_closes_after_the_grace_and_a_later_sound_never_counts() {
    let (dir, _, session, engine, feed) = rig("never used");
    let mut p = policy(dir.path(), false);
    p.release_tail = Duration::from_secs(2);
    session
        .start(p, engine.clone(), feed.clone(), None, Instant::now())
        .unwrap();
    feed.send(Event::pcm(vec![0; 1_600]));
    std::thread::sleep(Duration::from_millis(20));
    let late = feed.clone();
    let thump = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        late.send(Event::pcm(tone(320)));
    });
    let released = Instant::now();
    assert_eq!(session.stop().unwrap_err(), SessionError::Cancelled);
    let waited = released.elapsed();
    assert!(
        waited >= TAIL_STALL_GRACE && waited < Duration::from_millis(350),
        "closed {waited:?} after the release"
    );
    thump.join().unwrap();
    assert_eq!(engine.calls.load(std::sync::atomic::Ordering::Relaxed), 0);
}
