//! A stop acknowledges the final accepted PCM; silence in the queue is not an end.

mod support;

use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use dettivo_audio::{EndReason, Event};
use dettivo_session::SessionError;
use dettivo_session::source::{Source, SourceFactory};
use support::{policy, rig};

struct StopSource {
    acknowledge: bool,
}

impl SourceFactory for StopSource {
    fn open(&self, _: u64) -> Result<Source, String> {
        let (tx, events) = mpsc::channel();
        let acknowledge = self.acknowledge;
        Ok(Source::Channel {
            events,
            on_stop: Box::new(move || {
                if acknowledge {
                    let tx = tx.clone();
                    std::thread::spawn(move || {
                        let _ = tx.send(Event::Pcm(vec![3000; 320]));
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
            Arc::new(StopSource { acknowledge: true }),
            None,
        )
        .unwrap();
    let result = session.stop().unwrap();
    assert_eq!(result.duration_ms, 20);
    assert_eq!(result.text, "Last words");
    assert!(!result.silent);
}

#[test]
fn a_source_that_never_acknowledges_stop_fails_within_a_bound() {
    let (dir, _, session, engine, _) = rig("uncommitted");
    session
        .start(
            policy(dir.path(), false),
            engine,
            Arc::new(StopSource { acknowledge: false }),
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
