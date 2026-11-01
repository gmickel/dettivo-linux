//! A stop acknowledges the final accepted PCM; silence in the queue is not an end.
//! What arrives after the release joins the take but never counts as speech.

mod support;

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
        Ok(Source::Channel {
            events,
            on_stop: Box::new(move || {
                if acknowledge {
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

/// fn-79 R1: a sound that only arrives after the release (the stop cue,
/// the key coming up) does not make a silent take speech.
#[test]
fn speech_after_the_release_never_opens_the_gate() {
    let (dir, _, session, engine, _) = rig("never used");
    session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            Arc::new(StopSource {
                acknowledge: true,
                before: vec![0; 1_600],
            }),
            None,
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(session.stop().unwrap_err(), SessionError::Cancelled);
    assert_eq!(engine.calls.load(std::sync::atomic::Ordering::Relaxed), 0);
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
