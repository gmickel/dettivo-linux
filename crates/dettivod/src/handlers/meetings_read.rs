//! `meetings.list`, `meetings.get` and `meetings.search`: the read verbs
//! over the rows, with the Linux facts (the speakers and the diarization
//! block, ADR 0035; notes, analysis, the polished segments and the
//! matched search column, ADR 0036).

use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::meetings::{
    GetParams, GetResult, ListItem, ListParams, ListResult, SearchItem, SearchParams, SearchResult,
};
use dettivo_storage::dictations::ListFilter;
use dettivo_storage::meeting_reads::MeetingSummary;
use dettivo_storage::search::MAX_QUERY_BYTES;
use serde_json::Value;

use super::meetings::{history_status, json, lookup, params, reference};
use crate::daemon::Daemon;
use crate::history::store_error;

/// The contract list row with the Linux facts (the summary, the notes,
/// the analysis, the partial flag, the speakers).
pub fn list_item(row: &MeetingSummary) -> Result<ListItem, JsonRpcError> {
    Ok(ListItem {
        reference: reference(&row.id)?,
        title: row.title.clone(),
        started_at: row.started_at.clone(),
        duration_seconds: row.duration_seconds(),
        status: history_status(row.status),
        summary: row.summary.clone(),
        has_notes: row.has_notes,
        analysis_status: row.analysis_status,
        is_partial: row.is_partial,
        speaker_count: row.speaker_count,
    })
}

/// `meetings.list`.
pub fn list(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: ListParams = params(params_value)?;
    let page = daemon
        .history()
        .with_store(|store| {
            store.list_meetings(&ListFilter {
                limit: p.limit.max(1),
                cursor: p.cursor.clone(),
                ..ListFilter::default()
            })
        })
        .map_err(store_error)?;
    json(ListResult {
        items: page
            .items
            .iter()
            .map(list_item)
            .collect::<Result<Vec<_>, _>>()?,
        next_cursor: page.next_cursor,
    })
}

/// `meetings.get`.
pub fn get(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: GetParams = params(params_value)?;
    let row = lookup(daemon, p.meeting_id.as_str())?;
    json(GetResult {
        reference: reference(&row.id)?,
        title: row.title.clone(),
        started_at: row.started_at.clone(),
        duration_seconds: row.duration_seconds(),
        status: history_status(row.status),
        transcript: row.final_text.clone(),
        notes: row.notes_markdown.clone(),
        notes_source: row.notes_source,
        analysis: row.analysis.clone(),
        analysis_status: row.analysis_status,
        ended_at: row.ended_at.clone(),
        language: row.language.clone(),
        stt_provider_id: row.stt_provider.clone(),
        stt_model_id: row.stt_model.clone(),
        segments: row.segments,
        speakers: row.speakers,
        diarization: row.diarization,
    })
}

/// `meetings.search`: title, transcript, speaker names, notes and
/// analysis, each hit naming the column its snippet came from.
pub fn search(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: SearchParams = params(params_value)?;
    if p.query.len() > MAX_QUERY_BYTES {
        return Err(JsonRpcError::new(
            AppCode::InvalidParams,
            format!(
                "query is {} bytes; the limit is {MAX_QUERY_BYTES}",
                p.query.len()
            ),
            ErrorDetails::empty(),
        ));
    }
    let hits = daemon
        .history()
        .with_store(|store| store.search_meetings(&p.query, p.limit.max(1)))
        .map_err(store_error)?;
    json(SearchResult {
        items: hits
            .into_iter()
            .map(|h| {
                Ok(SearchItem {
                    reference: reference(&h.id)?,
                    snippet: h.snippet,
                    score: None,
                    matched_field: Some(h.matched_field.to_string()),
                })
            })
            .collect::<Result<Vec<_>, JsonRpcError>>()?,
    })
}
