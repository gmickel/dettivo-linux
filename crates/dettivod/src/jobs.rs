//! Background jobs over the store: an import decodes an upload and a
//! re-run reads an item's retained audio, and both go through the chunked
//! pipeline (`dettivo-transcribe`, ADR 0022) in a thread of their own
//! (`jobs_run`). Each job writes its item as `transcribing` first, reports
//! through `job.progress` with the chunk counts and the stage, can be
//! cancelled between chunks through `transcripts.cancel`, and ends the
//! item `completed`, `failed` with the reason, or cancelled. Imports
//! start in `imports.rs` on the same service.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::events::{JobProgressPayload, Topic};
use dettivo_proto::id::Id;
use dettivo_proto::methods::transcripts::{CancelResult, RerunResult};
use dettivo_proto::runtime::{JobState, JobStatus, RefKind, TranscriptRef};
use dettivo_speech::SttEngine;
use dettivo_storage::item::{DictationItem, ItemStatus, SourceKind};
use dettivo_storage::meetings::MeetingRow;
use dettivo_transcribe::Stage;
use serde_json::{Map, Value, json};

use crate::daemon::Daemon;
use crate::events::EventBus;
use crate::history::store_error;

#[path = "jobs_retry.rs"]
mod retry;

/// The overrides `transcripts.rerun` accepts.
#[derive(Debug, Clone, Default)]
pub struct Overrides {
    /// The provider; absent means the item's.
    pub provider: Option<String>,
    /// The model; absent means the selection in force.
    pub model: Option<String>,
    /// The mode; absent means `raw`.
    pub mode: Option<String>,
    /// The language; absent means the item's.
    pub language: Option<String>,
}

/// One job in flight.
pub(crate) struct Running {
    pub(crate) status: JobStatus,
    pub(crate) item_id: String,
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) settling: bool,
}

/// The service.
#[derive(Default)]
pub struct Jobs {
    rerun_seq: AtomicU64,
    pub(crate) import_seq: AtomicU64,
    pub(crate) import_running: Mutex<Option<String>>,
    pub(crate) running: Mutex<HashMap<String, Running>>,
}

/// An upload the job decodes before it transcribes.
pub(crate) struct Decode {
    /// The spooled upload; removed when the job ends.
    pub(crate) source: PathBuf,
    /// Its declared content type.
    pub(crate) content_type: String,
    /// `[history] max_import_seconds`.
    pub(crate) max_seconds: u64,
}

/// What one transcription job needs.
pub(crate) struct Work {
    pub(crate) job_id: String,
    pub(crate) item: DictationItem,
    /// The 16 kHz mono WAV the job reads (written first when `decode`).
    pub(crate) audio: PathBuf,
    /// Remove `audio` when the job ends (the item keeps no audio).
    pub(crate) discard_audio: bool,
    pub(crate) decode: Option<Decode>,
    pub(crate) engine: Arc<dyn SttEngine>,
    pub(crate) language: String,
    pub(crate) prompt: Option<String>,
    pub(crate) replacements: Vec<(String, String)>,
    pub(crate) spoken_punctuation: bool,
    pub(crate) protect_tokens: bool,
    pub(crate) settings: dettivo_transcribe::Settings,
    pub(crate) cancel: Arc<AtomicBool>,
    /// The meeting the job fills instead of a dictation item (the meeting
    /// kind of `transcripts.import`, ADR 0027).
    pub(crate) meeting: Option<MeetingRow>,
    /// Retention frozen at import or recovery start.
    pub(crate) meeting_retention: Option<(bool, dettivo_storage::retention::ArtifactPolicy)>,
    /// The archive a finished meeting goes through (the polished segments
    /// and the analysis, ADR 0036).
    pub(crate) archive: Option<Arc<dyn dettivo_meeting::Archive>>,
}

pub(crate) fn transcript_ref(id: &str) -> Result<TranscriptRef, JsonRpcError> {
    Ok(TranscriptRef {
        kind: RefKind::Dictation,
        id: Id::new(id).map_err(|e| {
            JsonRpcError::new(AppCode::InternalError, e.to_string(), ErrorDetails::empty())
        })?,
    })
}

