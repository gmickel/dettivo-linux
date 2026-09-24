//! The dictation session (FR-H1, FR-E7, FR-M1 to FR-M5, FR-A2, FR-A4):
//! one session per daemon that records through the audio layer,
//! transcribes through the frozen engine and model, runs the text through
//! the mode step in `dettivo-language` (the raw layer, the deterministic
//! Polish pass and the Enhanced rewrite), keeps the result as the last
//! transcript and hands it to an inserter. Every transition is reported
//! to a publisher so the event stream, the OSD and the bar see the same
//! state.

pub mod machine;
pub mod source;
mod worker;

use std::path::PathBuf;
use std::time::Duration;

use dettivo_core::config::polish_schema::Llm;
use dettivo_language::enhanced::Notice;
use dettivo_language::policy::RulesConfig;
use dettivo_proto::runtime::{
    ContextPack, ContextPackMetrics, ContextPackSource, ContextPackStatus, InsertionBackend,
    InsertionMethod, InsertionOutcome, InsertionResult, TargetApp,
};
use serde::{Deserialize, Serialize};

/// Where a session stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// No session.
    Idle,
    /// Capturing the microphone.
    Recording,
    /// The engine works on the take.
    Transcribing,
    /// The text is being handed to the inserter.
    Inserting,
    /// Cancelled; a transient exit reported once.
    Cancelled,
    /// Failed; a transient exit reported once with its reason.
    Failed,
}

impl State {
    /// The wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Recording => "recording",
            Self::Transcribing => "transcribing",
            Self::Inserting => "inserting",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

/// The policy a session freezes at start (FR-E7, FR-M5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    /// The provider id.
    pub provider: String,
    /// The model id.
    pub model: String,
    /// The model file the engine loads.
    pub model_path: PathBuf,
    /// Language code or `auto`.
    pub language: String,
    /// The mode identifier on the wire (`raw`, `deterministic_polish`,
    /// `enhanced`).
    pub mode: String,
    /// The vocabulary prompt words.
    pub vocabulary: Vec<String>,
    /// Replacement pairs applied after recognition.
    pub replacements: Vec<(String, String)>,
    /// Spoken punctuation is converted.
    pub spoken_punctuation: bool,
    /// Protected tokens pass through the raw layer byte for byte.
    pub protect_tokens: bool,
    /// The longest take before the session stops itself.
    pub max_duration: Duration,
    /// A take whose peak stays under this is treated as silence.
    pub silence_peak_threshold: f32,
    /// Keep the take after the session (FR-A4).
    pub keep_audio: bool,
    /// The meter interval.
    pub level_interval_ms: u64,
    /// A stable hash of the configuration the policy came from.
    pub config_hash: String,
    /// The `[polish]` rules the session froze at its start (FR-M5).
    pub rules: RulesConfig,
    /// The `[llm]` section the session froze at its start (FR-M6).
    pub llm: Llm,
}

/// A finished transcript kept as the session's last result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transcript {
    /// The session's job id.
    pub job_id: String,
    /// The dictation item id (a UUID-shaped string).
    pub id: String,
    /// What was inserted: the text the mode step produced.
    pub text: String,
    /// The engine's text before the mode step.
    pub raw_text: String,
    /// The text after the deterministic Polish pass, when the mode ran
    /// one.
    #[serde(default)]
    pub polished_text: Option<String>,
    /// Why the Enhanced pass inserted the deterministic result instead of
    /// a rewrite (FR-M3).
    #[serde(default)]
    pub notice: Option<Notice>,
    /// The hash of the Polish policy the session resolved (FR-M5).
    #[serde(default)]
    pub policy_hash: Option<String>,
    /// The language the engine reported.
    pub language: String,
    /// The take length.
    pub duration_ms: u64,
    /// The take was near silent and not sent to the engine.
    pub silent: bool,
    /// The policy the session ran under.
    pub policy: Policy,
    /// The insertion outcome.
    pub insertion: Option<InsertionResult>,
    /// The window captured at start.
    #[serde(default)]
    pub target: Option<SessionTarget>,
    /// Where the time after the key release went (Linux addition to the
    /// completion event).
    #[serde(default)]
    pub timings: Option<SessionTimings>,
}

/// The split of the path from the key release to the inserted text, in
/// milliseconds: draining and finalising the take after the stop, the
/// engine's recognition, and the inserter's call. Recorded on every
/// completed session; the QA rig reads it for the NFR-1 measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SessionTimings {
    /// From the stop being observed to the take being final.
    pub capture_ms: u64,
    /// The engine's `recognize` call; 0 for a silent take.
    pub transcribe_ms: u64,
    /// The inserter's call; 0 when there was nothing to insert.
    pub insert_ms: u64,
}

