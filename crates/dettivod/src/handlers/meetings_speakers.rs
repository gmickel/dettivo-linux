//! The speaker half of `meetings.*` (Linux additions, ADR 0035):
//! `meetings.diarize` runs or re-runs the post-meeting speaker pass on a
//! completed meeting, `meetings.speakers.list` reads the speakers and the
//! diarization block, `meetings.speakers.rename` names one across the
//! meeting (every segment, the exports and the search index follow) and
//! remembers the name, and `meetings.speakers.suggest` offers the names
//! used before, most recent first.

use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::speakers::{
    DiarizeParams, DiarizeResult, MAX_NAME_CHARS, RenameParams, RenameResult, SpeakersListParams,
    SpeakersListResult, SuggestParams, SuggestResult, default_label,
};
use dettivo_storage::speakers::DEFAULT_SUGGESTIONS;
use serde_json::Value;

use super::meetings::{conflict, json, lookup, params, reference};
use crate::daemon::Daemon;
use crate::history::store_error;

/// `meetings.diarize`: the pass as a job on a completed meeting.
pub fn diarize(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: DiarizeParams = params(params_value)?;
    let row = lookup(daemon, p.meeting_id.as_str())?;
    let id = row.id.clone();
    let job = daemon.diarization().start(daemon, row, p.speakers)?;
    json(DiarizeResult {
        reference: reference(&id)?,
        job,
    })
}

/// `meetings.speakers.list`.
pub fn list(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: SpeakersListParams = params(params_value)?;
    let row = lookup(daemon, p.meeting_id.as_str())?;
    json(SpeakersListResult {
        reference: reference(&row.id)?,
        speakers: row.speakers,
        diarization: row.diarization.unwrap_or_default(),
    })
}

/// `meetings.speakers.rename`: the speaker's name on its row and on every
/// segment it holds; an empty name restores the label.
pub fn rename(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: RenameParams = params(params_value)?;
    if p.name.chars().any(char::is_control) {
        let details = serde_json::from_value(serde_json::json!({"field":"name"}))
            .expect("name error details");
        return Err(JsonRpcError::new(
            AppCode::InvalidParams,
            "speaker name must not contain control characters",
            details,
        ));
    }
    let chars = p.name.chars().count();
    if chars > MAX_NAME_CHARS {
        return Err(JsonRpcError::new(
            AppCode::InvalidParams,
            format!("name is {chars} characters; the limit is {MAX_NAME_CHARS}"),
            ErrorDetails::empty(),
        ));
    }
    let id = p.meeting_id.as_str();
    if let Some(running) = daemon.diarization().job_for(id) {
        return Err(conflict(
            &format!("Meeting {id} is being diarized by {}", running.job_id),
            "diarizationRunning",
        ));
    }
    let mut row = lookup(daemon, id)?;
    let Some(position) = row
        .speakers
        .iter()
        .position(|s| s.speaker_id == p.speaker_id)
    else {
        return Err(JsonRpcError::new(
            AppCode::NotFound,
            format!("Meeting {id} has no speaker {}", p.speaker_id),
            ErrorDetails::empty(),
        ));
    };
    let given = p.name.trim();
    let name = if given.is_empty() {
        default_label(&p.speaker_id)
    } else {
        given.to_string()
    };
    row.speakers[position].name = name.clone();
    let mut updated = 0u32;
    for s in row
        .segments
        .iter_mut()
        .filter(|s| s.speaker_id.as_deref() == Some(p.speaker_id.as_str()))
    {
        s.speaker = Some(name.clone());
        updated += 1;
    }
    daemon
        .history()
        .with_store(|store| {
            store.update_meeting(&row)?;
            if !given.is_empty() && name != default_label(&p.speaker_id) {
                store.remember_speaker_name(&name)?;
            }
            Ok(())
        })
        .map_err(store_error)?;
    tracing::info!(speaker = %p.speaker_id, segments = updated, "speaker renamed");
    json(RenameResult {
        reference: reference(&row.id)?,
        speaker: row.speakers[position].clone(),
        segments_updated: updated,
    })
}

/// `meetings.speakers.suggest`.
pub fn suggest(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: SuggestParams = params(params_value)?;
    let names = daemon
        .history()
        .with_store(|store| {
            store.suggest_speaker_names(p.prefix.as_deref(), p.limit.unwrap_or(DEFAULT_SUGGESTIONS))
        })
        .map_err(store_error)?;
    json(SuggestResult { names })
}