pub(crate) fn meeting_ref(id: &str) -> Result<TranscriptRef, JsonRpcError> {
    Ok(TranscriptRef {
        kind: RefKind::Meeting,
        id: Id::new(id).map_err(|e| {
            JsonRpcError::new(AppCode::InternalError, e.to_string(), ErrorDetails::empty())
        })?,
    })
}

pub(crate) fn running(job_id: &str) -> JobStatus {
    JobStatus {
        job_id: job_id.to_string(),
        state: JobState::Running,
        progress: 0.0,
        message: Some("transcribing".into()),
        error: None,
    }
}

/// `[transcribe]` as the pipeline reads it.
pub(crate) fn settings_of(loaded: &dettivo_core::config::Loaded) -> dettivo_transcribe::Settings {
    let t = &loaded.config.transcribe;
    dettivo_transcribe::Settings {
        chunk_seconds: t.chunk_seconds,
        overlap_seconds: t.overlap_seconds,
        safety_margin_seconds: t.safety_margin_seconds,
        silence_rms_floor: t.silence_rms_floor,
        filler_filter: t.filler_filter,
    }
}

impl Jobs {
    /// Frees the import slot when `job_id` holds it; a poisoned lock is
    /// recovered so the slot can never stay taken.
    pub(crate) fn release_import(&self, job_id: &str) {
        let mut g = self
            .import_running
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if g.as_deref() == Some(job_id) {
            *g = None;
        }
    }

    /// True while an import job runs.
    pub fn import_in_progress(&self) -> bool {
        self.import_running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_some()
    }

    /// Jobs still running (for `system.health`).
    pub fn active(&self) -> usize {
        self.running.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    /// Publishes `job.progress` and keeps the job's status current.
    pub(crate) fn publish(
        &self,
        bus: &EventBus,
        job_id: &str,
        progress: f64,
        chunks: Option<(u32, u32)>,
        stage: Stage,
    ) {
        if let Some(r) = self
            .running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get_mut(job_id)
        {
            r.status.progress = progress;
            r.status.message = Some(stage.as_str().to_string());
        }
        bus.publish(
            Topic::JobProgress,
            json!(JobProgressPayload {
                job_id: job_id.to_string(),
                progress,
                chunks_done: chunks.map(|c| c.0),
                chunks_total: chunks.map(|c| c.1),
                stage: Some(stage.as_str().to_string()),
            }),
        );
    }

    /// Forgets a finished job.
    pub(crate) fn finished(&self, job_id: &str) {
        self.running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(job_id);
        self.release_import(job_id);
    }

    /// Freezes cancellation before persistence without holding the registry
    /// lock across the write; later requests are refused as too late.
    pub(crate) fn begin_settlement(&self, job_id: &str) -> Option<bool> {
        let mut jobs = self.running.lock().unwrap_or_else(|p| p.into_inner());
        let job = jobs.get_mut(job_id)?;
        job.settling = true;
        Some(job.cancel.load(Ordering::SeqCst))
    }

    /// `transcripts.cancel`: the job working on `item_id` (a dictation
    /// item or a meeting) stops after its current chunk.
    pub fn cancel(&self, item_id: &str, kind: RefKind) -> Result<CancelResult, JsonRpcError> {
        let g = self.running.lock().unwrap_or_else(|p| p.into_inner());
        let Some((_, job)) = g.iter().find(|(_, r)| r.item_id == item_id) else {
            return Err(JsonRpcError::new(
                AppCode::NotFound,
                format!(
                    "no job is working on {} {item_id}",
                    if kind == RefKind::Meeting {
                        "meeting"
                    } else {
                        "dictation"
                    }
                ),
                ErrorDetails::empty(),
            ));
        };
        if job.settling {
            return Err(JsonRpcError::new(
                AppCode::Conflict,
                "the job is committing its result; cancellation is too late",
                ErrorDetails::conflict_kind("jobSettling"),
            ));
        }
        job.cancel.store(true, Ordering::SeqCst);
        tracing::info!(job = %job.status.job_id, "job cancel requested");
        Ok(CancelResult {
            reference: if kind == RefKind::Meeting {
                meeting_ref(item_id)?
            } else {
                transcript_ref(item_id)?
            },
            job: JobStatus {
                state: JobState::Cancelled,
                message: Some("cancelled".into()),
                ..job.status.clone()
            },
        })
    }

    pub(crate) fn spawn(self: &Arc<Self>, daemon: &Daemon, work: Work) -> Result<(), JsonRpcError> {
        let jobs = self.clone();
        let history = daemon.history().clone();
        let bus = daemon.bus().clone();
        self.running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                work.job_id.clone(),
                Running {
                    status: running(&work.job_id),
                    item_id: work.item.id.clone(),
                    cancel: work.cancel.clone(),
                    settling: false,
                },
            );
        let name = work.job_id.clone();
        std::thread::Builder::new()
            .name(name.clone())
            .spawn(move || crate::jobs_run::run(&jobs, &history, &bus, work))
            .map(|_| ())
            .map_err(|e| {
                self.finished(&name);
                JsonRpcError::new(
                    AppCode::InternalError,
                    format!("cannot start the job thread: {e}"),
                    ErrorDetails::empty(),
                )
            })
    }

