//! `transcripts.list`, `get`, `latest`, `search` per the contract over
//! both kinds (the one timeline, ADR 0027), and the Linux additions
//! `delete`, `rerun`, `stats` and `cancel`. A request for an unknown item
//! is `NOT_FOUND`; a meeting re-run arrives with the transcription spec.

use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::id::Id;
use dettivo_proto::methods::transcripts::{
    CancelParams, CancelResult, DeleteParams, DeleteResult, GetParams, GetResult, ItemAudio,
    ItemFacts, ItemTimings, LatestKind, LatestParams, LatestResult, ListParams, ListResult,
    MatchRange, MeetingExtras, RerunParams, RerunResult, SearchItem, SearchParams, SearchResult,
};
use dettivo_proto::runtime::{HistoryItem, HistoryStatus, RefKind, TranscriptRef};
use dettivo_storage::dictations::ListFilter;
use dettivo_storage::item::{DictationItem, ItemStatus};
use dettivo_storage::meeting_reads::MeetingSummary;
use dettivo_storage::search::MAX_QUERY_BYTES;
use dettivo_storage::timeline::Entry;
use serde_json::Value;

use super::{json, params};

use crate::daemon::Daemon;
use crate::history::store_error;
use crate::jobs::Overrides;

/// The parts of the meeting kind the transcription spec brings.
pub fn meetings_pending(what: &str) -> JsonRpcError {
    JsonRpcError::new(
        AppCode::NotImplemented,
        format!("{what} for a meeting arrives with the transcription spec"),
        ErrorDetails::empty(),
    )
}

/// The history row of a meeting, for the timeline.
pub fn meeting_history_item(row: &MeetingSummary) -> Result<HistoryItem, JsonRpcError> {
    let item = super::meetings_read::list_item(row)?;
    Ok(HistoryItem {
        reference: item.reference,
        title: item.title,
        started_at: item.started_at,
        duration_seconds: item.duration_seconds,
        status: item.status,
        app_id: None,
        mode: None,
        source: Some(row.source_kind.clone()),
        speaker_count: Some(item.speaker_count),
        analysis_status: Some(item.analysis_status),
        is_partial: Some(item.is_partial),
    })
}

/// The contract reference of an item.
pub fn reference(id: &str) -> Result<TranscriptRef, JsonRpcError> {
    Ok(TranscriptRef {
        kind: RefKind::Dictation,
        id: Id::new(id).map_err(|e| {
            JsonRpcError::new(AppCode::InternalError, e.to_string(), ErrorDetails::empty())
        })?,
    })
}

/// A history row for an item.
pub fn history_item(item: &DictationItem) -> Result<HistoryItem, JsonRpcError> {
    Ok(HistoryItem {
        reference: reference(&item.id)?,
        title: item.title.clone(),
        started_at: item.created_at.clone(),
        duration_seconds: item.duration_seconds(),
        status: match item.status {
            ItemStatus::Transcribing => HistoryStatus::Transcribing,
            ItemStatus::Completed => HistoryStatus::Completed,
            ItemStatus::Failed
                if item.error_code.as_deref() == Some(crate::jobs_run::CANCELLED) =>
            {
                HistoryStatus::Cancelled
            }
            ItemStatus::Failed => HistoryStatus::Failed,
        },
        app_id: Some(item.app_id.clone()),
        mode: Some(item.mode.clone()),
        source: Some(item.source_kind.as_str().to_string()),
        speaker_count: None,
        analysis_status: None,
        is_partial: None,
    })
}

