//! The dictation item: the macOS `DictationItem` fields as one row of the
//! store, plus the derived title and the lifecycle status a row carries.

use dettivo_proto::methods::meetings::Segment;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::time::now_iso;

/// How an item came to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    /// A microphone dictation through the session.
    #[serde(rename = "dictation")]
    Dictation,
    /// An imported audio file.
    #[serde(rename = "audioImport")]
    AudioImport,
    /// A re-run of another item's retained audio.
    #[serde(rename = "rerun")]
    Rerun,
}

impl SourceKind {
    /// The stored spelling (the macOS enum's raw value).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dictation => "dictation",
            Self::AudioImport => "audioImport",
            Self::Rerun => "rerun",
        }
    }

    /// Parses the stored spelling.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "dictation" => Some(Self::Dictation),
            "audioImport" => Some(Self::AudioImport),
            "rerun" => Some(Self::Rerun),
            _ => None,
        }
    }
}

/// Where an item stands; `transcribing` while a re-run or import job works
/// on it, then `completed` or `failed` with the error fields set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    /// A job is producing the text.
    Transcribing,
    /// The text is final.
    Completed,
    /// The job failed; `error_code` and `error_message` say why.
    Failed,
}

impl ItemStatus {
    /// The stored spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Transcribing => "transcribing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    /// Parses the stored spelling.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "transcribing" => Some(Self::Transcribing),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

/// One dictation item, every field the macOS `DictationItem` carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DictationItem {
    /// The UUID the contract addresses the item by.
    pub id: String,
    /// ISO 8601 UTC creation time.
    pub created_at: String,
    /// ISO 8601 UTC time of the last change.
    pub updated_at: String,
    /// The app id (Wayland app id or X11 class) the text was inserted into.
    pub app_id: String,
    /// The app's display name.
    pub app_name: String,
    /// How the item came to be.
    pub source_kind: SourceKind,
    /// Where the item stands.
    pub status: ItemStatus,
    /// The dictation mode (`raw`, `deterministic_polish`, `enhanced`).
    pub mode: String,
    /// The speech provider (`whisper`).
    pub stt_provider: String,
    /// The model id.
    pub stt_model: String,
    /// The language the engine reported or was told.
    pub language: String,
    /// The engine's text before the pipeline.
    pub raw_text: String,
    /// The text after the pipeline, what was inserted.
    pub final_text: String,
    /// A generated title.
    pub title: String,
    /// A generated summary (empty until the language model spec).
    pub summary: String,
    /// The take length.
    pub duration_ms: u64,
    /// The item this one re-ran, when `source_kind` is `rerun`.
    pub rerun_of_item_id: Option<String>,
    /// A stable error code when `status` is `failed`.
    pub error_code: Option<String>,
    /// The error message when `status` is `failed`.
    pub error_message: Option<String>,
    /// The retained audio file, when kept.
    pub audio_path: Option<String>,
    /// The contract's `InsertionResult`, as it was reported.
    pub insertion: Option<Value>,
    /// The segments (with words when the engine aligned them) an import
    /// or re-run produced; empty for a session dictation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segments: Vec<Segment>,
    /// The hash of the Polish policy the session froze (ADR 0023);
    /// absent for a row written before the Polish layers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_hash: Option<String>,
    /// Why the text is what it is: the string `silent` when an import's
    /// audio never reached the engine, or `{ kind, reason }` when the
    /// Enhanced pass inserted the deterministic result instead of a
    /// rewrite; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<Value>,
    /// The split of the path from the key release to the inserted text
    /// (migration 0004); absent on imports, re-runs and rows written
    /// before it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timings: Option<Timings>,
}

/// The stop-to-insert split in milliseconds, as the completion event
/// reports it: draining the take, the engine's recognition, the
/// inserter's call. Stop to insert is the sum of the three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Timings {
    /// From the stop being observed to the take being final.
    pub capture_ms: u64,
    /// The engine's `recognize` call.
    pub transcribe_ms: u64,
    /// The inserter's call.
    pub insert_ms: u64,
}

impl Timings {
    /// The whole path from the key release to the inserted text.
    pub fn stop_to_insert_ms(&self) -> u64 {
        self.capture_ms + self.transcribe_ms + self.insert_ms
    }
}

impl DictationItem {
    /// A new item with a fresh id, timestamps now, empty text and the
    /// status `completed`; the caller fills the rest.
    pub fn new(source_kind: SourceKind) -> Self {
        let now = now_iso();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: now.clone(),
            updated_at: now,
            app_id: String::new(),
            app_name: String::new(),
            source_kind,
            status: ItemStatus::Completed,
            mode: "raw".into(),
            stt_provider: String::new(),
            stt_model: String::new(),
            language: String::new(),
            raw_text: String::new(),
            final_text: String::new(),
            title: String::new(),
            summary: String::new(),
            duration_ms: 0,
            rerun_of_item_id: None,
            error_code: None,
            error_message: None,
            audio_path: None,
            insertion: None,
            segments: Vec::new(),
            policy_hash: None,
            notice: None,
            timings: None,
        }
    }

    /// Sets the title from the final text when none was given.
    pub fn with_derived_title(mut self) -> Self {
        if self.title.is_empty() {
            self.title = title_for(&self.final_text);
        }
        self
    }

    /// The contract's `duration_seconds`, rounded.
    pub fn duration_seconds(&self) -> u64 {
        self.duration_ms.div_ceil(1000)
    }
}

/// The first words of the text, at most 48 characters on a word boundary;
/// `Dictation` for an empty text.
pub fn title_for(text: &str) -> String {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut out = String::new();
    for word in line.split_whitespace() {
        let next = if out.is_empty() {
            word.to_string()
        } else {
            format!("{out} {word}")
        };
        if next.chars().count() > 48 {
            break;
        }
        out = next;
    }
    if out.is_empty() {
        if line.trim().is_empty() {
            "Dictation".to_string()
        } else {
            line.trim().chars().take(48).collect()
        }
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_come_from_the_first_words_and_kinds_round_trip() {
        assert_eq!(title_for(""), "Dictation");
        assert_eq!(title_for("Hello world."), "Hello world.");
        let long = "one two three four five six seven eight nine ten eleven twelve";
        assert_eq!(
            title_for(long),
            "one two three four five six seven eight nine ten"
        );
        assert_eq!(title_for("\n\nsecond line first"), "second line first");
        for kind in [
            SourceKind::Dictation,
            SourceKind::AudioImport,
            SourceKind::Rerun,
        ] {
            assert_eq!(SourceKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(
            serde_json::to_string(&SourceKind::AudioImport).unwrap(),
            "\"audioImport\""
        );
        assert_eq!(ItemStatus::parse("failed"), Some(ItemStatus::Failed));
        let item = DictationItem::new(SourceKind::Dictation);
        assert_eq!(item.id.len(), 36);
        assert_eq!(item.with_derived_title().title, "Dictation");
    }
}