    /// `transcripts.rerun`: the retained audio through the same job.
    pub fn start_rerun(
        self: &Arc<Self>,
        daemon: &Daemon,
        original: DictationItem,
        overrides: Overrides,
    ) -> Result<RerunResult, JsonRpcError> {
        let audio = original
            .audio_path
            .as_deref()
            .map(PathBuf::from)
            .filter(|p| p.is_file())
            .ok_or_else(|| {
                let mut details = Map::new();
                details.insert("reason".into(), Value::String("audio_not_retained".into()));
                JsonRpcError::new(
                    AppCode::NotFound,
                    format!("dictation {} has no retained audio", original.id),
                    ErrorDetails(details),
                )
            })?;
        let mode = overrides.mode.clone().unwrap_or_else(|| "raw".into());
        if mode != "raw" {
            return Err(JsonRpcError::new(
                AppCode::NotImplemented,
                format!(
                    "mode {mode} is not available for a rerun; a rerun runs the raw layer (ADR 0023)"
                ),
                ErrorDetails::empty(),
            ));
        }
        let loaded = daemon.config();
        let provider = overrides
            .provider
            .clone()
            .unwrap_or_else(|| original.stt_provider.clone());
        let model = overrides
            .model
            .clone()
            .unwrap_or_else(|| loaded.config.speech.model.clone());
        let (engine, _) = daemon
            .engines()
            .engine_for(&daemon.paths, &loaded, &provider, &model)?;
        let history = daemon.history();
        let n = self.rerun_seq.fetch_add(1, Ordering::Relaxed) + 1;
        let job_id = format!("job_rerun_{n}");
        let mut item = DictationItem::new(SourceKind::Rerun);
        item.status = ItemStatus::Transcribing;
        item.rerun_of_item_id = Some(original.id.clone());
        item.app_id = original.app_id.clone();
        item.app_name = original.app_name.clone();
        item.mode = mode;
        item.stt_provider = provider;
        item.stt_model = model;
        item.language = overrides
            .language
            .clone()
            .unwrap_or_else(|| original.language.clone());
        item.duration_ms = original.duration_ms;
        let mut source = audio.clone();
        if history.retention().keep_audio {
            match history.artifacts().adopt_audio(&item.id, &audio) {
                Ok(p) => {
                    item.audio_path = Some(p.to_string_lossy().into_owned());
                    source = p;
                }
                Err(e) => tracing::warn!(error = %e, "re-run audio not copied"),
            }
        }
        history.begin_item(&item).map_err(store_error)?;
        history.mark_busy(&original.id);
        let d = &loaded.config.dictation;
        let work = Work {
            job_id: job_id.clone(),
            item: item.clone(),
            audio: source,
            discard_audio: false,
            decode: None,
            engine,
            language: if item.language.is_empty() {
                "auto".into()
            } else {
                item.language.clone()
            },
            prompt: (!d.vocabulary.is_empty()).then(|| d.vocabulary.join(", ")),
            replacements: d
                .replacements
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            spoken_punctuation: d.spoken_punctuation,
            protect_tokens: d.protect_tokens,
            settings: settings_of(&loaded),
            cancel: Arc::new(AtomicBool::new(false)),
            meeting: None,
            meeting_retention: None,
            archive: None,
        };
        self.spawn(daemon, work)?;
        Ok(RerunResult {
            reference: transcript_ref(&item.id)?,
            rerun_of: transcript_ref(&original.id)?,
            job: running(&job_id),
        })
    }
}
