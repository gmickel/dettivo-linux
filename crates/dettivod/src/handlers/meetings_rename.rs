//! The Linux addition `meetings.rename` (ADR 0061): the app edits the
//! title inline, the trimmed text lands on the row as `manual` and the
//! search index follows. Allowed in every meeting status.

use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::meetings_notes::{
    MAX_TITLE_CHARS, MeetingRenameParams, MeetingRenameResult,
};
use serde_json::Value;

use super::meetings::{json, lookup, params, reference};
use crate::daemon::Daemon;
use crate::history::store_error;

fn invalid(message: impl Into<String>, details: ErrorDetails) -> JsonRpcError {
    JsonRpcError::new(AppCode::InvalidParams, message, details)
}

/// `meetings.rename`: control characters, an empty title and one over
/// [`MAX_TITLE_CHARS`] are `INVALID_PARAMS`, in that order; an unknown
/// meeting is `NOT_FOUND`.
pub fn rename(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: MeetingRenameParams = params(params_value)?;
    if p.title.chars().any(char::is_control) {
        let details = serde_json::from_value(serde_json::json!({"field":"title"}))
            .expect("title error details");
        return Err(invalid(
            "title must not contain control characters",
            details,
        ));
    }
    let title = p.title.trim();
    if title.is_empty() {
        return Err(invalid("title must not be empty", ErrorDetails::empty()));
    }
    let chars = title.chars().count();
    if chars > MAX_TITLE_CHARS {
        return Err(invalid(
            format!("title is {chars} characters; the limit is {MAX_TITLE_CHARS}"),
            ErrorDetails::empty(),
        ));
    }
    let id = p.meeting_id.as_str();
    lookup(daemon, id)?;
    let row = daemon
        .history()
        .with_store(|s| s.set_meeting_title(id, title))
        .map_err(store_error)?;
    tracing::info!(id = %row.id, chars, "meeting renamed");
    json(MeetingRenameResult {
        reference: reference(&row.id)?,
        title: row.title,
        title_source: row.title_source,
    })
}
