//! The early end of the release tail (ADR 0093): a take whose last
//! [`SILENT_ENDING`] before the release holds no speech by the gate's own
//! rules ends at the release instead of after the tail. The level stage
//! judges that stretch, and what was captured after it up to the cut,
//! first; when it hears sound, the speech check that ran during the take
//! answers whether any voice window there reached the gate's threshold.
//! Without that answer the take keeps its whole tail.
//!
//! The take then ends at the first voice window boundary at or after the
//! release and after what the check already holds, so the check has run
//! every sample the take keeps and the release adds none for it to run.

use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Instant;

use dettivo_speech::gate::{SAMPLE_RATE, VAD_WINDOW, loudest_window_dbfs};

use super::check::SpeechCheck;
use crate::SILENT_ENDING;

/// Where the early end stands once a stop was seen.
pub(super) enum Early {
    /// Waiting for the audio up to the window boundary after the release.
    Pending,
    /// The check was asked whether a voice window reached past the
    /// stretch before `release`; the take would end at `end`.
    Asked {
        release: usize,
        end: usize,
        answer: Receiver<Option<u64>>,
    },
    /// The take keeps its whole tail.
    Kept,
}

/// What the early end needs from the recording loop.
pub(super) struct Heard<'a> {
    /// The take's samples so far.
    pub(super) samples: &'a [i16],
    /// The cut at the release, once audio captured after it arrived.
    pub(super) at_release: Option<usize>,
    /// The take's silence floor (dBFS).
    pub(super) silence_dbfs: f32,
    /// The speech check, when the detector streams.
    pub(super) check: Option<&'a SpeechCheck>,
    /// The tail is covered or stalled, so an answer arriving now is late
    /// and the take keeps its tail.
    pub(super) tail_done: bool,
}

/// The samples [`SILENT_ENDING`] spans.
fn window_samples() -> usize {
    (SILENT_ENDING.as_micros() * u128::from(SAMPLE_RATE) / 1_000_000) as usize
}

impl Early {
    /// Advances the early end. `fed` counts the samples sent to the check.
    /// Returns where the take ends when its ending is silent.
    pub(super) fn advance(&mut self, heard: Heard<'_>, fed: &mut usize) -> Option<usize> {
        // Once the tail is done the take already holds it, so cutting
        // early would only discard captured audio: the tail stays.
        if heard.tail_done {
            *self = Early::Kept;
            return None;
        }
        match self {
            Early::Kept => None,
            Early::Pending => {
                let release = heard.at_release?;
                // Too little was captured before the release to call its
                // last stretch silent.
                if release < window_samples() {
                    *self = Early::Kept;
                    return None;
                }
                // The check may already hold samples from after the release.
                let end = release.max(*fed).div_ceil(VAD_WINDOW) * VAD_WINDOW;
                if heard.samples.len() < end {
                    return None;
                }
                let window = &heard.samples[release - window_samples()..end];
                if loudest_window_dbfs(window, SAMPLE_RATE).is_none_or(|db| db < heard.silence_dbfs)
                {
                    tracing::debug!(
                        end,
                        "the take ends at the release: its last stretch is under the silence floor"
                    );
                    return Some(end);
                }
                *self = match heard.check {
                    Some(check) => {
                        let answer = check.ask_voiced(&heard.samples[*fed..end]);
                        *fed = end;
                        Early::Asked {
                            release,
                            end,
                            answer,
                        }
                    }
                    None => Early::Kept,
                };
                None
            }
            Early::Asked {
                release,
                end,
                answer,
            } => match answer.try_recv() {
                Err(TryRecvError::Empty) => None,
                Ok(Some(voiced)) if voiced as usize + window_samples() <= *release => {
                    tracing::debug!(
                        end = *end,
                        voiced,
                        "the take ends at the release: its last stretch holds no voice"
                    );
                    Some(*end)
                }
                _ => {
                    *self = Early::Kept;
                    None
                }
            },
        }
    }

    /// Waiting on the check's answer.
    pub(super) fn asked(&self) -> bool {
        matches!(self, Early::Asked { .. })
    }
}

/// True once audio captured after `released` arrived, so the cut at the
/// release no longer moves.
pub(super) fn past(heard: Option<Instant>, released: Instant) -> bool {
    heard.is_some_and(|h| h > released)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// An answer of no voice, already waiting when the loop next looks.
    fn asked() -> Early {
        let (tx, answer) = mpsc::channel();
        tx.send(Some(0)).unwrap();
        Early::Asked {
            release: 16_000,
            end: 16_384,
            answer,
        }
    }

    fn heard(samples: &[i16], tail_done: bool) -> Heard<'_> {
        Heard {
            samples,
            at_release: Some(16_000),
            silence_dbfs: -50.0,
            check: None,
            tail_done,
        }
    }

    #[test]
    fn an_answer_in_time_ends_the_take_at_the_release() {
        let samples = vec![0; 16_384];
        assert_eq!(
            asked().advance(heard(&samples, false), &mut 0),
            Some(16_384)
        );
    }

    #[test]
    fn an_answer_after_the_tail_is_done_keeps_the_tail() {
        let samples = vec![0; 16_384];
        let mut early = asked();
        assert_eq!(early.advance(heard(&samples, true), &mut 0), None);
        assert!(matches!(early, Early::Kept));
    }

    #[test]
    fn a_silent_stretch_seen_after_the_tail_is_done_keeps_the_tail() {
        let samples = vec![0; 20_000];
        let mut early = Early::Pending;
        assert_eq!(early.advance(heard(&samples, true), &mut 0), None);
        assert!(matches!(early, Early::Kept));
    }
}
