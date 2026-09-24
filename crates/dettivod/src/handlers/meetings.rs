//! `meetings.start`, `stop`, `cancel` and `status` per the contract (ADR
//! 0027, ADR 0030); `list`, `get` and `search` are in `meetings_read`,
//! `delete`, the recovery verbs and the disclosure methods in
//! `meetings_recovery`, the speaker methods in `meetings_speakers` and the
//! notes and analysis verbs in `meetings_notes` (ADR 0036). The gates
//! run in `start` in this order: one session at a time, the disclosure,
//! the meeting-capable engine, the downloaded model, then the capture.

pub(super) use super::{json, params};
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::id::Id;
use dettivo_proto::methods::meetings::{
    CancelResult, CaptureStatus, MeetingIdParams, Recoverable, StartParams, StartResult,
    StatusResult, StopResult,
};
use dettivo_proto::runtime::{HistoryStatus, JobState, JobStatus, RefKind, TranscriptRef};
use dettivo_speech::engines::{capabilities_of, meeting_capable};
use dettivo_storage::meeting_reads::MeetingCapture;
use dettivo_storage::meetings::{MeetingRow, MeetingStatus};
use serde_json::{Map, Value};

use crate::daemon::Daemon;
use crate::history::store_error;

pub(super) fn conflict(message: &str, kind: &str) -> JsonRpcError {
    JsonRpcError::new(
        AppCode::Conflict,
        message,
        ErrorDetails::conflict_kind(kind),
    )
}

/// The contract reference of a meeting.
pub fn reference(id: &str) -> Result<TranscriptRef, JsonRpcError> {
    Ok(TranscriptRef {
        kind: RefKind::Meeting,
        id: Id::new(id).map_err(|e| {
            JsonRpcError::new(AppCode::InternalError, e.to_string(), ErrorDetails::empty())
        })?,
    })
}

/// The wire status of a row.
pub fn history_status(status: MeetingStatus) -> HistoryStatus {
    match status {
        MeetingStatus::Recording => HistoryStatus::Recording,
        MeetingStatus::Stopping => HistoryStatus::Stopping,
        MeetingStatus::Stopped => HistoryStatus::Stopped,
        MeetingStatus::Transcribing => HistoryStatus::Transcribing,
        MeetingStatus::Completed => HistoryStatus::Completed,
        MeetingStatus::Failed => HistoryStatus::Failed,
        MeetingStatus::Cancelled => HistoryStatus::Cancelled,
        MeetingStatus::Partial => HistoryStatus::Partial,
    }
}

/// The contract job for a row that is not the active meeting.
fn row_job(
    status: MeetingStatus,
    error_message: Option<&str>,
    recovery_reason: Option<&str>,
    job_id: &str,
) -> JobStatus {
    let (state, progress, message) = match status {
        MeetingStatus::Recording => (JobState::Running, 0.0, Some("recording")),
        MeetingStatus::Stopping => (JobState::Running, 0.0, Some("stopping")),
        MeetingStatus::Transcribing => (JobState::Running, 0.0, Some("transcribing")),
        MeetingStatus::Stopped => (JobState::Succeeded, 1.0, Some("stopped")),
        MeetingStatus::Completed => (JobState::Succeeded, 1.0, None),
        MeetingStatus::Failed => (JobState::Failed, 0.0, None),
        MeetingStatus::Cancelled => (JobState::Cancelled, 0.0, None),
        MeetingStatus::Partial => (JobState::Failed, 0.0, Some("partial")),
    };
    JobStatus {
        job_id: job_id.to_string(),
        state,
        progress,
        message: message.map(str::to_string),
        error: error_message.map(str::to_string).or_else(|| {
            (status == MeetingStatus::Partial)
                .then(|| recovery_reason.unwrap_or_default().to_string())
        }),
    }
}

/// One meeting by id, or the contract's `NOT_FOUND`.
pub fn lookup(daemon: &Daemon, id: &str) -> Result<MeetingRow, JsonRpcError> {
    daemon
        .history()
        .with_store(|store| store.get_meeting(id))
        .map_err(store_error)?
        .ok_or_else(|| {
            JsonRpcError::new(
                AppCode::NotFound,
                "Meeting not found",
                ErrorDetails::empty(),
            )
        })
}

