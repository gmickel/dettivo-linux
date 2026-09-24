//! How a meeting ends: the takes are closed and the row settled
//! (`finish`), a stop frees the recording slot and finalises every take
//! through the offline pipeline (`finalize_meeting`, shared with the
//! recovery path), and only once the settled row is stored does the
//! artifact policy decide what stays in the directory, so finalisation
//! always finds the audio it needs and a store that refuses the row
//! leaves the takes for a retry.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use dettivo_proto::methods::meetings_notes::AnalysisStatus;
use dettivo_proto::methods::speakers::DiarizationStatus;
use dettivo_speech::SttEngine;
use dettivo_storage::meetings::{MeetingRow, MeetingStatus};
use dettivo_storage::retention::ArtifactPolicy;
use dettivo_storage::time::now_iso;
use dettivo_transcribe::{JobError, Progress, Request, Stage};
use serde_json::json;

use crate::checkpoint::Checkpoint;
use crate::finalize;
use crate::journal::Journal;
use crate::machine::{Finalizations, Finalizing, Shared};
use crate::worker::{Exit, TrackState, Worker};
use crate::{Archive, FinalizeProgress, Policy, Publisher, State, StateChange, Track};

/// What one finalisation needs, on the worker's thread after a stop or
/// on its own thread after a recovery.
pub(crate) struct FinalizeContext {
    pub(crate) job_id: String,
    pub(crate) row: MeetingRow,
    pub(crate) dir: PathBuf,
    pub(crate) policy: Policy,
    pub(crate) engine: Arc<dyn SttEngine>,
    pub(crate) publisher: Arc<dyn Publisher>,
    pub(crate) archive: Arc<dyn Archive>,
    pub(crate) finalizing: Finalizations,
    /// The state the meeting leaves for `transcribing`.
    pub(crate) previous: State,
    /// Milliseconds the capture measured; the takes decide when longer.
    pub(crate) capture_duration_ms: u64,
}

impl FinalizeContext {
    fn change(&self, state: State, previous: State, reason: Option<String>) -> StateChange {
        StateChange {
            meeting_id: self.row.id.clone(),
            job_id: self.job_id.clone(),
            state,
            previous,
            reason,
            duration_ms: self.row.duration_ms,
            microphone_takes: self.row.microphone_takes,
            system_audio: false,
            live_segment_count: self.row.segments.len() as u64,
            live_last_end_ms: self.row.segments.last().map(|s| s.end_ms).unwrap_or(0),
            is_finalizing: state == State::Transcribing,
            // The passes the archive planned on the row travel with the
            // `completed` transition, so a client hears what runs next
            // without waiting for the passes to say so themselves.
            diarization_status: (state == State::Completed)
                .then(|| self.row.diarization.as_ref().map(|d| d.status))
                .flatten()
                .filter(|s| *s == DiarizationStatus::Queued),
            analysis_status: (state == State::Completed
                && self.row.analysis_status == AnalysisStatus::Queued)
                .then_some(AnalysisStatus::Queued),
        }
    }
}

