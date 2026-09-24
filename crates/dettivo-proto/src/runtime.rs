//! Shared runtime types every namespace's results build on: `TranscriptRef`,
//! `HistoryItem`, `JobStatus`, `InsertionResult` (`docs/api/dettivo-ipc-v1.md`
//! sections 3.2, 3.3, 7.1, 7.2).

use crate::id::Id;
use serde::{Deserialize, Serialize};

/// The kind of history entity a [`TranscriptRef`] points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    /// A single dictation item.
    Dictation,
    /// A meeting recording session.
    Meeting,
}

/// A union reference to either a dictation item or a meeting session,
/// avoiding adapter drift between the two history entities (section 3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptRef {
    /// Which history entity this reference addresses.
    pub kind: RefKind,
    /// The entity's id.
    pub id: Id,
}

/// Lifecycle status shared by dictation items and meeting sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryStatus {
    /// No activity.
    Idle,
    /// Actively recording.
    Recording,
    /// Recording finished, transcription in progress.
    Transcribing,
    /// Finished successfully.
    Completed,
    /// Finished with an error.
    Failed,
    /// Cancelled by the client.
    Cancelled,
    /// Linux addition: a meeting whose stop was asked for and whose takes
    /// are being closed (`docs/api/linux-deltas.md`).
    Stopping,
    /// Linux addition: a meeting whose capture ended and whose
    /// transcription has not run.
    Stopped,
    /// Linux addition: a meeting a daemon restart interrupted; its audio
    /// is preserved and `meetings.recover` or `meetings.discard` decides.
    Partial,
}

impl HistoryStatus {
    /// The wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Recording => "recording",
            Self::Transcribing => "transcribing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Partial => "partial",
        }
    }
}

/// A single row in a history listing (section 3.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryItem {
    /// The item's union reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// Display title.
    pub title: String,
    /// ISO 8601 start timestamp.
    pub started_at: String,
    /// Duration in seconds.
    pub duration_seconds: u64,
    /// Current lifecycle status.
    pub status: HistoryStatus,
    /// Linux addition: the app id the text was inserted into, so a list
    /// row carries its meta line without a second request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    /// Linux addition: the dictation mode (`raw`, `deterministic_polish`,
    /// `enhanced`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Linux addition: how the item came to be (`dictation`,
    /// `audioImport`, `rerun`), the list's kind filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Linux addition (ADR 0038): a meeting row's speaker count, so the
    /// timeline's chip carries who spoke without a second request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_count: Option<u32>,
    /// Linux addition (ADR 0038): where a meeting row's analysis stands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis_status: Option<crate::methods::meetings_notes::AnalysisStatus>,
    /// Linux addition (ADR 0038): a meeting a restart left partial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_partial: Option<bool>,
}

/// Background job lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    /// Queued, not yet started.
    Queued,
    /// Actively running.
    Running,
    /// Finished successfully.
    Succeeded,
    /// Finished with an error.
    Failed,
    /// Cancelled by the client.
    Cancelled,
}

/// Progress and state of a long-running server operation (section 7.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobStatus {
    /// Opaque job id, e.g. `"job_dict_1"` (not a UUID by contract example).
    pub job_id: String,
    /// Current lifecycle state.
    pub state: JobState,
    /// Fractional progress in `[0.0, 1.0]`.
    pub progress: f64,
    /// Optional human-readable progress message.
    pub message: Option<String>,
    /// Optional error string when `state == Failed`.
    pub error: Option<String>,
}

/// How text insertion into the focused app concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsertionOutcome {
    /// Text was inserted directly.
    Inserted,
    /// Insertion fell back to the clipboard.
    CopiedToClipboard,
    /// Insertion failed entirely.
    Failed,
}

/// Which mechanism performed the insertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsertionMethod {
    /// Simulated paste.
    Paste,
    /// Paste failed and fell back to a clipboard copy.
    FallbackCopy,
    /// Clipboard-only mode was requested.
    ClipboardOnly,
}

/// The focused application at the time of insertion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetApp {
    /// Platform bundle/application identifier.
    pub bundle_id: String,
    /// Display name.
    pub name: String,
}

/// Context-pack availability state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextPackStatus {
    /// Context capture is disabled.
    Off,
    /// A context pack is available.
    Ready,
    /// A context pack is available but truncated/bounded.
    Limited,
    /// Context capture was blocked (e.g. permission denied).
    Blocked,
}

/// Where the captured context came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextPackSource {
    /// Adapter that captured the context.
    pub adapter_id: String,
    /// Coarse application class.
    pub app_class: String,
    /// Platform bundle/application identifier.
    pub bundle_id: String,
}

/// Bounded size metrics for a captured context pack.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextPackMetrics {
    /// Milliseconds spent capturing context.
    pub capture_duration_ms: u64,
    /// Captured payload size in bytes.
    pub payload_bytes: u64,
    /// Captured character count.
    pub character_count: u64,
    /// Token budget allotted to the context pack.
    pub token_budget: u64,
    /// Estimated token count of the captured payload.
    pub token_estimate: u64,
}

/// Context captured from the focused app around an insertion or dictation
/// session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextPack {
    /// Availability state.
    pub status: ContextPackStatus,
    /// Nullable reason code, populated when not `Ready`.
    pub reason: Option<String>,
    /// Where the context came from.
    pub source: ContextPackSource,
    /// Bounded size metrics.
    pub metrics: ContextPackMetrics,
}

/// The result of `insert.perform` (section 7.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InsertionResult {
    /// How the insertion concluded.
    pub outcome: InsertionOutcome,
    /// Which mechanism performed it.
    pub method: InsertionMethod,
    /// The focused app at the time of insertion.
    pub target_app: TargetApp,
    /// Captured context around the insertion.
    pub context_pack: ContextPack,
    /// Nullable reason code, populated on `Failed`.
    pub reason: Option<String>,
    /// Linux addition: the backend that performed the insertion, its
    /// latency and whether `insert.undo` can take the text back. Absent
    /// on a port that has no backend chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<InsertionBackend>,
}

/// Linux addition: which backend of the insertion chain performed the
/// insertion (`docs/api/linux-deltas.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InsertionBackend {
    /// Backend name: `virtual_keyboard`, `libei`, `ydotool`, `xdotool`,
    /// `clipboard_paste`, `clipboard` or `mock`.
    pub name: String,
    /// Milliseconds the backend took.
    pub latency_ms: u64,
    /// True when `insert.undo` can take this insertion back.
    pub undo_supported: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn transcript_ref_round_trips() {
        let v = json!({"kind":"meeting","id":"0f8fad5b-d9cb-469f-a165-70867728950e"});
        let r: TranscriptRef = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(r.kind, RefKind::Meeting);
        assert_eq!(serde_json::to_value(&r).unwrap(), v);
    }

    #[test]
    fn history_item_ref_field_renames_from_reference() {
        let v = json!({
            "ref": {"kind":"meeting","id":"0f8fad5b-d9cb-469f-a165-70867728950e"},
            "title": "Weekly sync",
            "started_at": "2026-02-13T16:00:00Z",
            "duration_seconds": 1800,
            "status": "idle"
        });
        let item: HistoryItem = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(item.status, HistoryStatus::Idle);
        assert_eq!(serde_json::to_value(&item).unwrap(), v);
    }
}
