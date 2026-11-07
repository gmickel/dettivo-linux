//! The take's recognition at the release. A take the rounds decoded while
//! it was held (ADR 0102) decodes only its uncommitted tail, stitched to
//! the rounds by word timestamps, or nothing when only silence followed
//! the last round. Every other take is decoded whole, exactly as before:
//! padded for the engine, and once more as captured when padding emptied
//! a Parakeet take (ADR 0081).

use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use dettivo_engine_proto::RecognizeResult;
use dettivo_engine_proto::{Backend, Word};
use dettivo_speech::gate::{self, SAMPLE_RATE};
use dettivo_speech::{EngineError, RecognizeRequest, prompt_terms, vocabulary_prompt};
use dettivo_transcribe::rounds::{self, Release, WINDOWS};

use super::Worker;
use super::held::{Held, Request};
use crate::ReleaseDecode;
use dettivo_core::config::vocabulary_schema;

const TIMEOUT: Duration = Duration::from_secs(300);

fn ms(samples: usize) -> u64 {
    samples as u64 * 1000 / u64::from(SAMPLE_RATE)
}

/// Stitched words as one result, with the segments a whole decode has, so
/// the output guards judge the same pieces.
fn result_of(
    words: Vec<Word>,
    language: String,
    duration_ms: u64,
    backend: Backend,
) -> RecognizeResult {
    RecognizeResult {
        text: rounds::text(&words),
        language,
        segments: rounds::segments(&words),
        duration_ms,
        backend,
    }
}

/// The tail decode started once the take's samples were final, while the
/// capture stops, the take is written and the speech gate judges it. The thread is never joined; a take the gate drops, or one
/// that reuses the last round, leaves it to finish on its own.
/// How the take ended, as the recording loop knows it at the release.
#[derive(Clone, Copy)]
pub(crate) enum Ending {
    /// The take kept its whole release tail: voice reached the release.
    Speech,
    /// A silent ending whose last voiced window ends here, in samples.
    VoicedUntil(u64),
    /// A silent ending the level stage decided: where the voice ended is
    /// known only once the speech check finishes.
    Silent,
}

pub(crate) struct Ahead {
    /// The take's length it decoded the end of.
    samples: usize,
    from_ms: u64,
    cut_ms: u64,
    answer: Receiver<Result<RecognizeResult, EngineError>>,
}

impl Worker {
    /// Starts the release's tail decode now, when the rounds call for one
    /// whatever the speech check says next: a take that may reuse its last
    /// round never starts one.
    pub(super) fn decode_ahead(
        &self,
        samples: &[i16],
        held: Option<&Held>,
        ending: Ending,
    ) -> Option<Ahead> {
        let held = held.filter(|h| h.rounds.any())?;
        let end_ms = ms(samples.len());
        let voiced_ms = match ending {
            Ending::Speech => end_ms,
            Ending::VoicedUntil(v) => v * 1000 / u64::from(SAMPLE_RATE),
            Ending::Silent => return None,
        };
        let Release::Tail { from_ms, cut_ms } =
            held.rounds.release(&WINDOWS, end_ms, Some(voiced_ms))
        else {
            return None;
        };
        let parakeet = self.engine.provider() == "parakeet";
        let start = (from_ms as usize * SAMPLE_RATE as usize / 1000).min(samples.len());
        let clip = gate::pad_for_engine(samples[start..].to_vec(), parakeet, SAMPLE_RATE);
        let (engine, request) = (self.engine.clone(), self.request());
        let (tx, answer) = mpsc::channel();
        std::thread::Builder::new()
            .name("dictation-tail".into())
            .spawn(move || {
                let _ = tx.send(engine.recognize(request(clip), TIMEOUT));
            })
            .ok()?;
        Some(Ahead {
            samples: samples.len(),
            from_ms,
            cut_ms,
            answer,
        })
    }

    /// Rounds run for Parakeet on a GPU; a CPU re-decoding overlapping
    /// windows spends more than it saves (ADR 0102).
    pub(super) fn rounds_wanted(&self) -> bool {
        self.engine.provider() == "parakeet"
            && self.engine.backend().is_some_and(|b| b != Backend::Cpu)
    }

    /// The terms an echo of the prompt is judged against.
    pub(super) fn echo_terms(&self) -> Vec<String> {
        if self.engine.capabilities().supports_custom_vocabulary {
            prompt_terms(&vocabulary_schema::terms(&self.policy.vocabulary)).to_vec()
        } else {
            Vec::new()
        }
    }

    /// The take's request for `pcm`: the rounds and the release ask alike.
    pub(super) fn request(&self) -> Request {
        let terms = vocabulary_schema::terms(&self.policy.vocabulary);
        let prompt = vocabulary_prompt(&terms);
        let language = self.policy.language.clone();
        let vocabulary = if self.engine.provider() == "parakeet" {
            crate::bias::vocabulary_bias(&self.policy.vocabulary, self.policy.vocabulary_boost)
        } else {
            crate::bias::checked_terms(&self.echo_terms(), &self.policy.vocabulary)
        };
        Box::new(move |pcm| RecognizeRequest {
            pcm,
            language: language.clone(),
            prompt: prompt.clone(),
            timestamps: true,
            vocabulary: vocabulary.clone(),
        })
    }