/// A state change for the publisher. The completion transition (into
/// `idle` from `inserting`) carries the insertion outcome (with the backend
/// that performed it) and the first words, so the OSD needs no second
/// request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateChange {
    /// The session's job id.
    pub job_id: String,
    /// The state entered.
    pub state: State,
    /// The state left.
    pub previous: State,
    /// Why, when it matters (`device lost: <name>`, `cancelled`, `loading engine`).
    pub reason: Option<String>,
    /// The insertion outcome, on the completion transition.
    pub insertion: Option<InsertionResult>,
    /// The first sentence of the transcript, bounded to one line.
    pub first_words: Option<String>,
    /// The split of the release-to-inserted path, on the completion
    /// transition.
    pub timings: Option<SessionTimings>,
    /// The mode the session ran in, on the completion transition.
    pub mode: Option<String>,
    /// Why the Enhanced pass fell back, on the completion transition.
    pub notice: Option<Notice>,
    /// The Polish policy's hash, on the completion transition.
    pub policy_hash: Option<String>,
}

impl StateChange {
    /// A transition without completion fields.
    pub fn plain(job_id: &str, state: State, previous: State, reason: Option<String>) -> Self {
        Self {
            job_id: job_id.to_string(),
            state,
            previous,
            reason,
            insertion: None,
            first_words: None,
            timings: None,
            mode: None,
            notice: None,
            policy_hash: None,
        }
    }
}

/// The longest `first_words` string, in characters.
pub const FIRST_WORDS_MAX_CHARS: usize = 72;

/// The first sentence of `text`, cut at the first sentence end and then at
/// [`FIRST_WORDS_MAX_CHARS`] with an ellipsis, so the OSD never shows more
/// than one line. Whitespace is collapsed.
pub fn first_words(text: &str) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut end = collapsed.len();
    let mut truncated = false;
    for (i, ch) in collapsed.char_indices() {
        if matches!(ch, '.' | '!' | '?') {
            let rest = &collapsed[i + ch.len_utf8()..];
            if rest.is_empty() || rest.starts_with(' ') {
                end = i + ch.len_utf8();
                truncated = !rest.is_empty();
                break;
            }
        }
    }
    let mut out: String = collapsed[..end].to_string();
    if out.chars().count() > FIRST_WORDS_MAX_CHARS {
        out = out.chars().take(FIRST_WORDS_MAX_CHARS - 1).collect();
        out = out.trim_end().to_string();
        truncated = true;
    }
    if truncated && !out.ends_with('\u{2026}') {
        out.push('\u{2026}');
    }
    out
}

/// A meter sample for the publisher.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Level {
    /// Root mean square, 0..1.
    pub rms: f32,
    /// Peak, 0..1.
    pub peak: f32,
}

/// Why a session call was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// A session is active; `start` is refused (`CONFLICT`).
    Active,
    /// No session; `stop` and `cancel` are refused (`NOT_FOUND`).
    NoSession,
    /// Nothing was dictated yet (`reinsert_last`).
    NoTranscript,
    /// The selected model is not on disk; names it and the action.
    ModelMissing {
        /// `provider/model`.
        model: String,
        /// What to run.
        recommended_action: String,
    },
    /// The mode identifier is not one the session knows.
    ModeUnknown(String),
    /// The capture could not start.
    Audio(String),
    /// The session ended `failed` with this reason (`stop` reports it).
    Failed(String),
    /// The session was cancelled meanwhile (`stop` reports it).
    Cancelled,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "a dictation session is active"),
            Self::NoSession => write!(f, "no dictation session is active"),
            Self::NoTranscript => write!(f, "nothing has been dictated yet"),
            Self::ModelMissing {
                model,
                recommended_action,
            } => write!(f, "model {model} is not downloaded; {recommended_action}"),
            Self::ModeUnknown(m) => write!(
                f,
                "mode {m} is not a dictation mode; the modes are {}",
                dettivo_language::pipeline::Mode::names()
            ),
            Self::Audio(e) => write!(f, "capture failed: {e}"),
            Self::Failed(r) => write!(f, "the session failed: {r}"),
            Self::Cancelled => write!(f, "the session was cancelled"),
        }
    }
}

impl std::error::Error for SessionError {}

/// The window a session captured when it started (FR-H2, S-08 guards):
/// the inserter compares it with the focused window at insertion time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SessionTarget {
    /// The app id or window class, when known.
    pub app_id: Option<String>,
    /// The owning pid, when known.
    pub pid: Option<u32>,
    /// The window's identity as the probe knows it, so the insertion
    /// lands in the window the key went down over and not in another
    /// window of the same process. Never on the wire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,
    /// The probe could not name the origin when the session started (it
    /// failed, or nothing was focused): the text goes to the clipboard
    /// rather than to whatever window has the focus later.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unverified: bool,
}

