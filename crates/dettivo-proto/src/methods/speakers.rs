//! The speaker half of `meetings.*` (Linux additions, ADR 0035, registered
//! in `docs/api/linux-deltas.md`): `meetings.diarize` runs or re-runs the
//! post-meeting speaker pass as a job, `meetings.speakers.list` reads a
//! meeting's speakers and its diarization block, `meetings.speakers.rename`
//! names one across the meeting, and `meetings.speakers.suggest` offers the
//! names used before. The shapes `meetings.get` carries (`speakers`,
//! `diarization`) live here too.

use crate::id::Id;
use crate::runtime::{JobStatus, TranscriptRef};
use serde::{Deserialize, Serialize};

/// The longest speaker name `meetings.speakers.rename` accepts.
pub const MAX_NAME_CHARS: usize = 64;

/// One speaker of a meeting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Speaker {
    /// The stable id: `you` for the microphone, `speaker_00`, `speaker_01`,
    /// ... for the diarized speakers in order of first appearance.
    pub speaker_id: String,
    /// The name shown: the label (`You`, `Speaker 1`) until renamed.
    pub name: String,
    /// The swatch index, in order of first appearance (`you` is 0).
    pub color_index: u32,
    /// Milliseconds of speech assigned to the speaker.
    pub talk_ms: u64,
}

/// Where a meeting's diarization stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiarizationStatus {
    /// The pass has not run.
    None,
    /// The finalisation planned the pass; it starts next.
    Queued,
    /// The pass runs; `job.progress` carries `stage = diarizing`.
    Running,
    /// The speakers are on the row.
    Ready,
    /// The pass failed; `error` says why.
    Failed,
    /// The model set is not downloaded; `error` names the command.
    Unavailable,
}

impl DiarizationStatus {
    /// The wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Unavailable => "unavailable",
        }
    }

    /// Parses the wire spelling; anything else is `none`.
    pub fn parse(text: &str) -> Self {
        match text {
            "queued" => Self::Queued,
            "running" => Self::Running,
            "ready" => Self::Ready,
            "failed" => Self::Failed,
            "unavailable" => Self::Unavailable,
            _ => Self::None,
        }
    }
}

/// The diarization block of a meeting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diarization {
    /// The status.
    pub status: DiarizationStatus,
    /// The share of the diarized track inside a speaker turn, once ready.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<f64>,
    /// The engine binary that ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    /// The model set that ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// ISO 8601 UTC time the pass ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ran_at: Option<String>,
    /// Why the pass failed or is unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The speaker count the meeting was started with, when given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_speakers: Option<u32>,
    /// Run the pass by itself once the meeting completes (the start's
    /// `diarize`, else `[meetings.diarization] auto`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto: Option<bool>,
}

impl Default for Diarization {
    fn default() -> Self {
        Self {
            status: DiarizationStatus::None,
            coverage: None,
            engine: None,
            model: None,
            ran_at: None,
            error: None,
            expected_speakers: None,
            auto: None,
        }
    }
}

/// `meetings.diarize` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiarizeParams {
    /// The completed meeting.
    pub meeting_id: Id,
    /// The speaker count when known; absent means the meeting's own, else
    /// the configuration's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speakers: Option<u32>,
}

/// `meetings.diarize` result: the job that runs the pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiarizeResult {
    /// The meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// The running job (`job_diarize_<n>`).
    pub job: JobStatus,
}

/// `meetings.speakers.list` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeakersListParams {
    /// The meeting.
    pub meeting_id: Id,
}

/// `meetings.speakers.list` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeakersListResult {
    /// The meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// The speakers, `you` first, then in order of first appearance.
    pub speakers: Vec<Speaker>,
    /// The diarization block.
    pub diarization: Diarization,
}

/// `meetings.speakers.rename` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameParams {
    /// The meeting.
    pub meeting_id: Id,
    /// The speaker.
    pub speaker_id: String,
    /// The new name; empty restores the label. At most 64 characters.
    pub name: String,
}

/// `meetings.speakers.rename` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameResult {
    /// The meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// The speaker after the rename.
    pub speaker: Speaker,
    /// How many segments now carry the name.
    pub segments_updated: u32,
}

/// `meetings.speakers.suggest` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SuggestParams {
    /// Names starting with this (case-insensitive); absent means every name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// The most names to return; 10 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// One remembered name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suggestion {
    /// The name as it was last given.
    pub name: String,
    /// ISO 8601 UTC time it was last used.
    pub last_used_at: String,
    /// How many renames used it.
    pub uses: u32,
}

/// `meetings.speakers.suggest` result: most recently used first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestResult {
    /// The names.
    pub names: Vec<Suggestion>,
}

/// The label a speaker id carries until it is renamed: `You` for the
/// microphone, `Speaker N` (counting from one) for a diarized speaker.
pub fn default_label(speaker_id: &str) -> String {
    if speaker_id == "you" {
        return "You".into();
    }
    speaker_id
        .strip_prefix("speaker_")
        .and_then(|n| n.parse::<u32>().ok())
        .map(|n| format!("Speaker {}", n + 1))
        .unwrap_or_else(|| speaker_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn labels_follow_the_ids_and_the_blocks_omit_what_is_absent() {
        assert_eq!(default_label("you"), "You");
        assert_eq!(default_label("speaker_00"), "Speaker 1");
        assert_eq!(default_label("speaker_11"), "Speaker 12");
        assert_eq!(default_label("odd"), "odd");
        let d = Diarization::default();
        assert_eq!(serde_json::to_value(&d).unwrap(), json!({"status": "none"}));
        let p: SuggestParams = serde_json::from_value(json!({})).unwrap();
        assert_eq!(p, SuggestParams::default());
        assert_eq!(MAX_NAME_CHARS, 64);
        assert_eq!(
            DiarizationStatus::parse("queued"),
            DiarizationStatus::Queued
        );
        assert_eq!(DiarizationStatus::Queued.as_str(), "queued");
        assert_eq!(DiarizationStatus::parse("nope"), DiarizationStatus::None);
        assert_eq!(
            serde_json::to_string(&DiarizationStatus::Queued).unwrap(),
            "\"queued\""
        );
    }
}
