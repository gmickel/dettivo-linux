//! Typed payloads for every message and event, and the catalogue that
//! names them. Segments carry word timestamps with confidence when the
//! engine aligns words. Requests: `load`, `unload`, `recognize`, `generate`,
//! `diarize`, `cancel`, `status`. Events: `progress`, `partial`, `loaded`,
//! `error`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Which compute backend an engine may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackendPreference {
    /// Vulkan when a device enumerates and the model fits, else CPU.
    #[default]
    Auto,
    /// Vulkan or fail.
    Vulkan,
    /// CUDA or fail (diarization only).
    Cuda,
    /// CPU only (also what `DETTIVO_FORCE_CPU=1` selects).
    Cpu,
}

/// The backend an engine ended up on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    /// ggml Vulkan.
    Vulkan,
    /// ONNX Runtime CUDA.
    Cuda,
    /// CPU.
    Cpu,
}

/// `load`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoadParams {
    /// Path of the model file (or directory for multi-file models).
    pub model: String,
    /// Backend preference.
    #[serde(default, alias = "provider")]
    pub backend_preference: BackendPreference,
    /// Optional VAD model path for engines that use one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vad_model: Option<String>,
    /// Context length in tokens for the LLM engine; the engine's default
    /// when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,
    /// A GGUF LoRA adapter the LLM engine applies over the model at load
    /// (a sideloaded fine-tune, ADR 0032); absent for a fused model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lora: Option<String>,
    /// Threads the diarization engine runs its models on; 0 or absent is
    /// the engine's default (the core count, capped at four).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads: Option<u32>,
}

/// `loaded` event and the `load` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoadedResult {
    /// The model that is loaded.
    pub model: String,
    /// The backend in use.
    pub backend: Backend,
    /// Why that backend (device found, forced, fallback...).
    pub reason: String,
    /// Why an automatic diarization load fell back to the CPU.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
}

/// `recognize` (with one `pcm16k` attachment).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecognizeParams {
    /// Language code, or `auto`.
    #[serde(default = "auto")]
    pub language: String,
    /// Vocabulary prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// Ask for segment timestamps.
    #[serde(default = "yes")]
    pub timestamps: bool,
}

fn auto() -> String {
    "auto".into()
}

fn yes() -> bool {
    true
}

/// One recognized word with its timestamps and confidence, from an engine
/// that aligns words (Parakeet); Whisper segments carry no words.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Word {
    /// Start in milliseconds.
    pub start_ms: u64,
    /// End in milliseconds.
    pub end_ms: u64,
    /// The word.
    pub text: String,
    /// Confidence in (0, 1].
    pub confidence: f64,
}

/// One recognized segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    /// Start in milliseconds.
    pub start_ms: u64,
    /// End in milliseconds.
    pub end_ms: u64,
    /// Text.
    pub text: String,
    /// Word timestamps, when the engine aligns words; absent otherwise.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<Word>,
}

/// `recognize` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecognizeResult {
    /// Full text.
    pub text: String,
    /// Detected or requested language.
    pub language: String,
    /// Segments with timestamps.
    pub segments: Vec<Segment>,
    /// Audio duration in milliseconds.
    pub duration_ms: u64,
    /// Backend that ran.
    pub backend: Backend,
}

/// How the LLM engine turns a `generate` request into model input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PromptProfile {
    /// The Polish profile: `system` and `user` go through the model's
    /// chat template with thinking switched off, and the answer starts
    /// as the assistant turn.
    #[default]
    Polish,
    /// `user` is the whole prompt, tokenized as is; `system` is ignored.
    Raw,
}

/// `generate` (the LLM engine).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerateParams {
    /// Which prompt profile builds the model input.
    #[serde(default)]
    pub prompt_profile: PromptProfile,
    /// The system prompt (the Polish profile).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// The user turn, or the whole prompt under the raw profile.
    pub user: String,
    /// The most tokens the answer may have.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Sampling temperature; 0 is greedy.
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    /// Strings that end the answer when they appear (not included).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<String>,
}

fn default_max_tokens() -> u32 {
    1024
}

fn default_temperature() -> f64 {
    0.2
}

/// Why a generation ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// The model ended its turn or a stop string appeared.
    Stop,
    /// `max_tokens` (or the context) ran out.
    Length,
    /// A `cancel` arrived mid-generation; `text` holds what came before it.
    Cancelled,
}

/// `generate` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerateResult {
    /// The generated text, raw (thinking blocks included when the model
    /// emitted them; the language crate strips them).
    pub text: String,
    /// Tokens generated.
    pub tokens: u32,
    /// Why generation ended.
    pub finish_reason: FinishReason,
    /// Backend that ran.
    pub backend: Backend,
}