/// Runs the finalisation over the takes under `ctx.dir`, keeps the row
/// and the checkpoint current per chunk, and settles the row completed,
/// failed or (after a cancel) stopped. Returns the row as stored.
pub(crate) fn finalize_meeting(mut ctx: FinalizeContext) -> MeetingRow {
    let journal = Journal::open(&ctx.dir, std::time::Instant::now());
    let cancel = Arc::new(AtomicBool::new(false));
    ctx.finalizing
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(
            ctx.row.id.clone(),
            Finalizing {
                job_id: ctx.job_id.clone(),
                cancel: cancel.clone(),
                chunks_completed: 0,
                chunks_total: 0,
            },
        );
    ctx.row.status = MeetingStatus::Transcribing;
    ctx.row.chunks_completed = 0;
    ctx.row.chunks_total = 0;
    let storage_error = ctx.archive.updated(&ctx.row).err();
    ctx.publisher
        .state(&ctx.change(State::Transcribing, ctx.previous, None));
    let takes = if let Some(error) = &storage_error {
        Err(JobError::Audio(format!("storage: {error}")))
    } else if ctx.row.system_audio && !ctx.dir.join("system-takes.json").is_file() {
        Err(JobError::Audio("missing system-takes.json".into()))
    } else {
        finalize::takes_in(&ctx.dir)
    };
    let gaps = finalize::journal_gaps(&ctx.dir);
    journal.record(
        "finalize",
        None,
        None,
        Some(format!(
            "takes={} gaps={}",
            takes.as_ref().map_or(0, Vec::len),
            gaps.len()
        )),
    );
    let request = Request {
        language: if ctx.row.language.is_empty() {
            "auto".into()
        } else {
            ctx.row.language.clone()
        },
        prompt: ctx.policy.prompt.clone(),
        from_system: false,
    };
    let mut checkpoint = Checkpoint::read(&ctx.dir)
        .unwrap_or_else(|_| Checkpoint::new(&ctx.row.id, &ctx.row.started_at));
    checkpoint.segments = ctx.row.segments.clone();
    checkpoint.is_finalizing = true;
    let mut on_progress = |p: Progress| {
        {
            let mut g = ctx.finalizing.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(f) = g.get_mut(&ctx.row.id) {
                f.chunks_completed = p.chunks_done;
                f.chunks_total = p.chunks_total;
            }
        }
        checkpoint.chunks_completed = p.chunks_done;
        checkpoint.chunks_total = p.chunks_total;
        checkpoint.updated_at = now_iso();
        if let Err(e) = checkpoint.write(&ctx.dir) {
            tracing::debug!(error = %e, "meeting: finalisation checkpoint not written");
        }
        ctx.publisher.progress(
            &ctx.job_id,
            &ctx.row.id,
            FinalizeProgress {
                stage: p.stage,
                chunks_done: p.chunks_done,
                chunks_total: p.chunks_total,
            },
        );
    };
    let outcome = takes.and_then(|takes| {
        finalize::run(
            &takes,
            &gaps,
            ctx.engine.as_ref(),
            &request,
            &ctx.policy.transcribe,
            &ctx.policy.tuning,
            &cancel,
            &mut on_progress,
        )
    });
    let (state, reason, stage) = match outcome {
        Ok(out) => {
            ctx.row.segments = dettivo_transcribe::live::contract_segments(&out.segments);
            ctx.row.raw_text = out.text.clone();
            ctx.row.final_text = out.text;
            if !out.language.is_empty() {
                ctx.row.language = out.language;
            }
            ctx.row.duration_ms = ctx.capture_duration_ms.max(out.duration_ms);
            ctx.row.chunks_completed = out.chunks;
            ctx.row.chunks_total = out.chunks;
            ctx.row.status = MeetingStatus::Completed;
            ctx.row.error_code = None;
            ctx.row.error_message = None;
            ctx.archive.completed(&mut ctx.row);
            journal.record(
                "finalized",
                None,
                None,
                Some(format!(
                    "segments={} chunks={}{}",
                    ctx.row.segments.len(),
                    out.chunks,
                    out.notice
                        .as_deref()
                        .map(|n| format!(" notice={n}"))
                        .unwrap_or_default()
                )),
            );
            (State::Completed, None, Stage::Done)
        }
        Err(JobError::Cancelled { .. }) => {
            ctx.row.status = MeetingStatus::Stopped;
            journal.record("finalize_cancelled", None, None, None);
            (
                State::Stopped,
                Some("finalisation cancelled".to_string()),
                Stage::Cancelled,
            )
        }
        Err(e) => {
            let message = e.to_string();
            ctx.row.status = MeetingStatus::Failed;
            ctx.row.error_code = Some(
                if storage_error.is_some() {
                    "storage"
                } else {
                    "transcription"
                }
                .into(),
            );
            ctx.row.error_message = Some(message.clone());
            journal.record("error", None, None, Some(message.clone()));
            (State::Failed, Some(message), Stage::Failed)
        }
    };
    ctx.row.ended_at.get_or_insert_with(now_iso);
    // The row is stored before anything is cleaned up: a store that
    // refuses it keeps the checkpoint and the takes for a retry, and the
    // meeting ends failed with the storage error instead of completed.
    let audio_dir = ctx.row.audio_dir.clone();
    settled_facts(&mut ctx.row, &ctx.policy, state);
    let (state, reason, stage) = match ctx.archive.updated(&ctx.row) {
        Ok(()) => {
            if state == State::Completed {
                Checkpoint::remove(&ctx.dir);
            }
            (state, reason, stage)
        }
        Err(e) => {
            let message = format!("storage: {e}");
            tracing::warn!(error = %e, "meeting: final row not stored; the takes stay");
            journal.record("error", None, None, Some(message.clone()));
            ctx.row.audio_dir = audio_dir.clone();
            ctx.row.status = MeetingStatus::Failed;
            ctx.row.error_code = Some("storage".into());
            ctx.row.error_message = Some(message.clone());
            if let Err(e) = ctx.archive.updated(&ctx.row) {
                tracing::warn!(error = %e, "meeting: failed row not stored");
            }
            (State::Failed, Some(message), Stage::Failed)
        }
    };
    let (done, total) = (ctx.row.chunks_completed, ctx.row.chunks_total);
    ctx.finalizing
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&ctx.row.id);
    write_metadata(&ctx.row, &ctx.dir, &ctx.policy);
    ctx.publisher.progress(
        &ctx.job_id,
        &ctx.row.id,
        FinalizeProgress {
            stage,
            chunks_done: done,
            chunks_total: total,
        },
    );
    ctx.publisher
        .state(&ctx.change(state, State::Transcribing, reason));
    if state == State::Completed {
        let mut processing = ctx.row.clone();
        processing.audio_dir = audio_dir;
        ctx.archive
            .settled(&processing, ctx.policy.keep_audio, ctx.policy.artifacts);
    }
    apply_artifacts(&ctx.dir, &ctx.policy, state);
    tracing::info!(job = %ctx.job_id, state = state.as_str(), segments = ctx.row.segments.len(), "meeting finalised");
    ctx.row
}