/// The facts of an item beyond its texts (Linux addition to
/// `transcripts.get`).
pub fn facts(item: &DictationItem) -> Result<ItemFacts, JsonRpcError> {
    let retained = item
        .audio_path
        .as_deref()
        .is_some_and(|p| std::path::Path::new(p).is_file());
    let audio = ItemAudio {
        retained,
        path: retained.then(|| item.audio_path.clone()).flatten(),
        reason: match (retained, &item.audio_path) {
            (true, _) => None,
            (false, None) => Some("not_retained".into()),
            (false, Some(_)) => Some("expired".into()),
        },
    };
    Ok(ItemFacts {
        created_at: item.created_at.clone(),
        app_id: item.app_id.clone(),
        app_name: item.app_name.clone(),
        source: item.source_kind.as_str().to_string(),
        status: history_item(item)?.status.as_str().to_string(),
        provider: item.stt_provider.clone(),
        model: item.stt_model.clone(),
        language: item.language.clone(),
        duration_seconds: item.duration_seconds(),
        title: item.title.clone(),
        rerun_of: item
            .rerun_of_item_id
            .as_deref()
            .map(reference)
            .transpose()?,
        audio,
        insertion: item.insertion.clone(),
        timings: item.timings.map(|t| ItemTimings {
            capture_ms: t.capture_ms,
            transcribe_ms: t.transcribe_ms,
            insert_ms: t.insert_ms,
            stop_to_insert_ms: t.stop_to_insert_ms(),
        }),
        error: item.error_message.clone(),
    })
}

/// One dictation item by reference, or the contract's refusal. A meeting
/// reference is the caller's to route (`meetings::lookup`).
pub fn lookup(daemon: &Daemon, reference: &TranscriptRef) -> Result<DictationItem, JsonRpcError> {
    if reference.kind == RefKind::Meeting {
        return Err(JsonRpcError::new(
            AppCode::InvalidParams,
            "ref.kind must be dictation here",
            ErrorDetails::empty(),
        ));
    }
    let id = reference.id.as_str();
    daemon
        .history()
        .with_store(|store| store.get(id))
        .map_err(store_error)?
        .ok_or_else(|| {
            JsonRpcError::new(
                AppCode::NotFound,
                format!("no dictation {id}"),
                ErrorDetails::empty(),
            )
        })
}

/// `transcripts.list`: the one timeline over the kinds asked for.
pub fn list(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: ListParams = params(params_value)?;
    let dictations = p.kinds.contains(&RefKind::Dictation);
    let meetings = p.kinds.contains(&RefKind::Meeting);
    if !dictations && !meetings {
        return json(ListResult {
            items: Vec::new(),
            next_cursor: None,
        });
    }
    let page = daemon
        .history()
        .with_store(|store| {
            store.timeline(
                &ListFilter {
                    limit: p.limit.max(1),
                    cursor: p.cursor.clone(),
                    app_id: p.app_id.clone(),
                    since: p.since.clone(),
                    until: p.until.clone(),
                },
                dictations,
                meetings,
            )
        })
        .map_err(store_error)?;
    json(ListResult {
        items: page
            .items
            .iter()
            .map(|e| match e {
                Entry::Dictation(item) => history_item(item),
                Entry::Meeting(row) => meeting_history_item(row),
            })
            .collect::<Result<Vec<_>, _>>()?,
        next_cursor: page.next_cursor,
    })
}

/// `transcripts.get`.
pub fn get(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: GetParams = params(params_value)?;
    if p.reference.kind == RefKind::Meeting {
        let row = super::meetings::lookup(daemon, p.reference.id.as_str())?;
        return json(GetResult {
            reference: super::meetings::reference(&row.id)?,
            facts: None,
            text_raw: row.raw_text,
            text_polish: row.final_text,
            segments: row.segments,
            mode: None,
            notice: None,
            policy_hash: None,
            meeting: Some(MeetingExtras {
                notes: row.notes_markdown,
                notes_source: row.notes_source,
                analysis: row.analysis,
                analysis_status: row.analysis_status,
                ended_at: row.ended_at,
                language: row.language,
                stt_provider_id: row.stt_provider,
                stt_model_id: row.stt_model,
            }),
        });
    }
    let item = lookup(daemon, &p.reference)?;
    json(GetResult {
        reference: reference(&item.id)?,
        facts: Some(facts(&item)?),
        text_raw: item.raw_text,
        text_polish: item.final_text,
        segments: item.segments,
        mode: Some(item.mode),
        notice: item.notice.and_then(|v| serde_json::from_value(v).ok()),
        policy_hash: item.policy_hash,
        meeting: None,
    })
}

