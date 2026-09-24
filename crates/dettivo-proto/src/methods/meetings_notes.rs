//! The Linux meeting additions for notes, analysis and the title
//! (`docs/api/linux-deltas.md`): `meetings.notes.get`, `meetings.notes.set`,
//! `meetings.analyze`, `meetings.analysis.get` and `meetings.rename`, and
//! the analysis shape every `meetings.get`, `transcripts.get` and export
//! carries. Notes are the user's Markdown and never touched by a model;
//! the analysis is the model's summary, decisions and action items over
//! the finalised transcript, regenerated on request; the rename sets the
//! title the app edits inline.

use crate::id::Id;
use crate::runtime::{JobStatus, TranscriptRef};
use serde::{Deserialize, Serialize};

/// The longest notes body `meetings.notes.set` accepts, in bytes (1 MiB).
pub const MAX_NOTES_BYTES: usize = 1024 * 1024;

/// The longest title `meetings.rename` accepts, in characters.
pub const MAX_TITLE_CHARS: usize = 200;

/// `meetings.rename` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingRenameParams {
    /// The meeting.
    pub meeting_id: Id,
    /// The new title; trimmed, at most [`MAX_TITLE_CHARS`] characters,
    /// no control characters.
    pub title: String,
}

/// `meetings.rename` result: the title as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingRenameResult {
    /// The meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// The stored title.
    pub title: String,
    /// `manual` after a rename.
    pub title_source: String,
}

/// Where the notes text came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NotesSource {
    /// Written by the user after the meeting (the default).
    #[default]
    User,
    /// Saved by the live editor while the meeting recorded.
    Live,
}

impl NotesSource {
    /// The stored spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Live => "live",
        }
    }

    /// Parses the stored spelling; anything else is `user`.
    pub fn parse(text: &str) -> Self {
        if text == "live" {
            Self::Live
        } else {
            Self::User
        }
    }
}

/// Where the analysis of a meeting stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    /// No analysis has run.
    #[default]
    None,
    /// The finalisation planned the analysis; it starts next.
    Queued,
    /// The job runs.
    Running,
    /// The analysis is on the row.
    Ready,
    /// The last run failed; `error` says why.
    Failed,
}

impl AnalysisStatus {
    /// The stored spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    /// Parses the stored spelling; anything else is `none`.
    pub fn parse(text: &str) -> Self {
        match text {
            "queued" => Self::Queued,
            "running" => Self::Running,
            "ready" => Self::Ready,
            "failed" => Self::Failed,
            _ => Self::None,
        }
    }
}

/// One action item of an analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionItem {
    /// What is to be done.
    pub text: String,
    /// Who owns it, when the transcript said.
    #[serde(default)]
    pub owner: Option<String>,
    /// When it is due, as the transcript put it.
    #[serde(default)]
    pub due: Option<String>,
}

/// The analysis of a meeting: the macOS shape (`summary`, `decisions`,
/// `action_items`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Analysis {
    /// A few sentences on what the meeting was about and what came of it.
    pub summary: String,
    /// The decisions taken, one line each.
    #[serde(default)]
    pub decisions: Vec<String>,
    /// The action items with their owner and due text when known.
    #[serde(default)]
    pub action_items: Vec<ActionItem>,
}

impl Analysis {
    /// Every line of text the analysis carries, for the search index.
    pub fn searchable_text(&self) -> String {
        let mut out = self.summary.trim().to_string();
        for d in &self.decisions {
            out.push('\n');
            out.push_str(d.trim());
        }
        for a in &self.action_items {
            out.push('\n');
            out.push_str(a.text.trim());
            if let Some(owner) = &a.owner {
                out.push(' ');
                out.push_str(owner.trim());
            }
        }
        out
    }
}

/// `meetings.notes.get` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotesGetParams {
    /// The meeting.
    pub meeting_id: Id,
}

/// `meetings.notes.get` and `meetings.notes.set` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotesResult {
    /// The meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// The notes, Markdown.
    pub markdown: String,
    /// Who last wrote them.
    pub source: NotesSource,
    /// When they were last written; `None` while empty.
    pub updated_at: Option<String>,
}

/// `meetings.notes.set` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotesSetParams {
    /// The meeting.
    pub meeting_id: Id,
    /// The notes, Markdown, at most [`MAX_NOTES_BYTES`].
    pub markdown: String,
    /// Who writes them; absent means `user`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<NotesSource>,
}

/// `meetings.analyze` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalyzeParams {
    /// The meeting.
    pub meeting_id: Id,
    /// Run again over a meeting that already has an analysis; the old one
    /// stays until the new one parses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,
}

/// `meetings.analyze` result: the job that runs, or the reason none does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalyzeResult {
    /// The meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// The analysis job.
    pub job: JobStatus,
    /// Where the analysis stands after this call.
    pub analysis_status: AnalysisStatus,
    /// Why no model was asked, when none was available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<crate::methods::polish::PolishNotice>,
}

/// `meetings.analysis.get` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisGetParams {
    /// The meeting.
    pub meeting_id: Id,
}

/// `meetings.analysis.get` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisGetResult {
    /// The meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// Where the analysis stands.
    pub analysis_status: AnalysisStatus,
    /// The analysis, when `ready` (kept through a failed regenerate).
    pub analysis: Option<Analysis>,
    /// Why the last run failed.
    pub error: Option<String>,
    /// The model that produced the analysis.
    pub model: Option<String>,
    /// When it was produced.
    pub analyzed_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn analysis_parses_the_macos_shape_and_flattens_for_search() {
        let a: Analysis = serde_json::from_value(json!({
            "summary": "Rollout moved.",
            "decisions": ["Ship Thursday"],
            "action_items": [{"text": "Write the runbook", "owner": "Mara", "due": "Wednesday"}, {"text": "Ping ops"}]
        }))
        .unwrap();
        assert_eq!(a.action_items[1].owner, None);
        assert_eq!(
            a.searchable_text(),
            "Rollout moved.\nShip Thursday\nWrite the runbook Mara\nPing ops"
        );
        assert_eq!(AnalysisStatus::parse("ready"), AnalysisStatus::Ready);
        assert_eq!(AnalysisStatus::parse("queued"), AnalysisStatus::Queued);
        assert_eq!(AnalysisStatus::Queued.as_str(), "queued");
        assert_eq!(AnalysisStatus::parse("nope"), AnalysisStatus::None);
        assert_eq!(
            serde_json::to_string(&AnalysisStatus::Queued).unwrap(),
            "\"queued\""
        );
        let p: MeetingRenameParams = serde_json::from_value(
            json!({"meeting_id": "0f8fad5b-d9cb-469f-a165-70867728950e", "title": "Roadmap"}),
        )
        .unwrap();
        assert_eq!(p.title, "Roadmap");
        assert_eq!(MAX_TITLE_CHARS, 200);
        assert_eq!(NotesSource::parse("live").as_str(), "live");
        assert_eq!(
            serde_json::to_string(&AnalysisStatus::Running).unwrap(),
            "\"running\""
        );
    }
}