fn capture_of(
    row: &MeetingCapture,
    live: Option<&dettivo_meeting::machine::Snapshot>,
    finalizing: Option<&dettivo_meeting::machine::Finalizing>,
) -> CaptureStatus {
    CaptureStatus {
        duration_ms: live
            .map(|s| s.started.elapsed().as_millis() as u64)
            .unwrap_or(row.duration_ms),
        microphone_takes: live
            .map(|s| s.microphone_takes)
            .unwrap_or(row.microphone_takes),
        system_audio: live.map(|s| s.system_audio).unwrap_or(row.system_audio),
        is_partial: row.is_partial,
        chunks_completed: finalizing
            .map(|f| f.chunks_completed)
            .unwrap_or(row.chunks_completed),
        chunks_total: finalizing
            .map(|f| f.chunks_total)
            .unwrap_or(row.chunks_total),
        reason: row.recovery_reason.clone(),
    }
}

/// The running finalisation as a contract job.
pub fn finalizing_job(f: &dettivo_meeting::machine::Finalizing) -> JobStatus {
    JobStatus {
        job_id: f.job_id.clone(),
        state: JobState::Running,
        progress: if f.chunks_total == 0 {
            0.0
        } else {
            f64::from(f.chunks_completed) / f64::from(f.chunks_total)
        },
        message: Some("transcribing".into()),
        error: None,
    }
}

/// Partial captures and retained failed finalizations, for `meetings.status`.
fn recoverable(daemon: &Daemon) -> Result<Vec<Recoverable>, JsonRpcError> {
    daemon
        .history()
        .with_store(|s| {
            s.meeting_captures_with_status(&[
                MeetingStatus::Partial,
                MeetingStatus::Failed,
                MeetingStatus::Cancelled,
                MeetingStatus::Stopped,
            ])
        })
        .map_err(store_error)?
        .iter()
        .filter(|row| {
            row.status == MeetingStatus::Partial
                || super::meetings_recovery::recovery_audio_for(
                    daemon,
                    &row.id,
                    &row.source_kind,
                    row.system_audio,
                )
                .is_ok()
        })
        .map(|row| {
            Ok(Recoverable {
                reference: reference(&row.id)?,
                title: row.title.clone(),
                started_at: row.started_at.clone(),
                duration_ms: row.duration_ms,
                chunks_completed: row.chunks_completed,
                chunks_total: row.chunks_total,
                reason: row.recovery_reason.clone().unwrap_or_default(),
            })
        })
        .collect()
}