/// `transcripts.latest`: the newest of the kind, or of either.
pub fn latest(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: LatestParams = params(params_value)?;
    let dictation = if p.kind == LatestKind::Meeting {
        None
    } else {
        daemon
            .history()
            .with_store(|store| store.latest())
            .map_err(store_error)?
    };
    let meeting = if p.kind == LatestKind::Dictation {
        None
    } else {
        daemon
            .history()
            .with_store(|store| store.latest_meeting())
            .map_err(store_error)?
    };
    let reference = match (dictation, meeting) {
        (Some(d), Some(m)) if m.created_at > d.created_at => super::meetings::reference(&m.id)?,
        (Some(d), _) => reference(&d.id)?,
        (None, Some(m)) => super::meetings::reference(&m.id)?,
        (None, None) => {
            return Err(JsonRpcError::new(
                AppCode::NotFound,
                "nothing has been stored yet",
                ErrorDetails::empty(),
            ));
        }
    };
    json(LatestResult { reference })
}

/// `transcripts.search`.
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
    use dettivo_storage::combined_search::CombinedHit;
    let hits = daemon
        .history()
        .with_store(|store| store.search_combined(&p.query, p.limit, &p.kinds))
        .map_err(store_error)?;
    let mut items = Vec::new();
    for hit in hits {
        items.push(match hit {
            CombinedHit::Dictation(h) => SearchItem {
                reference: reference(&h.item.id)?,
                item: Some(history_item(&h.item)?),
                snippet: h.snippet,
                score: h.score,
                matches: Some(
                    h.matches
                        .iter()
                        .map(|&(start, end)| MatchRange {
                            start: start as u32,
                            end: end as u32,
                        })
                        .collect(),
                ),
                matched_field: None,
            },
            CombinedHit::Meeting(h) => SearchItem {
                reference: super::meetings::reference(&h.id)?,
                item: None,
                snippet: h.snippet,
                score: None,
                matches: None,
                matched_field: Some(h.matched_field.to_string()),
            },
        });
    }
    json(SearchResult { items })
}

/// `transcripts.delete` (Linux addition).
pub fn delete(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: DeleteParams = params(params_value)?;
    if p.reference.kind == RefKind::Meeting {
        return super::meetings_recovery::delete(
            daemon,
            serde_json::json!({"meeting_id": p.reference.id.as_str(), "artifact_policy": "all"}),
        );
    }
    let id = p.reference.id.as_str();
    if daemon.history().busy().contains(id) {
        return Err(JsonRpcError::new(
            AppCode::Conflict,
            format!("dictation {id} is in use by a running job"),
            ErrorDetails::conflict_kind("jobRunning"),
        ));
    }
    let item = daemon.history().delete(id).map_err(store_error)?;
    tracing::info!("history: dictation deleted");
    json(DeleteResult {
        reference: reference(&item.id)?,
        deleted: true,
    })
}

/// `transcripts.rerun` (Linux addition).
pub fn rerun(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: RerunParams = params(params_value)?;
    if p.reference.kind == RefKind::Meeting {
        return Err(meetings_pending("a re-run"));
    }
    let original = lookup(daemon, &p.reference)?;
    let result: RerunResult = daemon.jobs().start_rerun(
        daemon,
        original,
        Overrides {
            provider: p.provider,
            model: p.model,
            mode: p.mode.map(|m| m.as_str().to_string()),
            language: p.language,
        },
    )?;
    json(result)
}

/// `transcripts.cancel` (Linux addition): the job on the item stops
/// between chunks.
pub fn cancel(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: CancelParams = params(params_value)?;
    let result: CancelResult = daemon
        .jobs()
        .cancel(p.reference.id.as_str(), p.reference.kind)?;
    json(result)
}

/// `transcripts.stats` (Linux addition).
pub fn stats(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    json(daemon.history().stats().map_err(store_error)?)
}
