//! The session's worker thread: drains the source into a take, meters
//! levels, passes the take through the speech gate, transcribes it through
//! the frozen engine, guards the engine's text, applies the raw pipeline,
//! hands the text over and records the outcome. The gate judges the take's
//! samples, never the meter's display windows: the take runs to the end of
//! its release tail (ADR 0092). A take the gate finds silent ends
//! `cancelled` without reaching the engine, and so does a tap.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use dettivo_audio::takes::TakeWriter;
use dettivo_speech::gate::{self, GateVerdict, VoiceDetector};
use dettivo_speech::{EngineError, RecognizeRequest, SttEngine, prompt_terms, vocabulary_prompt};

use dettivo_core::config::vocabulary_schema;
use dettivo_language::pipeline::{Mode, Settings};
use dettivo_language::provider::LocalEngine;

use crate::machine::{Exit, SharedState, finish_shared};
use crate::{
    Archive, Inserter, Policy, Publisher, SessionTarget, SessionTimings, State, StateChange,
    Transcript,
};

mod record;
use record::Recorded;

pub(crate) fn elapsed_ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub(crate) struct Worker {
    pub(crate) job_id: String,
    pub(crate) policy: Policy,
    pub(crate) engine: Arc<dyn SttEngine>,
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) cancel: Arc<AtomicBool>,
    /// When the release arrived, once a stop asked for the take's end.
    pub(crate) released: Arc<OnceLock<Instant>>,
    /// A cancel ends the take as a tap.
    pub(crate) tap: Arc<AtomicBool>,
    pub(crate) dir: PathBuf,
    pub(crate) publisher: Arc<dyn Publisher>,
    pub(crate) inserter: Arc<dyn Inserter>,
    pub(crate) archive: Option<Arc<dyn Archive>>,
    pub(crate) target: Option<SessionTarget>,
    pub(crate) local_llm: Option<Arc<dyn LocalEngine>>,
    pub(crate) vad: Option<Arc<dyn VoiceDetector>>,
}

impl Worker {
    fn set(&self, shared: &SharedState, state: State, progress: f64, reason: Option<String>) {
        let mut g = shared.active.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(a) = g.as_mut() {
            let previous = a.snapshot.state;
            a.snapshot.state = state;
            a.snapshot.progress = progress;
            drop(g);
            if previous != state {
                self.publisher
                    .state(&StateChange::plain(&self.job_id, state, previous, reason));
            }
        }
    }

