//! The speech check during the take (ADR 0097): a thread feeds the voice
//! detector's stream each chunk as it is captured, so at the release only
//! the samples it has not run remain. The recording loop only sends to it
//! and never waits; a check that falls behind runs the backlog in one
//! piece, and one that failed leaves the take to the whole-take pass.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

use dettivo_speech::gate::SpeechStream;

/// What the check thread ran.
struct Fed {
    stream: Box<dyn SpeechStream>,
    /// The take's first samples the stream holds.
    samples: usize,
    /// Every feed succeeded.
    ok: bool,
}

/// One take's check, running beside the capture.
pub(crate) struct SpeechCheck {
    tx: Option<Sender<Vec<i16>>>,
    /// Set at the release: the thread stops after the piece it is running.
    finishing: Arc<AtomicBool>,
    thread: Option<JoinHandle<Fed>>,
}

impl SpeechCheck {
    /// Starts the thread that feeds `stream`.
    pub(crate) fn start(mut stream: Box<dyn SpeechStream>) -> Self {
        let (tx, rx) = mpsc::channel::<Vec<i16>>();
        let finishing = Arc::new(AtomicBool::new(false));
        let stop = finishing.clone();
        let thread = std::thread::Builder::new()
            .name("speech-check".into())
            .spawn(move || {
                let (mut samples, mut ok) = (0, true);
                while let Ok(mut pcm) = rx.recv() {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    while let Ok(more) = rx.try_recv() {
                        pcm.extend_from_slice(&more);
                    }
                    if let Err(e) = stream.feed(&pcm) {
                        tracing::warn!(error = %e, "speech check failed; the release judges the whole take");
                        ok = false;
                        break;
                    }
                    samples += pcm.len();
                }
                Fed {
                    stream,
                    samples,
                    ok,
                }
            })
            .inspect_err(|e| tracing::warn!(error = %e, "speech check thread did not start"))
            .ok();
        Self {
            tx: Some(tx),
            finishing,
            thread,
        }
    }

    /// Queues the take's next captured samples.
    pub(crate) fn feed(&self, pcm: &[i16]) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(pcm.to_vec());
        }
    }

    /// The milliseconds of speech in `take`, the final take, from what the
    /// check ran and the rest; `None` when the check cannot answer (it
    /// failed, or ran samples the take was cut before).
    pub(crate) fn finish(mut self, take: &[i16]) -> Option<u64> {
        self.finishing.store(true, Ordering::Relaxed);
        self.tx = None;
        let mut fed = self.thread.take()?.join().ok()?;
        if !fed.ok || fed.samples > take.len() {
            return None;
        }
        let rest = &take[fed.samples..];
        tracing::debug!(
            ran = fed.samples,
            rest = rest.len(),
            "speech check finishing"
        );
        fed.stream
            .finish(rest)
            .inspect_err(|e| tracing::warn!(error = %e, "speech check failed; the release judges the whole take"))
            .ok()
    }
}

impl Drop for SpeechCheck {
    fn drop(&mut self) {
        // A take that ends without a verdict leaves the thread to stop on
        // its own after the piece it is running.
        self.finishing.store(true, Ordering::Relaxed);
    }
}
