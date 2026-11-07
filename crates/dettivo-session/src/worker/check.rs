//! The speech check during the take (ADR 0097): a thread feeds the voice
//! detector's stream each chunk as it is captured, so at the release only
//! the samples it has not run remain. The recording loop only sends to it
//! and never waits; a check that falls behind runs the backlog in one
//! piece, and one that failed leaves the take to the whole-take pass.
//! At the release the loop can also ask where the take's last voiced window
//! ends (ADR 0093); the answer comes back on a channel it polls.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

use dettivo_speech::gate::{SAMPLE_RATE, SpeechStream, VAD_WINDOW, loudest_window_dbfs};

/// What the check thread ran.
struct Fed {
    stream: Box<dyn SpeechStream>,
    /// The take's first samples the stream holds.
    samples: usize,
    /// Every feed succeeded.
    ok: bool,
}

/// The next samples for the check, and where to send the voiced end of
/// the take once they ran, when the loop asked for it.
type Piece = (Vec<i16>, Option<Sender<Option<u64>>>);

/// One take's check, running beside the capture.
pub(crate) struct SpeechCheck {
    tx: Option<Sender<Piece>>,
    /// Set at the release: the thread stops after the piece it is running.
    finishing: Arc<AtomicBool>,
    thread: Option<JoinHandle<Fed>>,
}

impl SpeechCheck {
    /// Starts the thread that feeds `stream`.
    pub(crate) fn start(mut stream: Box<dyn SpeechStream>) -> Self {
        let (tx, rx) = mpsc::channel::<Piece>();
        let finishing = Arc::new(AtomicBool::new(false));
        let stop = finishing.clone();
        let thread = std::thread::Builder::new()
            .name("speech-check".into())
            .spawn(move || {
                let (mut samples, mut ok) = (0, true);
                while let Ok((mut pcm, mut ask)) = rx.recv() {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    // A backlog runs in one piece, up to a question.
                    while ask.is_none()
                        && let Ok((more, asked)) = rx.try_recv()
                    {
                        pcm.extend_from_slice(&more);
                        ask = asked;
                    }
                    let fed = match &ask {
                        Some(reply) => stream.feed_voiced(&pcm).map(|v| {
                            let _ = reply.send(v);
                        }),
                        None => stream.feed(&pcm),
                    };
                    if let Err(e) = fed {
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
            let _ = tx.send((pcm.to_vec(), None));
        }
    }

    /// Queues the take's last samples before the release and asks where
    /// its last voiced window ends. The answer arrives once the check ran
    /// everything queued; a check that failed or cannot say drops it.
    pub(crate) fn ask_voiced(&self, pcm: &[i16]) -> Receiver<Option<u64>> {
        let (reply, answer) = mpsc::channel();
        if let Some(tx) = &self.tx {
            let _ = tx.send((pcm.to_vec(), Some(reply)));
        }
        answer
    }

    /// The milliseconds of speech in `take`, the final take, from what the
    /// check ran and the rest; `None` when the check cannot answer (it
    /// failed, or ran samples the take was cut before). With `voiced` (the
    /// take's silence floor), also where the take's last voiced window ends,
    /// in samples, when the stream can say (the rounds' release, ADR 0102).
    /// The stream names whole windows only, so a take whose last window
    /// reaches the floor counts as voiced to its end.
    pub(crate) fn finish(
        mut self,
        take: &[i16],
        voiced: Option<f32>,
    ) -> Option<(u64, Option<u64>)> {
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
        let finished = if let Some(floor) = voiced {
            let last = &take[take.len().saturating_sub(VAD_WINDOW)..];
            let sounding = loudest_window_dbfs(last, SAMPLE_RATE).is_some_and(|db| db >= floor);
            fed.stream.feed_voiced(rest).and_then(|v| {
                let v = if sounding { Some(take.len() as u64) } else { v };
                Ok((fed.stream.finish(&[])?, v))
            })
        } else {
            fed.stream.finish(rest).map(|ms| (ms, None))
        };
        finished
            .inspect_err(|e| tracing::warn!(error = %e, "speech check failed; the release judges the whole take"))
            .ok()
    }
}

impl Drop for SpeechCheck {
    /// A take that ends without a verdict waits for the piece the thread is
    /// running: the voice process holds one take's state, so a piece left
    /// running would land in the next take's.
    fn drop(&mut self) {
        self.finishing.store(true, Ordering::Relaxed);
        self.tx = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
