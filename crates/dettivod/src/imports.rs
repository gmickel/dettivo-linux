//! `transcripts.import`: an uploaded audio file is probed (the container
//! must match the declared type and the duration must fit `[history]
//! max_import_seconds`), becomes an `audioImport` item (the dictation
//! kind) or an imported meeting (the meeting kind, ADR 0027), and a job
//! decodes it to 16 kHz mono and transcribes it through the chunked
//! pipeline (ADR 0022); a Dettivo archive restores every item the profile
//! does not have yet. One import runs at a time.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use dettivo_audio::decode::{self, DecodeError};
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::transcripts::{ImportParams, ImportResult};
use dettivo_proto::runtime::{JobState, JobStatus, RefKind};
use dettivo_storage::export;
use dettivo_storage::item::{DictationItem, ItemStatus, SourceKind};
use dettivo_storage::meetings::{MeetingRow, MeetingStatus};
use dettivo_storage::retention::{AUDIO_FILE, ArtifactPolicy};

use crate::daemon::Daemon;
use crate::history::store_error;
use crate::jobs::{Decode, Jobs, Work, meeting_ref, running, settings_of, transcript_ref};
use crate::transfers::Upload;

impl Jobs {
    /// `transcripts.import` for the dictation kind: an archive restores at
    /// once, an audio file becomes an `audioImport` item and a job.
    pub fn start_import(
        self: &Arc<Self>,
        daemon: &Daemon,
        upload: Upload,
        params: &ImportParams,
    ) -> Result<ImportResult, JsonRpcError> {
        let n = self.import_seq.fetch_add(1, Ordering::Relaxed) + 1;
        let job_id = format!("job_import_{n}");
        // The slot is taken under the same lock that checks it, so two
        // imports arriving together cannot both pass; the guard frees it on
        // every path that does not hand the job to a worker.
        let slot = {
            let mut g = self
                .import_running
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            if g.is_some() {
                return Err(JsonRpcError::new(
                    AppCode::Conflict,
                    "Import already in progress",
                    ErrorDetails::conflict_kind("importAlreadyInProgress"),
                ));
            }
            *g = Some(job_id.clone());
            ImportSlot::new(self.clone(), job_id.clone())
        };
        let history = daemon.history();
        let filename = sanitize(&params.filename);
        let is_archive = upload.content_type == "application/zip" || filename.ends_with(".zip");
        if is_archive {
            let bytes = std::fs::read(&upload.path).map_err(|e| {
                JsonRpcError::new(AppCode::InternalError, e.to_string(), ErrorDetails::empty())
            })?;
            let retention = history.retention();
            let restored = history
                .with_store(|store| {
                    export::restore_zip(store, history.artifacts(), &retention, &bytes)
                })
                .map_err(store_error)?;
            daemon.transfers().finish_upload(&upload.id);
            let first = restored.first().cloned().ok_or_else(|| {
                JsonRpcError::new(
                    AppCode::InvalidParams,
                    "the archive holds no items this profile does not already have",
                    ErrorDetails::empty(),
                )
            })?;
            tracing::info!(job = %job_id, restored = restored.len(), "archive imported");
            return Ok(ImportResult {
                reference: transcript_ref(&first)?,
                is_partial: false,
                job: JobStatus {
                    job_id,
                    state: JobState::Succeeded,
                    progress: 1.0,
                    message: Some(format!("{} items restored", restored.len())),
                    error: None,
                },
            });
        }
        let mode = match params.mode {
            dettivo_proto::methods::dictation::DictationMode::Raw => "raw",
            other => {
                return Err(JsonRpcError::new(
                    AppCode::NotImplemented,
                    format!(
                        "mode {} is not available for an import; an import runs the raw layer (ADR 0023)",
                        other.as_str()
                    ),
                    ErrorDetails::empty(),
                ));
            }
        };
        let loaded = daemon.config();
        let provider = params
            .provider
            .clone()
            .unwrap_or_else(|| loaded.config.speech.provider.clone());
        let model = params
            .model
            .clone()
            .unwrap_or_else(|| loaded.config.speech.model.clone());
        let (engine, _) = daemon
            .engines()
            .engine_for(&daemon.paths, &loaded, &provider, &model)?;
        let max_seconds = loaded.config.history.max_import_seconds;
        let info = decode::probe(&upload.path, &upload.content_type).map_err(|e| {
            let code = match e {
                DecodeError::Io(_) | DecodeError::TooLong { .. } => AppCode::InternalError,
                _ => AppCode::InvalidParams,
            };
            JsonRpcError::new(code, e.to_string(), ErrorDetails::empty())
        })?;
        if info.duration_ms.is_some_and(|d| d > max_seconds * 1000) {
            return Err(JsonRpcError::new(
                AppCode::InvalidParams,
                DecodeError::TooLong { max_seconds }.to_string(),
                ErrorDetails::empty(),
            ));
        }
        let mut item = DictationItem::new(SourceKind::AudioImport);
        item.status = ItemStatus::Transcribing;
        item.title = filename
            .rsplit_once('.')
            .map(|(stem, _)| stem.to_string())
            .unwrap_or_else(|| filename.clone());
        item.mode = mode.into();
        item.stt_provider = provider;
        item.stt_model = model;
        item.language = params.language.clone();
        item.duration_ms = info.duration_ms.unwrap_or(0);
        // A meeting import keeps its audio under the meeting directory per
        // `[meetings]`; a dictation import per `[history]`.
        let meeting = (params.target_kind == RefKind::Meeting).then(|| {
            let mut row = MeetingRow::new();
            row.id = item.id.clone();
            row.status = MeetingStatus::Transcribing;
            row.source_kind = "audioImport".into();
            row.title = item.title.clone();
            row.title_source = "auto".into();
            row.language = item.language.clone();
            row.stt_provider = item.stt_provider.clone();
            row.stt_model = item.stt_model.clone();
            row.duration_ms = item.duration_ms;
            row.original_filename = Some(filename.clone());
            row.disclosure_acknowledged_at = daemon.meetings().disclosure().acknowledged_at;
            if params.expected_speakers.is_some() || params.diarize.is_some() {
                row.diarization = Some(dettivo_proto::methods::speakers::Diarization {
                    expected_speakers: params.expected_speakers,
                    auto: params.diarize,
                    ..Default::default()
                });
            }
            row
        });
        let retention = history.retention();
        let keep = match &meeting {
            Some(_) => {
                let m = &loaded.config.meetings;
                m.keep_audio && ArtifactPolicy::parse(&m.artifacts) != Some(ArtifactPolicy::None)
            }
            None => retention.keep_audio && retention.artifacts != ArtifactPolicy::None,
        };
        let audio = match (&meeting, keep) {
            (Some(row), _) => history.meeting_artifacts().dir(&row.id).join(AUDIO_FILE),
            (None, true) => history.artifacts().dir(&item.id).join(AUDIO_FILE),
            (_, false) => upload.path.with_extension("wav"),
        };
        let meeting = match meeting {
            Some(mut row) => {
                row.audio_dir = Some(
                    history
                        .meeting_artifacts()
                        .dir(&row.id)
                        .to_string_lossy()
                        .into_owned(),
                );
                history.begin_meeting(&row).map_err(store_error)?;
                if let Some(analyze) = params.analyze {
                    daemon.analysis().set_override(&row.id, analyze);
                }
                Some(row)
            }
            None => {
                history.begin_item(&item).map_err(store_error)?;
                None
            }
        };
        let reference = match &meeting {
            Some(row) => meeting_ref(&row.id)?,
            None => transcript_ref(&item.id)?,
        };
        let source = daemon.transfers().take_upload(&upload.id);
        let d = &loaded.config.dictation;
        let work = Work {
            job_id: job_id.clone(),
            item: item.clone(),
            audio,
            discard_audio: meeting.is_none() && !keep,
            decode: Some(Decode {
                source,
                content_type: upload.content_type.clone(),
                max_seconds,
            }),
            engine,
            language: if params.language.is_empty() {
                "auto".into()
            } else {
                params.language.clone()
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
            meeting: meeting.clone(),
            meeting_retention: meeting.as_ref().map(|_| {
                (
                    loaded.config.meetings.keep_audio,
                    ArtifactPolicy::parse(&loaded.config.meetings.artifacts)
                        .unwrap_or(ArtifactPolicy::Keep),
                )
            }),
            archive: Some(daemon.meeting_archive().clone()),
        };
        self.spawn(daemon, work)?;
        // The worker owns the slot from here and frees it when it finishes.
        slot.keep();
        tracing::info!(
            job = %job_id,
            container = %info.container,
            seconds = info.duration_ms.unwrap_or(0) / 1000,
            kind = if meeting.is_some() { "meeting" } else { "dictation" },
            "audio import started"
        );
        // The contract: a meeting import answers once the meeting is
        // visible with the handoff running (`is_partial = false`); a
        // dictation import reports the in-flight item as partial.
        Ok(ImportResult {
            reference,
            is_partial: meeting.is_none(),
            job: running(&job_id),
        })
    }
}

/// A file name reduced to its last path segment with no control characters.
fn sanitize(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let clean: String = base.chars().filter(|c| !c.is_control()).collect();
    if clean.is_empty() {
        "import".to_string()
    } else {
        clean
    }
}

/// Holds the single import slot until the job is handed to a worker.
struct ImportSlot {
    jobs: Arc<Jobs>,
    job_id: String,
    /// Cleared by `keep`: the worker frees the slot, the guard only drops
    /// its fields.
    armed: bool,
}

impl ImportSlot {
    fn new(jobs: Arc<Jobs>, job_id: String) -> Self {
        Self {
            jobs,
            job_id,
            armed: true,
        }
    }

