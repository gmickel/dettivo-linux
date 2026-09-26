//! `meetings.segments` (ADR 0071): the transcript of a meeting so far,
//! live while the session holds its tail (recording, stopping, stopped,
//! transcribing) and the stored row's segments once the settled row is
//! stored, with the cursor that reads only what is new. A read never
//! writes and never waits on capture.

use dettivo_meeting::State;
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::meetings_segments::{Cursor, SegmentsParams, SegmentsResult};
use dettivo_proto::runtime::HistoryStatus;
use dettivo_storage::meetings::MeetingStatus;
use serde_json::Value;

use super::meetings::{history_status, json, lookup, params};
use crate::daemon::Daemon;
use crate::history::store_error;

/// The wire status of the session's own state.
fn live_status(state: State) -> HistoryStatus {
    match state {
        State::Stopping => HistoryStatus::Stopping,
        State::Stopped => HistoryStatus::Stopped,
        State::Transcribing => HistoryStatus::Transcribing,
        State::Completed => HistoryStatus::Completed,
        State::Failed => HistoryStatus::Failed,
        State::Cancelled => HistoryStatus::Cancelled,
        State::Partial => HistoryStatus::Partial,
        State::Idle | State::Recording => HistoryStatus::Recording,
    }
}

/// `meetings.segments`.
pub fn segments(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: SegmentsParams = params(params_value)?;
    let id = p.meeting_id.as_str();
    let since = p
        .since
        .as_deref()
        .map(Cursor::parse)
        .transpose()
        .map_err(|e| {
            JsonRpcError::new(
                AppCode::InvalidParams,
                e,
                ErrorDetails(serde_json::Map::from_iter([(
                    "field".into(),
                    "since".into(),
                )])),
            )
        })?;
    let row_status = daemon
        .history()
        .with_store(|store| store.meeting_progress(id))
        .map_err(store_error)?
        .ok_or_else(|| {
            JsonRpcError::new(
                AppCode::NotFound,
                "Meeting not found",
                ErrorDetails::empty(),
            )
        })?
        .capture
        .status;
    let mut stored_status: Option<MeetingStatus> = None;
    let read = daemon.meetings().read_segments(id, since, || {
        let row = lookup(daemon, id)?;
        stored_status = Some(row.status);
        Ok::<_, JsonRpcError>(row.segments)
    })?;
    let status = match daemon.meetings().snapshot().filter(|s| s.meeting_id == id) {
        Some(s) => live_status(s.state),
        None if daemon.meetings().finalizing(id).is_some() => HistoryStatus::Transcribing,
        None => history_status(stored_status.unwrap_or(row_status)),
    };
    json(SegmentsResult {
        meeting_id: p.meeting_id,
        status,
        transcript: read.transcript,
        cursor: read.cursor,
        reset: read.reset,
        segments: read.segments,
        provisional: read.provisional,
    })
}
