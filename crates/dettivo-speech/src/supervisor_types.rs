//! The supervisor's plain types: its `[engines]` settings, what it
//! reports about one engine (`EngineStatus`), what the language model
//! engine's `load` carries (`LlmLoad`), and the transition the event
//! stream sees with the hook that receives it. `supervisor` re-exports
//! them, so `supervisor::Settings` stays the path.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dettivo_engine_proto::Backend;

/// Supervisor settings, from `[engines]`.
#[derive(Debug, Clone)]
pub struct Settings {
    /// Where engine binaries live besides `PATH`.
    pub directory: Option<PathBuf>,
    /// Idle time after which a speech engine is unloaded and terminated.
    pub stt_idle: Duration,
    /// The same for the language model engine (`[engines] llm_idle_seconds`).
    pub llm_idle: Duration,
    /// Engines skip GPU backends.
    pub force_cpu: bool,
    /// Longest wait for a load.
    pub load_timeout: Duration,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            directory: None,
            stt_idle: Duration::from_secs(300),
            llm_idle: Duration::from_secs(600),
            force_cpu: false,
            load_timeout: Duration::from_secs(120),
        }
    }
}

/// What the supervisor knows about one engine, for `dettivo doctor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineStatus {
    /// Binary name.
    pub binary: String,
    /// Resolved path, when found.
    pub path: Option<PathBuf>,
    /// Running right now.
    pub running: bool,
    /// Loaded model.
    pub model: Option<String>,
    /// Backend of the last load.
    pub backend: Option<Backend>,
    /// Reason for that backend.
    pub reason: Option<String>,
    /// The LoRA adapter applied over the loaded model, when one was.
    pub lora: Option<String>,
    /// Consecutive crashes.
    pub crashes: u32,
    /// Marked degraded (three consecutive crashes).
    pub degraded: bool,
    /// The process's resident set in bytes while it runs.
    pub memory_bytes: Option<u64>,
}

/// What the language model engine's `load` carries beyond the model
/// file: the context length and, for a sideloaded fine-tune shipped as an
/// adapter (ADR 0032), the GGUF LoRA path. The speech engines pass the
/// default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LlmLoad {
    /// Context length in tokens; the engine's default when absent.
    pub context_length: Option<u32>,
    /// A GGUF LoRA adapter applied over the model at load.
    pub lora: Option<String>,
}

/// An engine process transition, for the event stream (`engine.state`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineTransition {
    /// The binary.
    pub binary: String,
    /// `spawned`, `loaded`, `unloaded`, `crashed`, `degraded`.
    pub state: &'static str,
    /// The model, when loaded.
    pub model: Option<String>,
    /// The backend, when loaded.
    pub backend: Option<Backend>,
    /// The reason, when any.
    pub reason: Option<String>,
}

/// Receives engine transitions.
pub type StateHook = Arc<dyn Fn(EngineTransition) + Send + Sync>;
