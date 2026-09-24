//! The post-meeting speaker pass as the daemon runs it (ADR 0035): one
//! job per completed meeting, started by itself when a finalisation lands
//! `completed` and `[meetings.diarization] auto` is on, or by
//! `meetings.diarize`. The finalisation marks the pass it plans `queued`
//! on the row through `planned` (ADR 0061), and `auto_start` acts on the
//! same answer once the row is stored; a plan that cannot launch is
//! settled `failed`, never left queued. The job (`diarization_run.rs`)
//! reads the diarized track from the meeting directory, sends it through
//! `dettivo-engine-diarize` over the supervisor, labels the segments
//! under the coverage and share rule, stores the speakers with the
//! diarization block on the row, and reports through `job.progress`
//! (`stage = diarizing`) and `meeting.state` (`diarization_status`). A
//! missing model set leaves `unavailable` with the download command; a
//! crash leaves `failed` with the engine's last redacted line; the
//! meeting stays `completed` either way. The pass commits its result
//! through the meeting's job entry (`meeting_jobs`): a `meetings.cancel`
//! lets it record the cancelled block, a delete invalidates it and the
//! result is dropped.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use dettivo_core::config::Loaded;
use dettivo_core::config::audio_schema::Diarization as DiarizationConfig;
use dettivo_meeting::diarize::Rule;
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::events::{JobProgressPayload, MeetingLiveState, MeetingStatePayload, Topic};
use dettivo_proto::id::Id;
use dettivo_proto::methods::speakers::DiarizationStatus;
use dettivo_proto::runtime::{JobState, JobStatus};
use dettivo_speech::diarize::DiarizeEngine;
use dettivo_speech::models::Readiness;
use dettivo_storage::meetings::{MeetingRow, MeetingStatus};
use serde_json::{Map, Value, json};

use crate::daemon::Daemon;
use crate::events::EventBus;
use crate::history::History;
use crate::meeting_jobs::{Job, MeetingJobs};

/// The `job.progress` stage while the pass runs.
pub const STAGE: &str = "diarizing";

/// The service.
#[derive(Default)]
pub struct Diarization {
    seq: AtomicU64,
    jobs: MeetingJobs,
}

/// What one pass needs, gathered on the caller's thread.
pub(crate) struct Pass {
    pub(crate) job_id: String,
    pub(crate) row: MeetingRow,
    pub(crate) audio_dir: Option<String>,
    pub(crate) engine: DiarizeEngine,
    /// The catalogue id of the model set.
    pub(crate) model_id: String,
    pub(crate) rule: Rule,
    pub(crate) speakers: Option<u32>,
    pub(crate) clustering_threshold: f64,
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) history: Arc<History>,
    pub(crate) bus: Arc<EventBus>,
}

fn conflict(message: String, kind: &str) -> JsonRpcError {
    JsonRpcError::new(
        AppCode::Conflict,
        message,
        ErrorDetails::conflict_kind(kind),
    )
}

/// The `meetings.diarize` refusal for a model set that is not on disk,
/// naming the download command.
pub fn model_missing(model: &str) -> JsonRpcError {
    let mut details = Map::new();
    details.insert("reason".into(), Value::String("modelMissing".into()));
    JsonRpcError::new(
        AppCode::NotFound,
        format!(
            "model diarize/{model} is not downloaded; run `{}`",
            crate::engines::download_command("diarize", model)
        ),
        ErrorDetails(details),
    )
}

fn job(job_id: &str, chunks: (u32, u32)) -> JobStatus {
    JobStatus {
        job_id: job_id.to_string(),
        state: JobState::Running,
        progress: if chunks.1 == 0 {
            0.0
        } else {
            f64::from(chunks.0) / f64::from(chunks.1)
        },
        message: Some(STAGE.into()),
        error: None,
    }
}

impl Diarization {
    /// Whether the finalisation plans the pass on `row`: the pass is
    /// enabled and `auto` applies, the meeting's own override first. The
    /// archive marks the row `queued` from this answer and `auto_start`
    /// runs from the same one, so the two never disagree.
    pub(crate) fn planned(config: &DiarizationConfig, row: &MeetingRow) -> bool {
        config.enabled
            && row
                .diarization
                .as_ref()
                .and_then(|b| b.auto)
                .unwrap_or(config.auto)
    }

    /// The running pass on `meeting_id`, as a contract job.
    pub fn job_for(&self, meeting_id: &str) -> Option<JobStatus> {
        self.jobs.get(meeting_id).map(|r| job(&r.job_id, r.chunks))
    }

    /// Asks the running pass on `meeting_id` to stop; the pass records
    /// the cancelled block. True when one ran.
    pub fn cancel(&self, meeting_id: &str) -> bool {
        self.jobs.cancel(meeting_id)
    }

    /// Stops the pass on `meeting_id` and drops its result (a delete):
    /// nothing it computed reaches the row. Waits for a commit in
    /// progress. True when one ran.
    pub fn invalidate(&self, meeting_id: &str) -> bool {
        self.jobs.invalidate(meeting_id)
    }