/// The audio facts the row carries once the artifact policy has run:
/// `none` forgets the directory after a completed finalisation. Set
/// before the row is stored, so the stored row and the directory agree.
pub(crate) fn settled_facts(row: &mut MeetingRow, policy: &Policy, state: State) {
    if matches!(state, State::Completed) && policy.artifacts == ArtifactPolicy::None {
        row.audio_dir = None;
    }
}

/// What stays in the meeting directory once the meeting settled and its
/// row is stored: `none` removes it after a completed finalisation;
/// `keep_audio = false` removes the takes; `keep` writes `metadata.json`
/// beside what remains. A failed finalisation keeps every take so it can
/// be run again.
pub(crate) fn apply_artifacts(dir: &std::path::Path, policy: &Policy, state: State) {
    let settled = matches!(state, State::Completed);
    if settled && policy.artifacts == ArtifactPolicy::None {
        if let Err(e) = std::fs::remove_dir_all(dir) {
            tracing::warn!(error = %e, "meeting: directory not removed");
        }
        return;
    }
    if settled && !policy.keep_audio {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "wav") {
                    if let Err(e) = std::fs::remove_file(path) {
                        tracing::warn!(error = %e, "meeting: temporary audio not removed");
                    }
                }
            }
        }
    }
}

/// `metadata.json`: the identity, both take lists and the checkpoint schema.
pub(crate) fn write_metadata(row: &MeetingRow, dir: &std::path::Path, policy: &Policy) {
    if policy.artifacts != ArtifactPolicy::Keep {
        return;
    }
    let takes: Vec<serde_json::Value> = [Track::Microphone, Track::System]
        .into_iter()
        .map(|track| {
            let sidecar = if track == Track::Microphone {
                dir.join(dettivo_audio::takes::SIDECAR)
            } else {
                dir.join(format!("{}-takes.json", track.prefix()))
            };
            let takes = dettivo_audio::takes::read_sidecar(&sidecar).unwrap_or_default();
            json!({"track": track, "takes": takes.takes})
        })
        .collect();
    let metadata = json!({
        "meeting_id": row.id,
        "takes": takes,
        "journal": crate::JOURNAL_FILE,
        "checkpoint_schema": crate::checkpoint::SCHEMA_VERSION,
    });
    if let Err(e) = std::fs::write(
        dir.join(crate::METADATA_FILE),
        serde_json::to_string_pretty(&metadata).unwrap_or_default(),
    ) {
        tracing::warn!(error = %e, "meeting: metadata not written");
    }
}

