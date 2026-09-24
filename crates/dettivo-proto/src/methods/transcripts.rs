//! `transcripts.*`: the cross-kind projection over dictation and meeting
//! transcripts — `list`, `get`, `latest`, `search`, `import`, `export`
//! (`docs/api/dettivo-ipc-v1.md` section 8.4). The Linux facts blocks a
//! `transcripts.get` answer carries live in `transcripts_facts`.

use crate::capabilities::ExportFormat;
use crate::methods::dictation::DictationMode;
use crate::methods::meetings::Segment;
pub use crate::methods::transcripts_facts::{ItemAudio, ItemFacts, ItemTimings, MeetingExtras};
use crate::runtime::{HistoryItem, JobStatus, RefKind, TranscriptRef};
use serde::{Deserialize, Serialize};

/// `transcripts.list` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListParams {
    /// Which history kinds to include.
    pub kinds: Vec<RefKind>,
    /// Maximum items to return.
    pub limit: u32,
    /// Pagination cursor from a prior page.
    pub cursor: Option<String>,
    /// Linux addition: only items inserted into this app id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    /// Linux addition: only items created at or after this ISO 8601 time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// Linux addition: only items created before this ISO 8601 time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
}

/// `transcripts.list` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListResult {
    /// The page of history items.
    pub items: Vec<HistoryItem>,
    /// Cursor for the next page, `None` when exhausted.
    pub next_cursor: Option<String>,
}

/// `transcripts.get` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetParams {
    /// The target item's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
}

/// `transcripts.get` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetResult {
    /// The item's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// Raw (unpolished) transcript text.
    pub text_raw: String,
    /// LLM-polished transcript text.
    pub text_polish: String,
    /// Per-segment breakdown, empty for a dictation item.
    pub segments: Vec<Segment>,
    /// Linux addition: the mode the item was dictated in (`raw`,
    /// `deterministic_polish`, `enhanced`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Linux addition: why the Enhanced pass inserted the deterministic
    /// result instead of a rewrite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<crate::methods::polish::PolishNotice>,
    /// Linux addition: the hash of the Polish policy the session ran
    /// under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_hash: Option<String>,
    /// Linux addition: the item's facts the history detail shows, absent
    /// on the meeting kind until the meetings spec.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facts: Option<ItemFacts>,
    /// Linux addition, meeting kind only: the notes, the analysis and the
    /// facts `meetings.get` carries; omitted for a dictation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meeting: Option<MeetingExtras>,
}

/// `transcripts.latest` params.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LatestKind {
    /// Only dictation items.
    Dictation,
    /// Only meeting sessions.
    Meeting,
    /// Either kind, whichever is most recent.
    Any,
}

/// `transcripts.latest` params.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LatestParams {
    /// Which kind(s) to consider.
    pub kind: LatestKind,
}

/// `transcripts.latest` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LatestResult {
    /// The most recent matching item's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
}

/// `transcripts.search` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchParams {
    /// Free-text query.
    pub query: String,
    /// Which history kinds to search.
    pub kinds: Vec<RefKind>,
    /// Maximum results to return.
    pub limit: u32,
}

/// One `transcripts.search` hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchItem {
    /// The matched item's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// Matched text snippet.
    pub snippet: String,
    /// Relevance score; only meaningful when
    /// `knowledge.semantic_search=true`.
    pub score: Option<f64>,
    /// Linux addition: where the query's words start inside `snippet`, as
    /// character ranges, so a list paints the matches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matches: Option<Vec<MatchRange>>,
    /// Linux addition: the hit as a history row, so a search result lists
    /// without a `transcripts.get` per hit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<HistoryItem>,
    /// Linux addition, meeting hits only: the column the snippet came
    /// from (`title`, `transcript`, `notes`, `analysis`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_field: Option<String>,
}

/// One painted span of a search snippet, in characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchRange {
    /// The first character of the span.
    pub start: u32,
    /// One past the last character.
    pub end: u32,
}

/// `transcripts.search` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchResult {
    /// Matched items.
    pub items: Vec<SearchItem>,
}

/// `transcripts.import` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportParams {
    /// Transfer id from a prior `transfer.begin`.
    pub transfer_id: String,
    /// Which history kind to import into.
    pub target_kind: RefKind,
    /// Original filename; sanitized and preserved for extension detection
    /// and default titling.
    pub filename: String,
    /// BCP-47 language tag.
    pub language: String,
    /// Output mode for the imported transcript.
    pub mode: DictationMode,
    /// Adopted from the Windows port (registered in
    /// `docs/api/linux-deltas.md`): required by configuration for
    /// `target_kind=meeting`, exactly as on `meetings.start`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acknowledge_meeting_disclosure: Option<bool>,
    /// Linux addition: the speech provider to transcribe with; absent
    /// means `[speech] provider`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Linux addition: the model to transcribe with; absent means
    /// `[speech] model`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Linux addition (ADR 0035): for `target_kind=meeting`, the speaker
    /// count the diarization pass is told; absent lets the clustering
    /// decide.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_speakers: Option<u32>,
    /// Linux addition: for `target_kind=meeting`, run the diarization
    /// pass once the import completes; absent means
    /// `[meetings.diarization] auto`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diarize: Option<bool>,
    /// Linux addition, `target_kind = meeting`: run the analysis once the
    /// import completes; absent means `[meetings.analysis] auto`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analyze: Option<bool>,
}