/// `diarize` (with one `pcm16k` attachment).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DiarizeParams {
    /// Expected speaker count, when known; the clustering decides otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speakers: Option<u32>,
    /// The clustering distance threshold when the count is not known
    /// (smaller finds more speakers); the engine's default when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clustering_threshold: Option<f64>,
}

/// One diarized turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeakerTurn {
    /// Start in milliseconds.
    pub start_ms: u64,
    /// End in milliseconds.
    pub end_ms: u64,
    /// Speaker label.
    pub speaker: String,
}

/// `diarize` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiarizeResult {
    /// Turns.
    pub turns: Vec<SpeakerTurn>,
}

/// `cancel`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelParams {
    /// The request to cancel.
    pub request_id: u64,
}

/// `status` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusResult {
    /// A model is loaded.
    pub loaded: bool,
    /// Which model.
    pub model: Option<String>,
    /// Which backend.
    pub backend: Option<Backend>,
    /// A request is running.
    pub busy: bool,
    /// Bytes in use on the backend device the model sits on, when the
    /// backend reports it (the LLM engine; VRAM under Vulkan).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_bytes: Option<u64>,
}

/// `progress` event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressEvent {
    /// The request.
    pub request_id: u64,
    /// 0..1.
    pub fraction: f64,
    /// Chunks finished, on a request that counts them (`diarize`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed: Option<u32>,
    /// Chunks in total, on a request that counts them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
}

/// `partial` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartialEvent {
    /// The request.
    pub request_id: u64,
    /// Text so far.
    pub text: String,
}

/// `error` event and error responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorEvent {
    /// The request, or 0 for the engine as a whole.
    pub request_id: u64,
    /// A stable code: `model_missing`, `load_failed`, `cancelled`, `bad_request`, `internal`.
    pub code: String,
    /// A message with no transcript, prompt or audio in it.
    pub message: String,
}

/// Empty payload.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

/// Every request name.
pub const REQUESTS: &[&str] = &[
    "load",
    "unload",
    "recognize",
    "generate",
    "diarize",
    "cancel",
    "status",
];
/// Every event name.
pub const EVENTS: &[&str] = &["progress", "partial", "loaded", "error"];

/// Round-trips a request payload through its type, naming the field on drift.
pub fn check_request(name: &str, payload: &Value) -> Result<Value, String> {
    fn rt<T: serde::de::DeserializeOwned + Serialize>(v: &Value) -> Result<Value, String> {
        let typed: T = serde_path_to_error::deserialize(v.clone()).map_err(|e| e.to_string())?;
        serde_json::to_value(typed).map_err(|e| e.to_string())
    }
    match name {
        "load" => rt::<LoadParams>(payload),
        "unload" | "status" => rt::<Empty>(payload),
        "recognize" => rt::<RecognizeParams>(payload),
        "generate" => rt::<GenerateParams>(payload),
        "diarize" => rt::<DiarizeParams>(payload),
        "cancel" => rt::<CancelParams>(payload),
        other => Err(format!("unknown request {other:?}")),
    }
}

/// Round-trips a response payload through its type.
pub fn check_response(name: &str, payload: &Value) -> Result<Value, String> {
    fn rt<T: serde::de::DeserializeOwned + Serialize>(v: &Value) -> Result<Value, String> {
        let typed: T = serde_path_to_error::deserialize(v.clone()).map_err(|e| e.to_string())?;
        serde_json::to_value(typed).map_err(|e| e.to_string())
    }
    match name {
        "load" => rt::<LoadedResult>(payload),
        "unload" | "cancel" => rt::<Empty>(payload),
        "recognize" => rt::<RecognizeResult>(payload),
        "generate" => rt::<GenerateResult>(payload),
        "diarize" => rt::<DiarizeResult>(payload),
        "status" => rt::<StatusResult>(payload),
        "error" => rt::<ErrorEvent>(payload),
        other => Err(format!("unknown response {other:?}")),
    }
}

/// Round-trips an event payload through its type.
pub fn check_event(name: &str, payload: &Value) -> Result<Value, String> {
    fn rt<T: serde::de::DeserializeOwned + Serialize>(v: &Value) -> Result<Value, String> {
        let typed: T = serde_path_to_error::deserialize(v.clone()).map_err(|e| e.to_string())?;
        serde_json::to_value(typed).map_err(|e| e.to_string())
    }
    match name {
        "progress" => rt::<ProgressEvent>(payload),
        "partial" => rt::<PartialEvent>(payload),
        "loaded" => rt::<LoadedResult>(payload),
        "error" => rt::<ErrorEvent>(payload),
        other => Err(format!("unknown event {other:?}")),
    }
}