impl Worker {
    /// Closes the takes and settles the capture; a stop then frees the
    /// slot and finalises.
    pub(crate) fn finish(
        mut self,
        shared: &Shared,
        journal: &Journal,
        mut exit: Exit,
        mic: Option<TrackState>,
        sys: Option<TrackState>,
    ) {
        let mut tracks = Vec::new();
        for t in [mic, sys].into_iter().flatten() {
            if let Some(s) = &t.source {
                s.stop();
            }
            tracks.push(t);
        }
        let mut sidecars = Vec::new();
        for t in tracks {
            let track = t.track;
            match t.writer.finish() {
                Ok(takes) => sidecars.push((track, takes)),
                Err(e) => {
                    exit = Exit::Failed(format!("{} take not closed: {e}", track.prefix()));
                }
            }
        }
        let live_tail = self.live.take().map(|l| l.finish());
        let duration_ms = sidecars
            .iter()
            .map(|(_, t)| t.duration_ms())
            .max()
            .unwrap_or(0);
        let mic_takes = sidecars
            .iter()
            .find(|(t, _)| *t == Track::Microphone)
            .map(|(_, t)| t.takes.len() as u32)
            .unwrap_or(0);
        self.row.duration_ms = duration_ms;
        self.row.microphone_takes = mic_takes;
        self.row.ended_at = Some(now_iso());
        if !matches!(exit, Exit::Cancelled) {
            let mut checkpoint = Checkpoint::read(&self.dir)
                .unwrap_or_else(|_| Checkpoint::new(&self.row.id, &self.row.started_at));
            for (track, takes) in &sidecars {
                checkpoint.takes.retain(|take| take.track != *track);
                checkpoint.takes.extend(
                    takes
                        .takes
                        .iter()
                        .map(|take| crate::checkpoint::CheckpointTake::from_take(*track, take)),
                );
            }
            if let Err(e) = checkpoint.write(&self.dir)
                && !matches!(exit, Exit::Failed(_))
            {
                exit = Exit::Failed(format!("capture checkpoint: {e}"));
            }
        }
        if matches!(exit, Exit::Cancelled) {
            Checkpoint::remove(&self.dir);
        }
        let (mut state, mut reason) = match &exit {
            Exit::Stopped => {
                journal.record(
                    "stop",
                    None,
                    None,
                    Some(format!("duration_ms={duration_ms}")),
                );
                self.row.status = MeetingStatus::Stopped;
                (State::Stopped, None)
            }
            Exit::Cancelled => {
                journal.record("cancel", None, None, None);
                self.row.status = MeetingStatus::Cancelled;
                (State::Cancelled, Some("cancelled".to_string()))
            }
            Exit::Failed(why) => {
                journal.record("error", None, None, Some(why.clone()));
                self.row.status = MeetingStatus::Failed;
                self.row.error_code = Some("capture".into());
                self.row.error_message = Some(why.clone());
                (State::Failed, Some(why.clone()))
            }
        };
        if let Some(tail) = &live_tail
            && state == State::Stopped
        {
            // The live finals stand in for the transcript until the
            // finalisation replaces them.
            let mut finals = tail.finals.clone();
            finals.sort_by_key(|s| (s.start_ms, s.end_ms));
            self.row.segments = dettivo_transcribe::live::contract_segments(&finals);
            self.row.final_text = dettivo_transcribe::interleave::text_of(&finals);
            self.row.raw_text = self.row.final_text.clone();
        }
        if matches!(exit, Exit::Cancelled) {
            if let Err(e) = std::fs::remove_dir_all(&self.dir) {
                tracing::warn!(error = %e, "meeting: directory not removed");
            }
            self.row.audio_dir = None;
        }
        if let Err(e) = self.archive.updated(&self.row) {
            state = State::Failed;
            reason = Some(format!("storage: {e}"));
            self.row.status = MeetingStatus::Failed;
            self.row.error_code = Some("storage".into());
            self.row.error_message = reason.clone();
            if let Err(e) = self.archive.updated(&self.row) {
                tracing::warn!(error = %e, "meeting: failed capture row not stored");
            }
        }
        if !matches!(exit, Exit::Cancelled) {
            write_metadata(&self.row, &self.dir, &self.policy);
        }
        tracing::info!(job = %self.job_id, state = state.as_str(), duration_ms, takes = mic_takes, "meeting capture ended");
        self.set(shared, Some(state), reason, Some(mic_takes), Some(false));
        *shared.lock().unwrap_or_else(|p| p.into_inner()) = None;
        if state != State::Stopped {
            return;
        }
        finalize_meeting(FinalizeContext {
            job_id: self.job_id,
            row: self.row,
            dir: self.dir,
            policy: self.policy,
            engine: self.engine,
            publisher: self.publisher,
            archive: self.archive,
            finalizing: self.finalizing,
            previous: State::Stopped,
            capture_duration_ms: duration_ms,
        });
    }
}