impl SessionTarget {
    /// The target of a session whose origin could not be verified.
    pub fn unverified() -> Self {
        Self {
            unverified: true,
            ..Self::default()
        }
    }

    /// True when no window is named (an unverified origin included).
    pub fn is_empty(&self) -> bool {
        self.app_id.is_none() && self.pid.is_none() && self.window.is_none()
    }
}

/// Hands a transcript to the desktop. The daemon wires the backend chain
/// in; this crate ships a clipboard-only implementation.
pub trait Inserter: Send + Sync {
    /// Inserts `text` into `target` (the window captured at start; `None`
    /// means no guard), reporting the contract-shaped result.
    fn insert(&self, text: &str, target: Option<&SessionTarget>) -> InsertionResult;
}

/// Keeps a finished transcript: the history store writes the item and
/// moves the retained take out of `take_dir` before the session reports
/// its final state. An error is reported in that state's `reason` and
/// never undoes the insertion. Staging is discarded after the attempt. The
/// latest transcript remains recoverable through `Session::last` and
/// reinsertion until another completion replaces it or the daemon exits.
pub trait Archive: Send + Sync {
    /// Stores `transcript`; `take_dir` holds the session's takes.
    fn archive(&self, transcript: &Transcript, take_dir: &std::path::Path) -> Result<(), String>;
}

/// Receives state changes and levels.
pub trait Publisher: Send + Sync {
    /// A session state change.
    fn state(&self, change: &StateChange);
    /// A meter sample while recording.
    fn level(&self, level: Level);
}

/// Copies the text to the Wayland clipboard and reports
/// `copied_to_clipboard`; without a Wayland display it reports `failed`.
#[derive(Debug, Default)]
pub struct ClipboardOnly;

/// The context pack Linux reports until a capture adapter exists: off,
/// with the reason.
pub fn no_context_pack() -> ContextPack {
    ContextPack {
        status: ContextPackStatus::Off,
        reason: Some("context capture arrives with the insertion spec".into()),
        source: ContextPackSource {
            adapter_id: "linux".into(),
            app_class: "generic".into(),
            bundle_id: String::new(),
        },
        metrics: ContextPackMetrics {
            capture_duration_ms: 0,
            payload_bytes: 0,
            character_count: 0,
            token_budget: 0,
            token_estimate: 0,
        },
    }
}

impl Inserter for ClipboardOnly {
    fn insert(&self, text: &str, _target: Option<&SessionTarget>) -> InsertionResult {
        use wl_clipboard_rs::copy::{MimeType, Options, Source};
        let target = TargetApp {
            bundle_id: String::new(),
            name: String::new(),
        };
        match Options::new().copy(Source::Bytes(text.as_bytes().into()), MimeType::Text) {
            Ok(()) => {
                tracing::info!(
                    chars = text.chars().count(),
                    "transcript copied to the clipboard"
                );
                InsertionResult {
                    outcome: InsertionOutcome::CopiedToClipboard,
                    method: InsertionMethod::ClipboardOnly,
                    target_app: target,
                    context_pack: no_context_pack(),
                    reason: None,
                    backend: Some(clipboard_backend()),
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "clipboard unavailable; transcript kept only");
                InsertionResult {
                    outcome: InsertionOutcome::Failed,
                    method: InsertionMethod::ClipboardOnly,
                    target_app: target,
                    context_pack: no_context_pack(),
                    reason: Some(format!("clipboard unavailable: {e}")),
                    backend: Some(clipboard_backend()),
                }
            }
        }
    }
}

/// The backend block `ClipboardOnly` reports.
fn clipboard_backend() -> InsertionBackend {
    InsertionBackend {
        name: "clipboard".into(),
        latency_ms: 0,
        undo_supported: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_words_is_one_sentence_on_one_line() {
        assert_eq!(
            first_words("Add a regression test. Then ship it."),
            "Add a regression test.\u{2026}"
        );
        assert_eq!(
            first_words("  ask   not what\n your country  "),
            "ask not what your country"
        );
        assert_eq!(first_words("Version 2.0 is out"), "Version 2.0 is out");
        let long = "word ".repeat(40);
        let cut = first_words(&long);
        assert!(cut.ends_with('\u{2026}') && cut.chars().count() <= FIRST_WORDS_MAX_CHARS);
        assert_eq!(first_words(""), "");
    }
}
