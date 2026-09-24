//! The Linux facts `transcripts.get` carries beyond an item's texts
//! (`docs/api/linux-deltas.md`): a dictation's facts, its retained take and
//! its stop-to-insert timings, and a meeting's notes, analysis and frozen
//! engine facts (ADR 0036). Re-exported from `transcripts`.

use crate::runtime::TranscriptRef;
use serde::{Deserialize, Serialize};

/// Linux addition: what `transcripts.get` knows about a dictation beyond
/// its texts, so the detail needs one request (`docs/api/linux-deltas.md`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemFacts {
    /// ISO 8601 UTC creation time.
    pub created_at: String,
    /// The app id the text went into.
    pub app_id: String,
    /// The app's display name.
    pub app_name: String,
    /// How the item came to be (`dictation`, `audioImport`, `rerun`).
    pub source: String,
    /// The lifecycle status (`transcribing`, `completed`, `failed`).
    pub status: String,
    /// The speech provider.
    pub provider: String,
    /// The model id.
    pub model: String,
    /// The language.
    pub language: String,
    /// The take length in seconds, rounded up.
    pub duration_seconds: u64,
    /// The generated title.
    pub title: String,
    /// The item this one re-ran, when `source` is `rerun`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerun_of: Option<TranscriptRef>,
    /// The retained take.
    pub audio: ItemAudio,
    /// The insertion result as it was reported (`InsertionResult`, with
    /// the Linux `backend` block), absent when nothing was inserted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insertion: Option<serde_json::Value>,
    /// The stop-to-insert split in milliseconds, absent on imports,
    /// re-runs and rows written before it was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timings: Option<ItemTimings>,
    /// The error when `status` is `failed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Whether an item's take is on disk, and why not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemAudio {
    /// True when `path` names a file a re-run can read.
    pub retained: bool,
    /// The WAV file (16 kHz mono) when retained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// `not_retained` when the item never kept audio, `expired` when the
    /// sweep removed it; absent when retained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The split of the path from the key release to the inserted text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemTimings {
    /// From the stop being observed to the take being final.
    pub capture_ms: u64,
    /// The engine's `recognize` call.
    pub transcribe_ms: u64,
    /// The inserter's call.
    pub insert_ms: u64,
    /// The three added: the stop-to-insert time the product promises.
    pub stop_to_insert_ms: u64,
}

/// Linux addition on `transcripts.get` for the meeting kind: what the
/// meeting carries beyond its texts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingExtras {
    /// The user's notes, Markdown.
    pub notes: String,
    /// Who last wrote the notes.
    pub notes_source: crate::methods::meetings_notes::NotesSource,
    /// The analysis, when ready.
    pub analysis: Option<crate::methods::meetings_notes::Analysis>,
    /// Where the analysis stands.
    pub analysis_status: crate::methods::meetings_notes::AnalysisStatus,
    /// ISO 8601 end, once stopped.
    pub ended_at: Option<String>,
    /// The language the meeting was transcribed in.
    pub language: String,
    /// The speech provider frozen at start.
    pub stt_provider_id: String,
    /// The model frozen at start.
    pub stt_model_id: String,
}