    /// Passes in flight (for `system.health`).
    pub fn active(&self) -> usize {
        self.jobs.len()
    }

    /// The automatic run after a finalisation: silent unless `planned`
    /// says so and the meeting is completed. A launch that fails settles
    /// a block still `queued` to `failed` with the reason (the missing
    /// model set already wrote `unavailable`).
    pub fn auto_start(self: &Arc<Self>, daemon: &Daemon, row: MeetingRow) {
        let audio_dir = row.audio_dir;
        let Ok(Some(row)) = daemon.history().with_store(|s| s.get_meeting(&row.id)) else {
            return;
        };
        let loaded = daemon.config();
        if !Self::planned(&loaded.config.meetings.diarization, &row)
            || row.status != MeetingStatus::Completed
        {
            return;
        }
        let id = row.id.clone();
        if let Err(e) = self.launch(daemon, row, None, true, audio_dir) {
            tracing::info!(error = %e.message, "diarization not started");
            self.settle_queued(daemon, &id, e.message);
        }
    }

    /// Marks a block the finalisation left `queued` as `failed` with
    /// `reason`, on the row and on the bus; any other status stays.
    fn settle_queued(&self, daemon: &Daemon, meeting_id: &str, reason: String) {
        let Ok(Some(mut row)) = daemon.history().with_store(|s| s.get_meeting(meeting_id)) else {
            return;
        };
        let Some(block) = row.diarization.as_mut() else {
            return;
        };
        if block.status != DiarizationStatus::Queued {
            return;
        }
        block.status = DiarizationStatus::Failed;
        block.error = Some(reason);
        if let Err(e) = daemon.history().with_store(|s| s.update_meeting(&row)) {
            tracing::warn!(error = %e, "history: diarization block not stored");
        }
        publish_state(daemon.bus(), &row, DiarizationStatus::Failed);
    }

    /// Starts a pass on `row` (`meetings.diarize`, or the automatic run);
    /// `speakers` overrides the meeting's expected count for this pass.
    pub fn start(
        self: &Arc<Self>,
        daemon: &Daemon,
        row: MeetingRow,
        speakers: Option<u32>,
    ) -> Result<JobStatus, JsonRpcError> {
        let audio_dir = row.audio_dir.clone();
        self.launch(daemon, row, speakers, false, audio_dir)
    }

    fn launch(
        self: &Arc<Self>,
        daemon: &Daemon,
        mut row: MeetingRow,
        speakers: Option<u32>,
        wait: bool,
        audio_dir: Option<String>,
    ) -> Result<JobStatus, JsonRpcError> {
        let loaded = daemon.config();
        let d = &loaded.config.meetings.diarization;
        if !d.enabled {
            return Err(conflict(
                "diarization is off ([meetings.diarization] enabled = false)".into(),
                "diarizationDisabled",
            ));
        }
        if row.status != MeetingStatus::Completed {
            return Err(conflict(
                format!(
                    "Meeting {} is not completed; it is {}",
                    row.id,
                    row.status.as_str()
                ),
                "meetingNotCompleted",
            ));
        }
        if let Some(running) = self.job_for(&row.id) {
            return Err(conflict(
                format!("Meeting {} is being diarized by {}", row.id, running.job_id),
                "diarizationRunning",
            ));
        }
        let engine = match self.engine(daemon, &loaded) {
            Ok(engine) => engine,
            Err(e) => {
                let mut block = row.diarization.clone().unwrap_or_default();
                block.status = DiarizationStatus::Unavailable;
                block.error = Some(e.message.clone());
                row.diarization = Some(block);
                if let Err(e) = daemon.history().with_store(|s| s.update_meeting(&row)) {
                    tracing::warn!(error = %e, "history: diarization block not stored");
                }
                publish_state(daemon.bus(), &row, DiarizationStatus::Unavailable);
                return Err(e);
            }
        };
        let n = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        let job_id = format!("job_diarize_{n}");
        let cancel = Arc::new(AtomicBool::new(false));
        let expected = speakers
            .or_else(|| row.diarization.as_ref().and_then(|b| b.expected_speakers))
            .or((d.max_speakers > 0).then_some(d.max_speakers));
        let pass = Pass {
            job_id: job_id.clone(),
            row: row.clone(),
            audio_dir,
            engine,
            model_id: d.model.clone(),
            rule: Rule {
                min_coverage: d.min_coverage,
                min_speaker_share: d.min_speaker_share,
            },
            speakers: expected,
            clustering_threshold: d.clustering_threshold,
            cancel: cancel.clone(),
            history: daemon.history().clone(),
            bus: daemon.bus().clone(),
        };
        self.jobs.insert(
            &row.id,
            Job {
                job_id: job_id.clone(),
                cancel,
                chunks: (0, 0),
            },
        );
        let service = self.clone();
        let meeting_id = row.id.clone();
        let own_job = job_id.clone();
        let worker = std::thread::Builder::new()
            .name(job_id.clone())
            .spawn(move || {
                crate::diarization_run::run(&service, pass);
                service.jobs.remove(&meeting_id, &own_job);
            })
            .map_err(|e| {
                self.jobs.remove(&row.id, &job_id);
                JsonRpcError::new(
                    AppCode::InternalError,
                    format!("cannot start the job thread: {e}"),
                    ErrorDetails::empty(),
                )
            })?;
        if wait && worker.join().is_err() {
            self.jobs.remove(&row.id, &job_id);
            tracing::warn!(job = %job_id, "automatic diarization worker panicked");
        }
        tracing::info!(job = %job_id, "diarization started");
        Ok(job(&job_id, (0, 0)))
    }

