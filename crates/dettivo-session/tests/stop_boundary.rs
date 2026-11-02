//! A stop acknowledges the final accepted PCM; silence in the queue is not an end.
//! What arrives after the release joins the take but never counts as speech.

mod support;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use dettivo_audio::{EndReason, Event};
use dettivo_session::SessionError;
use dettivo_session::source::{Source, SourceFactory};
use support::{policy, rig, tone};

/// Delivers `before` while recording and one 20 ms chunk of speech while
/// the stop drains, when it acknowledges the stop.
struct StopSource {
    acknowledge: bool,
    before: Vec<i16>,
}

impl SourceFactory for StopSource {
    fn open(&self, _: u64) -> Result<Source, String> {
        let (tx, events) = mpsc::channel();
        let acknowledge = self.acknowledge;
        if !self.before.is_empty() {
            let _ = tx.send(Event::Pcm(self.before.clone()));
        }
        // The stop request and the worker both stop the source: only the
        // first acknowledges, as a real capture does.
        let stopped = AtomicBool::new(false);
        Ok(Source::Channel {
            events,
            on_stop: Arc::new(move || {
                if acknowledge && !stopped.swap(true, Ordering::SeqCst) {
                    let tx = tx.clone();
                    std::thread::spawn(move || {
                        let _ = tx.send(Event::Pcm(tone(320)));
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
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    let result = session.stop().unwrap();
    assert_eq!(result.duration_ms, 120);
    assert_eq!(result.text, "Last words");
}

/// fn-79 R1: the stop request ends the capture on the stopping thread, so
/// the take holds nothing recorded after the release, and audio a source
/// sends after its end never joins the take.
#[test]
fn the_capture_ends_at_the_stop_request_and_nothing_after_it_counts() {
    let (dir, _, session, engine, _) = rig("never used");
    let stopped_on = Arc::new(std::sync::Mutex::new(None));
    let seen = stopped_on.clone();
    let source = move |_: u64| {
        let (tx, events) = mpsc::channel();
        let _ = tx.send(Event::Pcm(vec![0; 1_600]));
        let seen = seen.clone();
        Ok(Source::Channel {
            events,
            on_stop: Arc::new(move || {
                seen.lock()
                    .unwrap()
                    .get_or_insert(std::thread::current().id());
                let _ = tx.send(Event::Ended {
                    reason: EndReason::Stopped,
                });
                let _ = tx.send(Event::Pcm(tone(1_600)));
            }),
        })
    };
    session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            Arc::new(Opener(Box::new(source))),
            None,
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(session.stop().unwrap_err(), SessionError::NoSpeech);
    assert_eq!(
        *stopped_on.lock().unwrap(),
        Some(std::thread::current().id())
    );
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
        )
        .unwrap();
    feed.speak(200);
    let result = session.stop().unwrap();
    assert_eq!(result.text, "Queued words");
    assert_eq!(result.duration_ms, 200);
}

/// fn-79 R1: a stop while the worker waits on an empty source, then a
/// key-up thump: the capture already ended, so the thump never opens the gate.
#[test]
fn a_sound_that_arrives_after_the_stop_while_the_worker_waits_is_tail() {
    let (dir, _, session, engine, feed) = rig("never used");
    session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            feed.clone(),
            None,
        )
        .unwrap();
    feed.send(Event::Pcm(vec![0; 1_600]));
    std::thread::sleep(Duration::from_millis(20));
    let late = feed.clone();
    let thump = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(10));
        late.send(Event::Pcm(tone(320)));
    });
    assert_eq!(session.stop().unwrap_err(), SessionError::NoSpeech);
    thump.join().unwrap();
    assert_eq!(engine.calls.load(std::sync::atomic::Ordering::Relaxed), 0);
}