    /// The worker frees the slot at the end of the job instead.
    fn keep(mut self) {
        self.armed = false;
    }
}

impl Drop for ImportSlot {
    fn drop(&mut self) {
        if self.armed {
            self.jobs.release_import(&self.job_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// daemon/F18: a kept slot stays taken for the worker, and the guard's
    /// `Arc` is dropped rather than forgotten.
    #[test]
    fn a_kept_slot_drops_its_guard_and_leaves_the_slot_to_the_worker() {
        let jobs = Arc::new(Jobs::default());
        *jobs.import_running.lock().unwrap() = Some("job_import_1".into());
        ImportSlot::new(jobs.clone(), "job_import_1".into()).keep();
        assert_eq!(Arc::strong_count(&jobs), 1, "the guard's Arc is released");
        assert!(jobs.import_in_progress(), "the worker still owns the slot");
        drop(ImportSlot::new(jobs.clone(), "job_import_1".into()));
        assert!(!jobs.import_in_progress(), "an armed guard frees the slot");
    }

    #[test]
    fn file_names_are_reduced_to_their_last_segment() {
        assert_eq!(sanitize("../../etc/passwd"), "passwd");
        assert_eq!(sanitize("C:\\Users\\x\\call.m4a"), "call.m4a");
        assert_eq!(sanitize("a\u{7}b.wav"), "ab.wav");
        assert_eq!(sanitize(""), "import");
    }
}
