//! The Linux additions `meetings.notes.get`, `meetings.notes.set`,
//! `meetings.analyze` and `meetings.analysis.get` (ADR 0036): the user's
//! notes on the row and in `notes.md`, and the analysis job over the
//! finalised transcript.

use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::meetings_notes::{
    AnalysisGetParams, AnalysisGetResult, AnalysisStatus, AnalyzeParams, AnalyzeResult,
    MAX_NOTES_BYTES, NotesGetParams, NotesResult, NotesSetParams, NotesSource,
};
use dettivo_proto::methods::polish::PolishNotice;
use dettivo_proto::runtime::{JobState, JobStatus};
use dettivo_storage::meeting_notes;
use dettivo_storage::meetings::MeetingRow;
use serde_json::{Map, Value};

use super::meetings::{conflict, json, lookup, params, reference};
use crate::analysis::Refusal;
use crate::daemon::Daemon;
use crate::history::store_error;

fn notes_result(row: &MeetingRow) -> Result<Value, JsonRpcError> {
    json(NotesResult {
        reference: reference(&row.id)?,
        markdown: row.notes_markdown.clone(),
        source: row.notes_source,
        updated_at: row.notes_updated_at.clone(),
    })
}

/// `meetings.notes.get`.
pub fn notes_get(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: NotesGetParams = params(params_value)?;
    notes_result(&lookup(daemon, p.meeting_id.as_str())?)
}

/// `meetings.notes.set`: the row and `notes.md` in the meeting directory;
/// a body over 1 MiB is `INVALID_PARAMS` naming the limit. Notes on a
/// recording meeting are allowed (the live editor saves with `live`).
pub fn notes_set(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: NotesSetParams = params(params_value)?;
    if p.markdown.len() > MAX_NOTES_BYTES {
        return Err(JsonRpcError::new(
            AppCode::InvalidParams,
            format!(
                "markdown is {} bytes; the limit is {MAX_NOTES_BYTES} bytes (1 MiB)",
                p.markdown.len()
            ),
            ErrorDetails::empty(),
        ));
    }
    let id = p.meeting_id.as_str();
    lookup(daemon, id)?;
    let source = p.source.unwrap_or(NotesSource::User);
    let row = daemon
        .history()
        .with_store(|s| s.set_meeting_notes(id, &p.markdown, source))
        .map_err(store_error)?;
    let dir = daemon.history().meeting_artifacts().dir(id);
    if let Err(e) = meeting_notes::write_notes_file(&dir, &row.notes_markdown) {
        tracing::warn!(error = %e, "meeting notes.md not written");
    }
    tracing::info!(
        bytes = row.notes_markdown.len(),
        source = source.as_str(),
        "meeting notes stored"
    );
    notes_result(&row)
}

fn analysis_result(row: &MeetingRow) -> Result<Value, JsonRpcError> {
    json(AnalysisGetResult {
        reference: reference(&row.id)?,
        analysis_status: row.analysis_status,
        analysis: row.analysis.clone(),
        error: row.analysis_error.clone(),
        model: row.analysis_model.clone(),
        analyzed_at: row.analysis_at.clone(),
    })
}

/// `meetings.analysis.get`.
pub fn analysis_get(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: AnalysisGetParams = params(params_value)?;
    analysis_result(&lookup(daemon, p.meeting_id.as_str())?)
}

fn job(job_id: &str, state: JobState, message: Option<&str>, error: Option<String>) -> JobStatus {
    JobStatus {
        job_id: job_id.to_string(),
        state,
        progress: if state == JobState::Succeeded {
            1.0
        } else {
            0.0
        },
        message: message.map(str::to_string),
        error,
    }
}

/// `meetings.analyze`: starts the job (`force` runs again over a meeting
/// that has an analysis); `CONFLICT` `analysisRunning` while one runs,
/// `meetingNotCompleted` before the transcript is final,
/// `endpointNotTrusted` for an unconfirmed remote endpoint; when no
/// provider answers the row is `failed` with `provider_unavailable` and
/// the answer carries the notice, so a later call succeeds once one does.
pub fn analyze(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: AnalyzeParams = params(params_value)?;
    let id = p.meeting_id.as_str();
    let row = lookup(daemon, id)?;
    let force = p.force.unwrap_or(false);
    match daemon.analysis().start(row.clone(), force) {
        Ok(running) => json(AnalyzeResult {
            reference: reference(id)?,
            job: job(&running.job_id, JobState::Running, Some("analyzing"), None),
            analysis_status: AnalysisStatus::Running,
            notice: None,
        }),
        Err(Refusal::AlreadyReady) => json(AnalyzeResult {
            reference: reference(id)?,
            job: job("job_analysis", JobState::Succeeded, None, None),
            analysis_status: AnalysisStatus::Ready,
            notice: None,
        }),
        Err(Refusal::Running(job_id)) => Err(JsonRpcError::new(
            AppCode::Conflict,
            format!("an analysis of meeting {id} is running ({job_id})"),
            ErrorDetails::conflict_kind("analysisRunning"),
        )),
        Err(Refusal::NotCompleted(status)) => Err(conflict(
            &format!(
                "Meeting {id} has no final transcript yet; it is {}",
                status.as_str()
            ),
            "meetingNotCompleted",
        )),
        Err(Refusal::EndpointNotTrusted(detail)) => {
            let mut details = Map::new();
            details.insert("kind".into(), Value::String("endpointNotTrusted".into()));
            details.insert("detail".into(), Value::String(detail.clone()));
            Err(JsonRpcError::new(
                AppCode::Conflict,
                format!("{detail}; confirm it with dettivo llm trust <url>"),
                ErrorDetails(details),
            ))
        }
        Err(Refusal::NoProvider { detail, hint }) => {
            let reason = match hint {
                Some(h) => format!("{detail}; {h}"),
                None => detail,
            };
            json(AnalyzeResult {
                reference: reference(id)?,
                job: job(
                    "job_analysis",
                    JobState::Failed,
                    None,
                    Some(format!("provider_unavailable: {reason}")),
                ),
                analysis_status: AnalysisStatus::Failed,
                notice: Some(PolishNotice {
                    kind: "provider_unavailable".into(),
                    reason,
                }),
            })
        }
    }
}
