//! Speech recognition for the daemon (ADR 0003): the `SttEngine` trait that
//! mirrors the macOS `STTEngine` capability contract, `EngineProcess` (a
//! child speaking the engine protocol over stdio), the `Supervisor` that
//! starts, keeps warm, unloads and restarts those processes, the
//! `WhisperEngine` and `ParakeetEngine` recognizers that implement the
//! trait through it (`engines`), the `LlmEngine` that drives the language
//! model binary over the same supervisor (`llm`), the `DiarizeEngine`
//! that drives the diarization binary (`diarize`, ADR 0035), the
//! coalescing `Preloader`, the hardware tier the benchmarks read
//! (`tier`), and the speech gate a dictation take passes before a
//! recognizer hears it (`gate`).

pub mod catalogue;
pub mod diarize;
pub mod download;
pub mod engines;
pub mod fixture_server;
pub mod gate;
pub mod llm;
pub mod models;
pub mod preload;
pub mod process;
pub mod supervisor;
mod supervisor_types;
pub mod tier;

use std::path::PathBuf;
use std::time::Duration;

use dettivo_engine_proto::{Backend, RecognizeResult};

/// Package name of this crate, as declared in its manifest.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Package names of the workspace crates this crate builds on.
pub const UPSTREAM: &[&str] = &[dettivo_proto::CRATE_NAME, dettivo_engine_proto::CRATE_NAME];

/// The capability contract every engine reports (macOS `STTEngine`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    /// Segment timestamps are returned.
    pub supports_timestamps: bool,
    /// Partial results stream during recognition.
    pub supports_streaming: bool,
    /// Language codes, or empty for "any".
    pub supported_languages: Vec<String>,
    /// Longest audio one request may carry.
    pub max_duration_seconds: u64,
    /// A vocabulary prompt is honoured.
    pub supports_custom_vocabulary: bool,
    /// The model can be deleted through the catalogue.
    pub supports_model_deletion: bool,
}

/// What a session asks a recognizer for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecognizeRequest {
    /// 16 kHz mono samples.
    pub pcm: Vec<i16>,
    /// Language code or `auto`.
    pub language: String,
    /// Vocabulary prompt.
    pub prompt: Option<String>,
    /// Segment timestamps wanted.
    pub timestamps: bool,
}

/// The longest vocabulary prompt, in characters. Whisper keeps the end of
/// a prompt that overflows its window and loses the first terms, which are
/// the most important ones.
pub const PROMPT_MAX_CHARS: usize = 500;

/// The vocabulary terms the prompt carries: whole terms in list order
/// until the next one would take the prompt past [`PROMPT_MAX_CHARS`].
pub fn prompt_terms(words: &[String]) -> &[String] {
    let mut len = 0;
    for (i, word) in words.iter().enumerate() {
        // The separator before the term, and the full stop the prompt may need.
        len += word.chars().count() + if i == 0 { 0 } else { 2 };
        if len + 1 > PROMPT_MAX_CHARS {
            return &words[..i];
        }
    }
    words
}

/// The prompt a vocabulary primes the engine with: the [`prompt_terms`]
/// joined by `", "` and closed with a full stop, or `None` for no terms.
/// Whisper reads an unterminated list as an open sentence, and it pulled
/// some English takes into German even with the language forced (issue #37).
pub fn vocabulary_prompt(words: &[String]) -> Option<String> {
    let terms = prompt_terms(words);
    if terms.is_empty() {
        return None;
    }
    let mut prompt = terms.join(", ");
    if !prompt.ends_with(['.', '!', '?']) {
        prompt.push('.');
    }
    Some(prompt)
}

