//! The engine half of the `config.toml` schema: `[engines]` with its
//! per-engine sections, `[speech]` and `[models]`. `schema`
//! re-exports every type here, so `config::schema::Speech` stays the path.

use serde::{Deserialize, Serialize};

/// `[engines]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Engines {
    /// Directory searched for engine binaries before the daemon's own
    /// directory and `PATH`; empty means those two only.
    pub directory: String,
    /// Idle seconds after which a speech engine is unloaded and terminated.
    pub stt_idle_seconds: u64,
    /// Idle seconds after which the language model engine is unloaded.
    pub llm_idle_seconds: u64,
    /// `[engines.whisper]`: the Whisper engine's backend.
    pub whisper: EngineSection,
    /// `[engines.parakeet]`: the Parakeet engine's backend.
    pub parakeet: EngineSection,
    /// `[engines.llm]`: the language model engine's backend, context
    /// length and answer budget.
    pub llm: LlmEngineSection,
    /// `[engines.diarize]`: the diarization engine's backend and thread count.
    pub diarize: DiarizeEngineSection,
}

impl Default for Engines {
    fn default() -> Self {
        Self {
            directory: String::new(),
            stt_idle_seconds: 300,
            llm_idle_seconds: 600,
            whisper: EngineSection::default(),
            parakeet: EngineSection::default(),
            llm: LlmEngineSection::default(),
            diarize: DiarizeEngineSection::default(),
        }
    }
}

/// `[engines.diarize]`: what the diarization engine is told at every
/// load (ADR 0035, amended by ADR 0057).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DiarizeEngineSection {
    /// The ONNX Runtime backend; independent of the ggml engines' Vulkan choice.
    pub backend: DiarizeBackend,
    /// Threads per model; 0 is the core count, capped at four.
    pub threads: u32,
}

/// The diarization engine's ONNX Runtime provider preference.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiarizeBackend {
    /// CUDA when its provider and runtime load, else CPU with the reason.
    #[default]
    Auto,
    /// CPU only.
    Cpu,
    /// CUDA or a failed load naming the unavailable dependency.
    Cuda,
}

/// `[engines.llm]`: what the language model engine is told at every load
/// and every request (ADR 0026).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LlmEngineSection {
    /// The compute backend, as for the speech engines.
    pub backend: EngineBackend,
    /// The context length in tokens the model is loaded with.
    pub context_length: u32,
    /// The most tokens one rewrite may generate.
    pub max_tokens: u32,
}

impl Default for LlmEngineSection {
    fn default() -> Self {
        Self {
            backend: EngineBackend::Auto,
            context_length: 4096,
            max_tokens: 1024,
        }
    }
}

/// `[engines.<name>]`: what one ggml engine is told at every load.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EngineSection {
    /// The compute backend: `auto` (Vulkan when a device enumerates, else
    /// CPU), `vulkan` (Vulkan or a failed load), `cpu`.
    pub backend: EngineBackend,
}

/// The backend preference an engine load carries (FR-E9).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineBackend {
    /// Vulkan when a device enumerates and the model fits, else CPU.
    #[default]
    Auto,
    /// Vulkan or a failed load.
    Vulkan,
    /// CPU only.
    Cpu,
}

/// `[speech]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Speech {
    /// Speech provider: `whisper` or `parakeet`.
    pub provider: String,
    /// Model id within the provider (a directory under `<models>/<provider>/`).
    pub model: String,
    /// Whisper meeting model; empty inherits the dictation model and provider.
    pub meeting_model: String,
    /// The Parakeet model `speech.selection.set { provider: "parakeet" }`
    /// switches to when `model` is not a Parakeet model.
    pub parakeet_model_id: String,
}

impl Default for Speech {
    fn default() -> Self {
        Self {
            provider: "whisper".into(),
            model: "large-v3-turbo".into(),
            meeting_model: String::new(),
            parakeet_model_id: "parakeet-v3".into(),
        }
    }
}

/// `[models]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Models {
    /// Downloads that may run at once.
    pub max_concurrent_downloads: u32,
    /// Re-hash models on disk that have no verified manifest at start.
    pub verify_on_start: bool,
    /// A catalogue file to use instead of the built-in one (mirrors, tests).
    pub catalogue_file: String,
}

impl Default for Models {
    fn default() -> Self {
        Self {
            max_concurrent_downloads: 1,
            verify_on_start: true,
            catalogue_file: String::new(),
        }
    }
}