    pub(crate) fn run(self, src: crate::source::Source, shared: SharedState) {
        let outcome = self.record(&src, &shared);
        src.stop();
        let Recorded {
            samples,
            duration_ms,
            exit: record_exit,
            capture_ms,
        } = match outcome {
            Ok(v) => v,
            Err(exit) => {
                self.cleanup();
                finish_shared(
                    &shared,
                    &*self.publisher,
                    &self.job_id,
                    State::Recording,
                    exit,
                    None,
                );
                return;
            }
        };
        if let Some(exit) = record_exit {
            self.cleanup();
            finish_shared(
                &shared,
                &*self.publisher,
                &self.job_id,
                State::Recording,
                exit,
                None,
            );
            return;
        }
        self.set(
            &shared,
            State::Transcribing,
            0.0,
            Some("loading engine".into()),
        );
        let transcribe_started = Instant::now();
        let verdict = gate::judge(&samples, self.policy.silence_dbfs, self.vad.as_deref());
        // A cancel during the voice pass reached no engine to stop.
        if self.cancel.load(Ordering::Relaxed) {
            self.cleanup();
            finish_shared(
                &shared,
                &*self.publisher,
                &self.job_id,
                State::Transcribing,
                self.cancelled(),
                None,
            );
            return;
        }
        if let GateVerdict::Silence(stage) = verdict {
            tracing::info!(job = %self.job_id, stage = stage.as_str(), "take is silent; nothing sent to the engine");
            self.cleanup();
            finish_shared(
                &shared,
                &*self.publisher,
                &self.job_id,
                State::Transcribing,
                Exit::Silence(stage),
                None,
            );
            return;
        }
        let terms = vocabulary_schema::terms(&self.policy.vocabulary);
        // An echo of the prompt is only possible from an engine that took one.
        let echo_terms = if self.engine.capabilities().supports_custom_vocabulary {
            prompt_terms(&terms)
        } else {
            &[]
        };
        let parakeet = self.engine.provider() == "parakeet";
        let request = |pcm| RecognizeRequest {
            pcm,
            language: self.policy.language.clone(),
            prompt: vocabulary_prompt(&terms),
            timestamps: true,
        };
        let padded = gate::pad_for_engine(samples.clone(), parakeet, gate::SAMPLE_RATE);
        let was_padded = padded.len() != samples.len();
        let timeout = Duration::from_secs(300);
        let mut recognized = self.engine.recognize(request(padded), timeout);
        // Padding never empties a take: Parakeet v3 reads some takes whole
        // only without the added silence (ADR 0081), so those run once more as captured.
        if was_padded
            && recognized.as_ref().is_ok_and(|r| r.text.trim().is_empty())
            && !self.cancel.load(Ordering::Relaxed)
        {
            recognized = self.engine.recognize(request(samples), timeout);
        }
        let recognized = recognized.map(|r| (crate::guards::clean(&r, echo_terms), r.language));
        if self.cancel.load(Ordering::Relaxed) {
            self.cleanup();
            finish_shared(
                &shared,
                &*self.publisher,
                &self.job_id,
                State::Transcribing,
                self.cancelled(),
                None,
            );
            return;
        }
        let (raw_text, language) = match recognized {
            Ok(v) => v,
            Err(EngineError::Cancelled) => {
                self.cleanup();
                finish_shared(
                    &shared,
                    &*self.publisher,
                    &self.job_id,
                    State::Transcribing,
                    self.cancelled(),
                    None,
                );
                return;
            }
            Err(e) => {
                tracing::warn!(job = %self.job_id, error = %redact_error(&e), "transcription failed");
                self.cleanup();
                finish_shared(
                    &shared,
                    &*self.publisher,
                    &self.job_id,
                    State::Transcribing,
                    Exit::Failed(format!("engine: {}", redact_error(&e))),
                    None,
                );
                return;
            }
        };
        let mode = Mode::parse(&self.policy.mode).unwrap_or(Mode::Raw);
        let app_id = self.target.as_ref().and_then(|t| t.app_id.as_deref());
        let stepped = dettivo_language::pipeline::run(
            mode,
            &raw_text,
            app_id,
            &Settings {
                replacements: &self.policy.replacements,
                spoken_punctuation: self.policy.spoken_punctuation,
                protect_tokens: self.policy.protect_tokens,
                vocabulary: &self.policy.vocabulary,
                rules: &self.policy.rules,
                llm: &self.policy.llm,
                local: self.local_llm.as_ref(),
            },
        );
        tracing::info!(
            job = %self.job_id,
            mode = mode.as_str(),
            policy_hash = %stepped.policy_hash,
            model = %stepped.model,
            notice = stepped.notice.as_ref().map(|n| n.kind.as_str()).unwrap_or("-"),
            "dictation text step"
        );
        let text = stepped.text;
        let transcribe_ms = elapsed_ms(transcribe_started);
        // A cancel that landed while the text was being polished: nothing
        // is typed and nothing is archived. Past this point the delivery
        // has started and a cancel finds no session to stop.
        if self.cancel.load(Ordering::Relaxed) {
            self.cleanup();
            finish_shared(
                &shared,
                &*self.publisher,
                &self.job_id,
                State::Transcribing,
                self.cancelled(),
                None,
            );
            return;
        }
        self.set(&shared, State::Inserting, 0.9, None);
        let insert_started = Instant::now();
        let insertion = if text.is_empty() {
            None
        } else {
            Some(self.inserter.insert(&text, self.target.as_ref()))
        };
        let timings = SessionTimings {
            capture_ms,
            transcribe_ms,
            // Nothing to insert means no inserter call: the contract says 0.
            insert_ms: if insertion.is_some() {
                elapsed_ms(insert_started)
            } else {
                0
            },
        };
        let transcript = Transcript {
            job_id: self.job_id.clone(),
            id: uuid_like(&self.job_id),
            text,
            raw_text,
            polished_text: (mode != Mode::Raw).then_some(stepped.polished),
            notice: stepped.notice,
            policy_hash: Some(stepped.policy_hash),
            language,
            duration_ms,
            policy: self.policy.clone(),
            insertion,
            target: self.target.clone(),
            timings: Some(timings),
        };
        // The item is stored before the final state is published, so a
        // client that sees `idle` can already read it back; a store that
        // fails is a warning on that state, never a lost insertion.
        let warning = self.archive.as_ref().and_then(|a| {
            a.archive(&transcript, &self.dir).err().map(|e| {
                tracing::warn!(job = %self.job_id, error = %e, "history: item not stored");
                format!("history: {e}")
            })
        });
        self.cleanup();
        tracing::info!(job = %self.job_id, chars = transcript.text.chars().count(), duration_ms, "dictation finished");
        finish_shared(
            &shared,
            &*self.publisher,
            &self.job_id,
            State::Inserting,
            Exit::Completed(warning),
            Some(transcript),
        );
    }

    /// How a cancel ends this take: a tap, or a cancel.
    fn cancelled(&self) -> Exit {
        if self.tap.load(Ordering::Relaxed) {
            Exit::Tap
        } else {
            Exit::Cancelled
        }
    }

    fn cleanup(&self) {
        if let Err(e) = TakeWriter::discard(&self.dir) {
            tracing::debug!(job = %self.job_id, error = %e, "take discard");
        }
        let _ = std::fs::remove_dir(&self.dir);
    }
}

/// A UUID-shaped id derived from the job id and the clock, so `TranscriptRef`
/// carries what the contract expects until the store assigns real ids.
fn uuid_like(job_id: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in job_id.bytes().chain(nanos.to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    let a = h;
    let b = h.rotate_left(29) ^ (nanos as u64);
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (a >> 32) as u32,
        (a >> 16) as u16,
        (a as u16) & 0x0fff,
        ((b >> 48) as u16 & 0x3fff) | 0x8000,
        b & 0xffff_ffff_ffff
    )
}

/// Engine errors without any content: codes and states only.
fn redact_error(e: &EngineError) -> String {
    match e {
        EngineError::Crashed(_) => "engine crashed".into(),
        other => other
            .to_string()
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(120)
            .collect(),
    }
}
