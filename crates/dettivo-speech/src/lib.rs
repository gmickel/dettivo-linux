//! Speech recognition for the daemon (ADR 0003): the `SttEngine` trait that
//! mirrors the macOS `STTEngine` capability contract, `EngineProcess` (a
//! child speaking the engine protocol over stdio), the `Supervisor` that
//! starts, keeps warm, unloads and restarts those processes, the
//! `WhisperEngine` and `ParakeetEngine` recognizers that implement the
//! trait through it (`engines`), the `LlmEngine` that drives the language
//! model binary over the same supervisor (`llm`), the `DiarizeEngine`
//! that drives the diarization binary (`diarize`, ADR 0035), the
//! coalescing `Preloader`, and the hardware tier the benchmarks read
//! (`tier`).

pub mod catalogue;
pub mod diarize;
pub mod download;
pub mod engines;
pub mod fixture_server;
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
}
