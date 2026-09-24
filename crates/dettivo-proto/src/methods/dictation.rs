//! `dictation.*`: `start`, `stop`, `cancel`, `status`
//! (`docs/api/dettivo-ipc-v1.md` section 8.2).

use crate::runtime::{ContextPack, JobStatus, TranscriptRef};
use serde::{Deserialize, Serialize};

/// Insertion/output mode for a dictation session. `raw` and `polish` are
/// the contract's own spellings; `deterministic_polish` and `enhanced`
/// are the macOS mode identifiers this port answers to as well
/// (`docs/api/linux-deltas.md`), with `polish` an accepted alias for
/// `deterministic_polish`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DictationMode {
    /// Insert the raw transcript.
    Raw,
    /// The contract's spelling of the deterministic Polish pass.
    Polish,
    /// The deterministic Polish pass, no model involved.
    DeterministicPolish,
    /// The Polish pass rewritten by a language model.
    Enhanced,
}

impl DictationMode {
    /// The identifier the session, the history item and the completion
    /// event carry: `polish` resolves to `deterministic_polish`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Polish | Self::DeterministicPolish => "deterministic_polish",
            Self::Enhanced => "enhanced",
        }
    }

    /// The mode an identifier names, or `None`.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "raw" => Some(Self::Raw),
            "polish" | "deterministic_polish" => Some(Self::DeterministicPolish),
            "enhanced" => Some(Self::Enhanced),
            _ => None,
        }
    }

    /// The valid spellings, for an error message.
    pub fn names() -> &'static str {
        "raw, polish, deterministic_polish, enhanced"
    }
}

/// `dictation.start` params. The two guards are Linux additions
/// (`docs/api/linux-deltas.md`): a caller that captured the focused window
/// at hotkey time passes it here, and the daemon probes it itself
/// otherwise; the insertion at the end of the session is refused with
/// `CONFLICT` when the focus moved. `language` and `mode` may be left
/// out on Linux (a second delta): the session then runs on `[dictation]
/// language` and `[dictation] mode`, the values the hotkey path uses, so
/// a client that shows the configured mode starts that mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartParams {
    /// BCP-47 language tag; empty or absent means `[dictation] language`.
    #[serde(default)]
    pub language: String,
    /// Output mode; absent means `[dictation] mode`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<DictationMode>,
    /// Linux addition: the app id (window class) the text must land in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_target_bundle_id: Option<String>,
    /// Linux addition: the pid of the window the text must land in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_target_pid: Option<String>,
}

/// The window a session captured at its start (Linux addition, reported
/// by `dictation.status` while the session runs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionTarget {
    /// The Wayland app id or X11 window class, when known.
    pub app_id: Option<String>,
    /// The owning process id, when known.
    pub pid: Option<u32>,
}

/// `dictation.start` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartResult {
    /// The started job's status.
    pub job: JobStatus,
}

/// `dictation.stop` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopResult {
    /// The completed dictation item's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// The finishing job's status.
    pub job: JobStatus,
    /// Linux addition: how the insertion went, when text was handed over
    /// (`outcome = failed` with a `CONFLICT: ...` reason when the focus
    /// moved away from the window captured at start).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insertion: Option<crate::runtime::InsertionResult>,
}

/// `dictation.cancel` parameters. A guard confines cleanup to its own job.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelParams {
    /// Cancel only this active job; a different job is `NOT_FOUND`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_job_id: Option<String>,
}

/// `dictation.cancel` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelResult {
    /// The cancelled job's status.
    pub job: JobStatus,
}

/// `dictation.status` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusResult {
    /// Whether a dictation session is currently active.
    pub is_active: bool,
    /// The active job's status, when active.
    pub job: Option<JobStatus>,
    /// Captured context around the active session, when available.
    pub context_pack: Option<ContextPack>,
    /// Linux addition: the window captured when the session started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<SessionTarget>,
}

/// `dictation.reinsert_last` result (Linux addition): the last transcript
/// handed to the inserter again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReinsertLastResult {
    /// The transcript's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// How the insertion went.
    pub insertion: crate::runtime::InsertionResult,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_omitted_mode_means_the_configured_one_and_stays_omitted() {
        let p: StartParams = serde_json::from_value(json!({"language": "en"})).unwrap();
        assert_eq!(p.mode, None);
        assert_eq!(serde_json::to_value(&p).unwrap(), json!({"language": "en"}));
        let raw: StartParams =
            serde_json::from_value(json!({"language": "", "mode": "raw"})).unwrap();
        assert_eq!(raw.mode, Some(DictationMode::Raw));
        let unknown = json!({"language": "en", "mode": "meeting"});
        assert!(serde_json::from_value::<StartParams>(unknown).is_err());
    }
}

/// A toggle starts with configured defaults or finishes the active take.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToggleResult {
    /// A newly started take.
    Started(StartResult),
    /// A finished take.
    Stopped(Box<StopResult>),
}