    /// Recognizes the take: from the rounds when they cover it, whole
    /// otherwise. `voiced_until` is the end of the take's last voiced
    /// window in samples, when the speech check knows.
    pub(super) fn recognize_take(
        &self,
        samples: &[i16],
        held: Option<Held>,
        voiced_until: Option<u64>,
        ahead: Option<Ahead>,
    ) -> (Result<RecognizeResult, EngineError>, Option<ReleaseDecode>) {
        let Some(held) = held else {
            return (self.recognize_whole(samples), None);
        };
        // Every round skipped or failed: the whole take, noted in History.
        if !held.rounds.any() {
            let fell_back = (held.skipped > 0).then_some(ReleaseDecode::Whole);
            return (self.recognize_whole(samples), fell_back);
        }
        let end_ms = ms(samples.len());
        let voiced_ms = voiced_until.map(|v| v * 1000 / u64::from(SAMPLE_RATE));
        let parakeet = self.engine.provider() == "parakeet";
        let (started, mut decoded_ms) = (std::time::Instant::now(), 0);
        let decoded = match held.rounds.release(&WINDOWS, end_ms, voiced_ms) {
            Release::Reuse(words) => {
                let backend = held.backend.unwrap_or(Backend::Cpu);
                Some((
                    Ok(result_of(words, held.language, end_ms, backend)),
                    ReleaseDecode::Silence,
                ))
            }
            Release::Tail { from_ms, cut_ms } => {
                decoded_ms = end_ms - from_ms;
                let early = ahead
                    .filter(|a| {
                        (a.samples, a.from_ms, a.cut_ms) == (samples.len(), from_ms, cut_ms)
                    })
                    .and_then(|a| a.answer.recv().ok());
                let start = (from_ms as usize * SAMPLE_RATE as usize / 1000).min(samples.len());
                let clip = &samples[start..];
                let request = self.request();
                let tail = early.unwrap_or_else(|| {
                    let padded = gate::pad_for_engine(clip.to_vec(), parakeet, SAMPLE_RATE);
                    self.engine.recognize(request(padded), TIMEOUT)
                });
                // An empty padded tail runs once more as captured, as a
                // whole take does (ADR 0081).
                let tail = match tail {
                    Ok(r) if r.text.trim().is_empty() && !self.cancel.load(Ordering::Relaxed) => {
                        self.engine.recognize(request(clip.to_vec()), TIMEOUT)
                    }
                    other => other,
                };
                Some((
                    tail.map(|r| {
                        let words = r.segments.into_iter().flat_map(|s| s.words);
                        let all = held.rounds.stitch(cut_ms, rounds::shifted(words, from_ms));
                        result_of(all, r.language, end_ms, r.backend)
                    }),
                    ReleaseDecode::Tail,
                ))
            }
            Release::Whole => None,
        };
        tracing::info!(
            job = %self.job_id,
            ran = held.ran,
            skipped = held.skipped,
            committed = held.rounds.committed(),
            busy_ms = held.busy.as_millis() as u64,
            waited_ms = held.waited.as_millis() as u64,
            decoded_ms,
            release_ms = started.elapsed().as_millis() as u64,
            release = ?decoded.as_ref().map_or(ReleaseDecode::Whole, |d| d.1),
            "dictation rounds"
        );
        match decoded {
            // Stitched words that came out empty are decoded whole, so the
            // rounds never lose a take the whole decode would have read.
            Some((Ok(r), how)) if !r.text.trim().is_empty() => (Ok(r), Some(how)),
            Some((Err(e), how)) => (Err(e), Some(how)),
            _ => (self.recognize_whole(samples), Some(ReleaseDecode::Whole)),
        }
    }

    /// The whole take, as dictation decoded every take before the rounds.
    fn recognize_whole(&self, samples: &[i16]) -> Result<RecognizeResult, EngineError> {
        let parakeet = self.engine.provider() == "parakeet";
        let request = self.request();
        let padded = gate::pad_for_engine(samples.to_vec(), parakeet, SAMPLE_RATE);
        let was_padded = padded.len() != samples.len();
        let recognized = self.engine.recognize(request(padded), TIMEOUT);
        // Padding never empties a take: Parakeet v3 reads some takes whole
        // only without the added silence (ADR 0081), so those run once more as captured.
        if was_padded
            && recognized.as_ref().is_ok_and(|r| r.text.trim().is_empty())
            && !self.cancel.load(Ordering::Relaxed)
        {
            return self.engine.recognize(request(samples.to_vec()), TIMEOUT);
        }
        recognized
    }
}
