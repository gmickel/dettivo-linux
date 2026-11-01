//! The session's worker thread: drains the source into a take, meters
//! levels, passes the take through the speech gate, transcribes it through
//! the frozen engine, guards the engine's text, applies the raw pipeline,
//! hands the text over and records the outcome. The gate judges what was
//! captured before the release, never the meter's display windows, and a
//! take it finds silent ends `cancelled` without reaching the engine.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

use dettivo_audio::takes::TakeWriter;
use dettivo_audio::{EndReason, Event};
use dettivo_speech::gate::{self, GateVerdict, VoiceDetector};
use dettivo_speech::{EngineError, RecognizeRequest, SttEngine, prompt_terms, vocabulary_prompt};

use dettivo_language::pipeline::{Mode, Settings};
use dettivo_language::provider::LocalEngine;

use crate::machine::{Exit, SharedState, finish_shared};
use crate::{
    Archive, Inserter, Level, Policy, Publisher, SessionTarget, SessionTimings, State, StateChange,
    Transcript,
};

/// What the recording phase hands to the rest of the worker.
struct Recorded {
    samples: Vec<i16>,
    duration_ms: u64,
    /// How many samples were captured before the release.
    released_at: usize,
    /// An early exit when the recording itself decided the outcome.
    exit: Option<Exit>,
    /// From the stop being observed to the take being final.
    capture_ms: u64,
}

fn elapsed_ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub(crate) struct Worker {
    pub(crate) job_id: String,
    pub(crate) policy: Policy,
    pub(crate) engine: Arc<dyn SttEngine>,
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) cancel: Arc<AtomicBool>,
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
            released_at,
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
        let verdict = gate::judge(
            &samples[..released_at],
            self.policy.silence_dbfs,
            self.vad.as_deref(),
        );
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
        // An echo of the prompt is only possible from an engine that took one.
        let echo_terms = if self.engine.capabilities().supports_custom_vocabulary {
            prompt_terms(&self.policy.vocabulary)
        } else {
            &[]
        };
        let parakeet = self.engine.provider() == "parakeet";
        let request = |pcm| RecognizeRequest {
            pcm,
            language: self.policy.language.clone(),
            prompt: vocabulary_prompt(&self.policy.vocabulary),
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
                Exit::Cancelled,
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
                    Exit::Cancelled,
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
                Exit::Cancelled,
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

    /// Drains the source until stop, cancel, the maximum duration or the
    /// source's end. Returns the samples, the duration, how many samples
    /// came before the release, an early exit when the recording itself
    /// decided the outcome, and how long the drain and the take's finish
    /// took after the stop.
    fn record(&self, src: &crate::source::Source, shared: &SharedState) -> Result<Recorded, Exit> {
        let mut writer =
            TakeWriter::new(&self.dir).map_err(|e| Exit::Failed(format!("take: {e}")))?;
        writer
            .start_take(false)
            .map_err(|e| Exit::Failed(format!("take: {e}")))?;
        let mut samples: Vec<i16> = Vec::new();
        let started = Instant::now();
        let mut ended: Option<EndReason> = None;
        let mut terminal = false;
        let mut released_at: Option<usize> = None;
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return Ok(Recorded {
                    released_at: samples.len(),
                    samples,
                    duration_ms: 0,
                    exit: Some(Exit::Cancelled),
                    capture_ms: 0,
                });
            }
            // What the source delivered before the stop was captured before
            // the release, queued or not; a chunk the worker waited for and
            // that came after the stop opens the post-release tail.
            let stopping =
                self.stop.load(Ordering::Relaxed) || started.elapsed() >= self.policy.max_duration;
            let (next, waited) = match src.events().try_recv() {
                Ok(event) => (Ok(event), false),
                Err(TryRecvError::Disconnected) => (Err(RecvTimeoutError::Disconnected), false),
                Err(TryRecvError::Empty) if stopping => break,
                Err(TryRecvError::Empty) => {
                    (src.events().recv_timeout(Duration::from_millis(50)), true)
                }
            };
            if waited && self.stop.load(Ordering::Relaxed) {
                released_at.get_or_insert(samples.len());
            }
            match next {
                Ok(Event::Pcm(chunk)) => {
                    writer
                        .write(&chunk)
                        .map_err(|e| Exit::Failed(format!("take: {e}")))?;
                    samples.extend_from_slice(&chunk);
                    let progress = (started.elapsed().as_secs_f64()
                        / self.policy.max_duration.as_secs_f64())
                    .min(1.0);
                    if let Some(a) = shared
                        .active
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .as_mut()
                    {
                        a.snapshot.progress = progress;
                    }
                }
                Ok(Event::Level { rms, peak: p }) => {
                    self.publisher.level(Level { rms, peak: p });
                }
                Ok(Event::DeviceChanged { from, to }) => {
                    ended = Some(EndReason::DeviceLost(format!(
                        "default moved from {} to {}",
                        from.unwrap_or_else(|| "none".into()),
                        to.unwrap_or_else(|| "none".into())
                    )));
                    break;
                }
                Ok(Event::Ended { reason }) => {
                    ended = Some(reason);
                    terminal = true;
                    break;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    ended = Some(EndReason::Error(
                        "capture stop acknowledgement missing".into(),
                    ));
                    terminal = true;
                    break;
                }
            }
        }
        let released = Instant::now();
        let released_at = released_at.unwrap_or(samples.len());
        src.stop();
        let deadline = released + Duration::from_secs(2);
        while !terminal {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(Exit::Failed(
                    "capture stop acknowledgement timed out".into(),
                ));
            }
            let event = src.events().recv_timeout(remaining).map_err(|_| {
                Exit::Failed("capture stop acknowledgement missing or timed out".into())
            })?;
            match event {
                Event::Pcm(chunk) => {
                    writer
                        .write(&chunk)
                        .map_err(|e| Exit::Failed(format!("take: {e}")))?;
                    samples.extend_from_slice(&chunk);
                }
                Event::Level { rms, peak: p } => {
                    self.publisher.level(Level { rms, peak: p });
                }
                Event::Ended { reason } => {
                    terminal = true;
                    if ended.is_none() {
                        ended = Some(reason);
                    }
                }
                _ => {}
            }
        }
        let takes = writer
            .finish()
            .map_err(|e| Exit::Failed(format!("take: {e}")))?;
        let duration_ms = takes.takes.iter().map(|t| t.samples).sum::<u64>() * 1000 / 16_000;
        let exit = match ended {
            Some(EndReason::DeviceLost(name)) => Some(Exit::Failed(format!("device lost: {name}"))),
            Some(EndReason::NoSource) => Some(Exit::Failed("no audio source".into())),
            Some(EndReason::Error(e)) | Some(EndReason::Write(e)) => {
                Some(Exit::Failed(format!("capture error: {e}")))
            }
            Some(EndReason::Stopped) | Some(EndReason::FixtureFinished) | None => None,
        };
        Ok(Recorded {
            samples,
            duration_ms,
            released_at,
            exit,
            capture_ms: elapsed_ms(released),
        })
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