/// Why an engine call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// The engine binary is not on `PATH` or in the configured directory; names both.
    NotFound {
        /// The binary name.
        binary: String,
        /// Where it was looked for.
        searched: Vec<PathBuf>,
    },
    /// The model file is missing.
    ModelMissing(String),
    /// The engine reported an error (`code`, `message`).
    Engine {
        /// Stable code from the protocol.
        code: String,
        /// Message, free of transcript text.
        message: String,
    },
    /// The engine process died; the last redacted stderr lines follow.
    Crashed(String),
    /// The request was cancelled.
    Cancelled,
    /// The engine is degraded after repeated crashes.
    Degraded,
    /// The engine is serving another request right now.
    Busy,
    /// Transport failure.
    Transport(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { binary, searched } => write!(
                f,
                "engine {binary} not found (searched {})",
                searched
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::ModelMissing(m) => write!(f, "model missing: {m}"),
            Self::Engine { code, message } => write!(f, "engine error {code}: {message}"),
            Self::Crashed(tail) => write!(f, "engine crashed: {tail}"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Degraded => write!(f, "engine degraded after repeated crashes"),
            Self::Busy => write!(f, "engine busy with another request"),
            Self::Transport(m) => write!(f, "engine transport: {m}"),
        }
    }
}

impl std::error::Error for EngineError {}

/// Where a preload came from (macOS contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreloadSource {
    /// Daemon start.
    Startup,
    /// The user picked a model.
    ModelSelection,
    /// A session is about to start.
    SessionStart,
}

/// A speech recognizer the daemon can drive.
pub trait SttEngine: Send + Sync {
    /// The provider name (`whisper`, `parakeet`).
    fn provider(&self) -> &str;
    /// Capabilities.
    fn capabilities(&self) -> Capabilities;
    /// Loads the model so the first recognition is fast; never downloads.
    fn preload(&self, source: PreloadSource) -> Result<(), EngineError>;
    /// Recognizes one request.
    fn recognize(
        &self,
        request: RecognizeRequest,
        timeout: Duration,
    ) -> Result<RecognizeResult, EngineError>;
    /// Cancels the running request, if any.
    fn cancel(&self);
    /// The backend of the loaded model, when loaded.
    fn backend(&self) -> Option<Backend>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "dettivo-speech");
    }

    #[test]
    fn upstream_edges_resolve() {
        assert_eq!(super::UPSTREAM, &["dettivo-proto", "dettivo-engine-proto"]);
    }

    fn prompt(words: &[&str]) -> Option<String> {
        super::vocabulary_prompt(&words.iter().map(|w| w.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn vocabulary_prompt_is_none_without_words() {
        assert_eq!(prompt(&[]), None);
    }

    #[test]
    fn vocabulary_prompt_ends_in_a_full_stop() {
        assert_eq!(prompt(&["Dettivo"]).as_deref(), Some("Dettivo."));
        assert_eq!(
            prompt(&["Dettivo", "Astra", "Jev", "Clef"]).as_deref(),
            Some("Dettivo, Astra, Jev, Clef.")
        );
    }

    /// R5: a list that overflows the cap keeps whole terms in list order,
    /// stays terminated and never passes 500 characters.
    #[test]
    fn vocabulary_prompt_is_capped_at_whole_terms() {
        let words: Vec<String> = (0..60).map(|i| format!("Term{i:03}xyz")).collect();
        let capped = super::vocabulary_prompt(&words).unwrap();
        assert!(
            capped.chars().count() <= super::PROMPT_MAX_CHARS,
            "{capped}"
        );
        assert!(capped.starts_with("Term000xyz, Term001xyz,"));
        assert!(capped.ends_with("xyz."), "{capped}");
        let kept = super::prompt_terms(&words);
        assert_eq!(capped, format!("{}.", kept.join(", ")));
        // One more term would have passed the cap.
        let longer = format!("{}, {}.", kept.join(", "), words[kept.len()]);
        assert!(longer.chars().count() > super::PROMPT_MAX_CHARS);
        // A term longer than the cap leaves no prompt.
        assert_eq!(prompt(&[&"x".repeat(500)]), None);
        assert_eq!(prompt(&[&"x".repeat(499)]).unwrap().len(), 500);
    }

    #[test]
    fn vocabulary_prompt_keeps_a_closing_mark_the_last_word_has() {
        assert_eq!(prompt(&["Astra", "Clef."]).as_deref(), Some("Astra, Clef."));
        assert_eq!(
            prompt(&["Astra", "Yahoo!"]).as_deref(),
            Some("Astra, Yahoo!")
        );
        assert_eq!(
            prompt(&["Astra", "Jeopardy?"]).as_deref(),
            Some("Astra, Jeopardy?")
        );
    }
}