    /// The engine for the configured model set, or the refusal naming the
    /// download command when it is not on disk.
    fn engine(&self, daemon: &Daemon, loaded: &Loaded) -> Result<DiarizeEngine, JsonRpcError> {
        let model = &loaded.config.meetings.diarization.model;
        let store = daemon.models().store();
        let ready = store
            .catalogue()
            .find("diarize", model)
            .map(|entry| (store.readiness(entry), store.load_path(entry)));
        match ready {
            Some((Readiness::Ready | Readiness::Unverified, dir)) => {
                // Verified, or hashed now; a set that fails is quarantined
                // and refused rather than opened.
                daemon.models().verifier().ensure(&dir).map_err(|why| {
                    JsonRpcError::new(AppCode::NotFound, why, ErrorDetails::empty())
                })?;
                let backend = match loaded.config.engines.diarize.backend {
                    dettivo_core::config::schema::DiarizeBackend::Auto => {
                        dettivo_engine_proto::BackendPreference::Auto
                    }
                    dettivo_core::config::schema::DiarizeBackend::Cpu => {
                        dettivo_engine_proto::BackendPreference::Cpu
                    }
                    dettivo_core::config::schema::DiarizeBackend::Cuda => {
                        dettivo_engine_proto::BackendPreference::Cuda
                    }
                };
                Ok(DiarizeEngine::new(
                    daemon.engines().supervisor(),
                    dir.to_string_lossy().into_owned(),
                    Some(loaded.config.engines.diarize.threads),
                )
                .with_backend(backend))
            }
            _ => Err(model_missing(model)),
        }
    }

    pub(crate) fn note_progress(&self, meeting_id: &str, chunks: (u32, u32)) {
        self.jobs.progress(meeting_id, chunks);
    }

    /// Runs `f` while the pass still owns the meeting's job entry; `None`
    /// when a delete invalidated it.
    pub(crate) fn commit<T>(
        &self,
        meeting_id: &str,
        job_id: &str,
        f: impl FnOnce() -> T,
    ) -> Option<T> {
        self.jobs.commit(meeting_id, job_id, f)
    }
}

pub(crate) fn publish_progress(bus: &EventBus, job_id: &str, chunks: (u32, u32), stage: &str) {
    bus.publish(
        Topic::JobProgress,
        json!(JobProgressPayload {
            job_id: job_id.to_string(),
            progress: job(job_id, chunks).progress,
            chunks_done: Some(chunks.0),
            chunks_total: Some(chunks.1),
            stage: Some(stage.to_string()),
        }),
    );
}

/// `meeting.state` on the completed meeting with the pass's status.
pub(crate) fn publish_state(bus: &EventBus, row: &MeetingRow, status: DiarizationStatus) {
    let Ok(meeting_id) = Id::new(row.id.clone()) else {
        return;
    };
    bus.publish(
        Topic::MeetingState,
        json!(MeetingStatePayload {
            kind: "meeting".into(),
            state: MeetingLiveState::Completed,
            meeting_id,
            live_segment_count: row.segments.len() as u64,
            live_last_end_ms: row.segments.last().map(|s| s.end_ms).unwrap_or(0),
            is_finalizing: false,
            previous_state: Some(MeetingLiveState::Completed),
            reason: None,
            duration_ms: Some(row.duration_ms),
            microphone_takes: Some(row.microphone_takes),
            system_audio: Some(row.system_audio),
            diarization_status: Some(status),
            analysis_status: None,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use dettivo_proto::methods::speakers::Diarization as Block;

    /// The plan follows the meeting's own `auto` first, then the
    /// configuration, and is never made while the pass is off.
    #[test]
    fn the_plan_follows_the_override_then_the_configuration() {
        let mut config = DiarizationConfig::default();
        let mut row = MeetingRow::new();
        assert!(
            Diarization::planned(&config, &row),
            "defaults plan the pass"
        );
        config.auto = false;
        assert!(!Diarization::planned(&config, &row));
        row.diarization = Some(Block {
            auto: Some(true),
            ..Default::default()
        });
        assert!(
            Diarization::planned(&config, &row),
            "the meeting's own choice wins"
        );
        config.enabled = false;
        assert!(!Diarization::planned(&config, &row), "off is off");
        config.enabled = true;
        config.auto = true;
        row.diarization = Some(Block {
            auto: Some(false),
            ..Default::default()
        });
        assert!(!Diarization::planned(&config, &row));
    }
}