/// The gates and the capture behind `meetings.start`.
pub fn start(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: StartParams = params(params_value)?;
    let _reservation = daemon.capture_start_lock();
    if daemon.dictation().snapshot().is_some() {
        return Err(conflict("A dictation session is active", "sessionActive"));
    }
    if daemon.meetings().snapshot().is_some() {
        return Err(conflict("A meeting session is active", "sessionActive"));
    }
    let disclosure = if p.acknowledge_meeting_disclosure == Some(true) {
        daemon
            .meetings()
            .acknowledge()
            .map_err(|e| JsonRpcError::new(AppCode::InternalError, e, ErrorDetails::empty()))?
    } else {
        daemon.meetings().disclosure()
    };
    if !disclosure.acknowledged {
        return Err(conflict(
            "Meeting disclosure acknowledgement required",
            "meetingDisclosureRequired",
        ));
    }
    let loaded = daemon.config();
    let provider = if loaded.config.speech.meeting_model.is_empty() {
        loaded.config.speech.provider.clone()
    } else {
        "whisper".to_string()
    };
    if !meeting_capable(&provider) || !capabilities_of(&provider).supports_timestamps {
        let mut details = Map::new();
        details.insert(
            "kind".into(),
            Value::String("engineWithoutTimestamps".into()),
        );
        details.insert("provider".into(), Value::String(provider.clone()));
        details.insert("settings".into(), Value::String("settings.models".into()));
        return Err(JsonRpcError::new(
            AppCode::Conflict,
            format!(
                "Provider {provider} is not meeting-capable: its timestamps are not precise enough for the meeting merger; pick a meeting-capable provider under Settings > Models (dettivo app open settings.models)"
            ),
            ErrorDetails(details),
        ));
    }
    if !p.capture.microphone {
        return Err(JsonRpcError::new(
            AppCode::InvalidParams,
            "capture.microphone must be true: a meeting records the microphone; capture.system_audio may be off",
            ErrorDetails::empty(),
        ));
    }
    let mut row = MeetingRow::new();
    match p.title.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        Some(title) => {
            row.title = title.to_string();
            row.title_source = "manual".into();
        }
        None => row.title = MeetingRow::derived_title(&row.started_at),
    }
    row.language = p
        .language
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| loaded.config.dictation.language.clone());
    row.stt_provider = provider;
    let speech = &loaded.config.speech;
    row.stt_model = if speech.meeting_model.is_empty() {
        speech.model.clone()
    } else {
        speech.meeting_model.clone()
    };
    row.disclosure_acknowledged_at = disclosure.acknowledged_at;
    if p.expected_speakers.is_some() || p.diarize.is_some() {
        row.diarization = Some(dettivo_proto::methods::speakers::Diarization {
            expected_speakers: p.expected_speakers,
            auto: p.diarize,
            ..Default::default()
        });
    }
    // The engine is frozen at start (FR-G10): a model that is not on disk
    // is refused here naming the download command, never mid-meeting.
    let (engine, _) =
        daemon
            .engines()
            .engine_for(&daemon.paths, &loaded, &row.stt_provider, &row.stt_model)?;
    let snapshot = daemon
        .meetings()
        .start(
            daemon.config_handle(),
            daemon.audio().cloned(),
            engine,
            p.capture.system_audio,
            row,
        )
        .map_err(meeting_error)?;
    if let Some(analyze) = p.analyze {
        daemon
            .analysis()
            .set_override(&snapshot.meeting_id, analyze);
    }
    json(StartResult {
        reference: reference(&snapshot.meeting_id)?,
        job: JobStatus {
            job_id: snapshot.job_id,
            state: JobState::Running,
            progress: 0.0,
            message: None,
            error: None,
        },
    })
}

fn meeting_error(e: dettivo_meeting::MeetingError) -> JsonRpcError {
    use dettivo_meeting::MeetingError as E;
    let message = e.to_string();
    match e {
        E::Active => conflict(&message, "sessionActive"),
        E::NoSession => JsonRpcError::new(AppCode::NotFound, message, ErrorDetails::empty()),
        E::Audio(_) | E::Store(_) => {
            JsonRpcError::new(AppCode::InternalError, message, ErrorDetails::empty())
        }
        E::Settings(_) => JsonRpcError::new(AppCode::InvalidParams, message, ErrorDetails::empty()),
    }
}

/// `meetings.stop`: the active meeting answers the contract's
/// `transcribing` job at once (the takes close and the finalisation
/// follows; `meetings.status` shows `stopping` in between); a finalising
/// meeting answers its job; any other row answers its current state,
/// never an error.
pub fn stop(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: MeetingIdParams = params(params_value)?;
    let id = p.meeting_id.as_str();
    if let Some(live) = daemon.meetings().snapshot().filter(|s| s.meeting_id == id) {
        daemon.meetings().stop().map_err(meeting_error)?;
        return json(StopResult {
            reference: reference(id)?,
            job: JobStatus {
                job_id: live.job_id,
                state: JobState::Running,
                progress: 0.0,
                message: Some("transcribing".into()),
                error: None,
            },
        });
    }
    if let Some(f) = daemon.meetings().finalizing(id) {
        return json(StopResult {
            reference: reference(id)?,
            job: finalizing_job(&f),
        });
    }
    let row = lookup(daemon, id)?;
    json(StopResult {
        reference: reference(id)?,
        job: row_job(
            row.status,
            row.error_message.as_deref(),
            row.recovery_reason.as_deref(),
            "job_meeting",
        ),
    })
}

