//! Rounds while the key is held (ADR 0102): a thread keeps a copy of the
//! take as it is captured and, at the cadence, decodes it from the context
//! before the commit point to the newest audio, through
//! `dettivo_transcribe::rounds`. The recording loop only sends to it and
//! never waits. A round that overruns the cadence skips the rounds it
//! missed. At the release no new round starts; the release waits for the
//! round already running, the engine being one process, and takes the
//! rounds as they stand.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use dettivo_speech::gate::{SAMPLE_RATE, pad_for_engine};
use dettivo_speech::{RecognizeRequest, SttEngine};
use dettivo_transcribe::rounds::{Rounds, WINDOWS, shifted};

/// What a round asks the engine: the take's request for `pcm`.
pub(crate) type Request = Box<dyn Fn(Vec<i16>) -> RecognizeRequest + Send>;

/// The rounds of a take once the release came.
pub(crate) struct Held {
    pub(crate) rounds: Rounds,
    /// The language and backend the last round reported.
    pub(crate) language: String,
    pub(crate) backend: Option<dettivo_engine_proto::Backend>,
    /// Rounds that ran, and rounds the cadence skipped or the engine failed.
    pub(crate) ran: usize,
    pub(crate) skipped: usize,
    /// How long the engine spent on rounds.
    pub(crate) busy: Duration,
    /// How long the release waited for the round running.
    pub(crate) waited: Duration,
}

/// One take's rounds, running beside the capture.
pub(crate) struct HeldRounds {
    tx: Option<Sender<Vec<i16>>>,
    /// Set at the release: no round starts after it.
    released: Arc<AtomicBool>,
    thread: Option<JoinHandle<Held>>,
}

fn ms(samples: usize) -> u64 {
    samples as u64 * 1000 / u64::from(SAMPLE_RATE)
}

impl HeldRounds {
    /// Starts the thread; the first round runs one cadence from now.
    pub(crate) fn start(engine: Arc<dyn SttEngine>, request: Request) -> Self {
        let (tx, rx) = mpsc::channel::<Vec<i16>>();
        let released = Arc::new(AtomicBool::new(false));
        let stop = released.clone();
        let parakeet = engine.provider() == "parakeet";
        let cadence = Duration::from_millis(WINDOWS.cadence_ms);
        let thread = std::thread::Builder::new()
            .name("dictation-rounds".into())
            .spawn(move || {
                let mut pcm: Vec<i16> = Vec::new();
                let mut held = Held {
                    rounds: Rounds::default(),
                    language: String::new(),
                    backend: None,
                    ran: 0,
                    skipped: 0,
                    busy: Duration::ZERO,
                    waited: Duration::ZERO,
                };
                let mut next = Instant::now() + cadence;
                loop {
                    match rx.recv_timeout(next.saturating_duration_since(Instant::now())) {
                        Ok(chunk) => pcm.extend_from_slice(&chunk),
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                    while let Ok(chunk) = rx.try_recv() {
                        pcm.extend_from_slice(&chunk);
                    }
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let now = Instant::now();
                    if now < next {
                        continue;
                    }
                    let (from, until) = held.rounds.window(&WINDOWS, ms(pcm.len()));
                    let start = (from as usize * SAMPLE_RATE as usize / 1000).min(pcm.len());
                    let clip = pad_for_engine(pcm[start..].to_vec(), parakeet, SAMPLE_RATE);
                    let padded = clip.len() != pcm.len() - start;
                    let mut recognized = engine.recognize(request(clip), Duration::from_secs(30));
                    // An empty padded round runs once more as captured, as
                    // a whole take does (ADR 0081).
                    if padded && recognized.as_ref().is_ok_and(|r| r.text.trim().is_empty()) {
                        recognized = engine
                            .recognize(request(pcm[start..].to_vec()), Duration::from_secs(30));
                    }
                    match recognized {
                        Ok(r) => {
                            let words = r.segments.into_iter().flat_map(|s| s.words);
                            held.rounds.absorb(&WINDOWS, until, shifted(words, from));
                            held.language = r.language;
                            held.backend = Some(r.backend);
                            held.ran += 1;
                        }
                        Err(e) => {
                            tracing::debug!(error = %e, "dictation round failed");
                            held.skipped += 1;
                        }
                    }
                    held.busy += now.elapsed();
                    // A round that overran skips the cadence slots it missed.
                    next += cadence;
                    while next <= Instant::now() {
                        next += cadence;
                        held.skipped += 1;
                    }
                }
                held
            })
            .inspect_err(|e| tracing::warn!(error = %e, "dictation rounds did not start"))
            .ok();
        Self {
            tx: Some(tx),
            released,
            thread,
        }
    }

    /// Queues the take's next captured samples.
    pub(crate) fn feed(&self, pcm: &[i16]) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(pcm.to_vec());
        }
    }

    /// The release came: no round starts after the one running.
    pub(crate) fn release(&self) {
        self.released.store(true, Ordering::Relaxed);
    }

    /// The rounds as they stand once the running round ended.
    pub(crate) fn finish(mut self) -> Option<Held> {
        self.release();
        self.tx = None;
        let at = Instant::now();
        let mut held = self.thread.take()?.join().ok()?;
        held.waited = at.elapsed();
        Some(held)
    }
}

impl Drop for HeldRounds {
    /// A take that ends without a release decode still waits for the round
    /// running, so the next take's requests never queue behind it unseen.
    fn drop(&mut self) {
        self.release();
        self.tx = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