/// `transcripts.import` result. For `target_kind=meeting`, the response
/// returns once the meeting session is visible and the import handoff is
/// running (job state `running`), rather than waiting for completion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportResult {
    /// The imported item's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// Whether this result reflects a partial (still in-flight) import.
    pub is_partial: bool,
    /// The import job's status.
    pub job: JobStatus,
}

/// Known `CONFLICT` detail kinds for `transcripts.import` (section 8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportConflictKind {
    /// An import is already running for this target.
    ImportAlreadyInProgress,
    /// An active session blocks starting a new import.
    ImportBlockedByActiveSession,
}

/// What a Linux export covers (a Linux addition on `transcripts.export`,
/// registered in `docs/api/linux-deltas.md`); absent means `item`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportScope {
    /// The one item `ref` names (the contract's behaviour).
    Item,
    /// Every dictation item created in `[from, to)`.
    Range,
    /// Every dictation item.
    All,
}

/// `transcripts.export` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportParams {
    /// The item to export; required for scope `item`, and for the other
    /// scopes it only selects the kind (a Linux addition lets it be absent).
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<TranscriptRef>,
    /// Requested export format. Validity must be checked against the
    /// capability scope for the reference's kind (`dictation_export` vs
    /// `meeting_export`); `zip` is accepted when `history.archive_export`.
    pub format: ExportFormat,
    /// Transfer id from a prior `transfer.begin`.
    pub transfer_id: String,
    /// Linux addition: the scope; absent means `item`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ExportScope>,
    /// Linux addition: the range start (ISO 8601) for scope `range`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Linux addition: the range end (ISO 8601, exclusive) for scope `range`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Linux addition, meeting kind: the live editor's notes, which take
    /// the place of the saved notes in `md` and `json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes_override: Option<String>,
    /// Linux addition, meeting kind: render the engine's words rather
    /// than the polished segments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<bool>,
}

/// `transcripts.export` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportResult {
    /// The transfer id the exported bytes are bound to.
    pub transfer_id: String,
    /// MIME content type of the exported payload.
    pub content_type: String,
    /// Suggested filename for the exported payload.
    pub filename: String,
}

/// `transcripts.delete` params (Linux addition).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteParams {
    /// The item to delete.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
}

/// `transcripts.delete` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteResult {
    /// The deleted item's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// Always `true` on success.
    pub deleted: bool,
}

/// `transcripts.rerun` params (Linux addition): transcribe an item's
/// retained audio again, with overrides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RerunParams {
    /// The item whose audio is re-run.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// The provider to use; absent means the item's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// The model to use; absent means the selection in force.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The mode; absent means `raw`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<DictationMode>,
    /// The language; absent means the item's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// `transcripts.rerun` result: the new item, linked to the original, and
/// the running job that fills it in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RerunResult {
    /// The new item's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// The original.
    pub rerun_of: TranscriptRef,
    /// The job producing the text; `job.progress` reports on it.
    pub job: JobStatus,
}

/// `transcripts.cancel` params (Linux addition): stop the import or
/// re-run job working on an item between two chunks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelParams {
    /// The item whose job is cancelled.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
}

/// `transcripts.cancel` result: the item and its job, now `cancelled`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelResult {
    /// The item.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// The job, in state `cancelled`.
    pub job: JobStatus,
}

/// `transcripts.stats` result (Linux addition): what `dettivo doctor`
/// reports about the history database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatsResult {
    /// The database file.
    pub path: String,
    /// Its size in bytes, WAL included.
    pub size_bytes: u64,
    /// Dictation items stored.
    pub item_count: u64,
    /// Items with retained audio.
    pub audio_items: u64,
    /// The schema version in force.
    pub schema_version: u32,
    /// The last applied migration's name.
    pub last_migration: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn export_params_without_linux_fields_serialize_as_the_contract() {
        let v = json!({"format": "json", "ref": {"id": "0f8fad5b-d9cb-469f-a165-70867728950e", "kind": "meeting"}, "transfer_id": "xfer_out_1"});
        let p: ExportParams = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(p.scope, None);
        assert_eq!(serde_json::to_value(&p).unwrap(), v);
        let all: ExportParams =
            serde_json::from_value(json!({"format": "zip", "transfer_id": "x", "scope": "all"}))
                .unwrap();
        assert_eq!(all.reference, None);
        assert_eq!(all.scope, Some(ExportScope::All));
    }

    #[test]
    fn import_conflict_kind_serializes_camel_case() {
        let json = serde_json::to_string(&ImportConflictKind::ImportAlreadyInProgress).unwrap();
        assert_eq!(json, "\"importAlreadyInProgress\"");
    }

    #[test]
    fn get_params_ref_field_renames_from_reference() {
        let v = json!({"ref": {"kind": "meeting", "id": "0f8fad5b-d9cb-469f-a165-70867728950e"}});
        let params: GetParams = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(&params).unwrap(), v);
    }
}