/// `meetings.cancel`: the recording meeting is dropped with its takes; a
/// captured finalisation stops between chunks and stays `stopped` with its
/// audio; an import or imported retry cancels its transcription job. A
/// completed one whose speaker pass runs stops the pass and
/// keeps its transcript (ADR 0035).
pub fn cancel(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: MeetingIdParams = params(params_value)?;
    let id = p.meeting_id.as_str();
    if let Some(live) = daemon.meetings().snapshot().filter(|s| s.meeting_id == id) {
        daemon.meetings().cancel().map_err(meeting_error)?;
        return json(CancelResult {
            reference: reference(id)?,
            job: JobStatus {
                job_id: live.job_id,
                state: JobState::Cancelled,
                progress: 0.0,
                message: None,
                error: None,
            },
        });
    }
    if let Some(f) = daemon.meetings().finalizing(id) {
        daemon.meetings().cancel_finalize(id);
        return json(CancelResult {
            reference: reference(id)?,
            job: JobStatus {
                job_id: f.job_id,
                state: JobState::Cancelled,
                progress: 0.0,
                message: Some("finalisation cancelled; the audio is kept".into()),
                error: None,
            },
        });
    }
    if let Some(job) = daemon.diarization().job_for(id) {
        daemon.diarization().cancel(id);
        return json(CancelResult {
            reference: reference(id)?,
            job: JobStatus {
                state: JobState::Cancelled,
                message: Some("diarization cancelled; the transcript is kept".into()),
                ..job
            },
        });
    }
    let row = lookup(daemon, id)?;
    if row.source_kind == "audioImport" && row.status == MeetingStatus::Transcribing {
        let cancelled = daemon.jobs().cancel(id, RefKind::Meeting)?;
        return json(CancelResult {
            reference: cancelled.reference,
            job: cancelled.job,
        });
    }
    if row.status != MeetingStatus::Cancelled {
        return Err(conflict(
            &format!("Meeting {id} is not active; it is {}", row.status.as_str()),
            "meetingNotActive",
        ));
    }
    json(CancelResult {
        reference: reference(id)?,
        job: row_job(
            row.status,
            row.error_message.as_deref(),
            row.recovery_reason.as_deref(),
            "job_meeting",
        ),
    })
}

/// `meetings.status`.
pub fn status(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: MeetingIdParams = params(params_value)?;
    let id = p.meeting_id.as_str();
    let progress = daemon
        .history()
        .with_store(|store| store.meeting_progress(id))
        .map_err(store_error)?
        .ok_or_else(|| {
            JsonRpcError::new(
                AppCode::NotFound,
                "Meeting not found",
                ErrorDetails::empty(),
            )
        })?;
    let row = &progress.capture;
    let live = daemon.meetings().snapshot().filter(|s| s.meeting_id == id);
    let finalizing = daemon.meetings().finalizing(id);
    let (status, job) = match (&live, &finalizing) {
        (Some(s), _) => (
            match s.state {
                dettivo_meeting::State::Stopping => HistoryStatus::Stopping,
                _ => HistoryStatus::Recording,
            },
            Some(JobStatus {
                job_id: s.job_id.clone(),
                state: JobState::Running,
                progress: 0.0,
                message: None,
                error: None,
            }),
        ),
        (None, Some(f)) => (HistoryStatus::Transcribing, Some(finalizing_job(f))),
        (None, None) => (
            history_status(row.status),
            daemon.diarization().job_for(id).or_else(|| {
                (row.status == MeetingStatus::Transcribing).then(|| {
                    row_job(
                        row.status,
                        row.error_message.as_deref(),
                        row.recovery_reason.as_deref(),
                        "job_import",
                    )
                })
            }),
        ),
    };
    let (live_segment_count, live_last_end_ms) = match &live {
        Some(s) => (s.live_segment_count, s.live_last_end_ms),
        None => (progress.segment_count, progress.last_end_ms),
    };
    json(StatusResult {
        reference: reference(id)?,
        status,
        live_segment_count,
        live_last_end_ms,
        is_finalizing: finalizing.is_some(),
        job,
        capture: Some(capture_of(row, live.as_ref(), finalizing.as_ref())),
        recoverable: recoverable(daemon)?,
    })
}
