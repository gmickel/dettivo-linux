//! `llm.*` (Linux additions, `docs/api/linux-deltas.md`): which language
//! model providers the Enhanced pass can reach, the trust gate in front
//! of any endpoint that is not on this machine, the language model
//! catalogue (`llm.models.*`, the speech model shapes over the `llm`
//! provider) and the local engine's state (`llm.engine.status`).

use serde::{Deserialize, Serialize};

use super::speech::{EngineBackend, ModelStatus};

/// One provider and whether a rewrite would reach it right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    /// `local`, `ollama`, `openai_compatible` or `mock`.
    pub id: String,
    /// The model it would use.
    pub model: String,
    /// Whether it answers right now.
    pub available: bool,
    /// One line saying what answered, or what did not.
    pub detail: String,
    /// What to do about it, when there is something to do.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// `llm.providers.list` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvidersListResult {
    /// Every provider `[llm]` describes, in the order `auto` tries them.
    pub providers: Vec<Provider>,
    /// The provider a session starting now would use, when one answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<String>,
}

/// `llm.endpoints.trust` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointsTrustParams {
    /// The endpoint to confirm; a loopback endpoint is already trusted.
    pub url: String,
}

/// `llm.endpoints.trust` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointsTrustResult {
    /// The canonical form stored in `[llm] trusted_endpoints`.
    pub url: String,
    /// Always `true` on success.
    pub trusted: bool,
}

/// One entry of the trust list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Endpoint {
    /// The canonical endpoint.
    pub url: String,
    /// Whether it answers on this machine (and needs no confirmation).
    pub loopback: bool,
}

/// `llm.endpoints.list` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointsListResult {
    /// The endpoints the user confirmed, plus the configured loopback
    /// ones that never needed confirming.
    pub endpoints: Vec<Endpoint>,
}

/// `llm.models.status` params: every catalogue language model, or one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelsStatusParams {
    /// Only this model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// `llm.models.status` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelsStatusResult {
    /// Every model asked for, catalogue order; `is_selected` marks
    /// `[llm] model`.
    pub models: Vec<ModelStatus>,
    /// The models directory.
    pub models_dir: String,
    /// The catalogue version.
    pub catalogue_version: u32,
    /// `[llm] model`.
    pub selected: String,
    /// The model meeting analysis loads (`[llm] analysis_model`, or
    /// `selected` when empty).
    pub analysis_model: String,
    /// `[llm] polish_experiment`: the sideload manifest Enhanced runs on
    /// instead of `selected`, empty when none (ADR 0032).
    pub polish_experiment: String,
}

/// `llm.models.download` and `llm.models.cancel` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelParams {
    /// The catalogue model id.
    pub model: String,
}

/// `llm.models.delete` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelDeleteParams {
    /// The catalogue model id.
    pub model: String,
    /// Delete even the selected model.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub force: bool,
}

/// `llm.models.delete` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelDeleteResult {
    /// A file was removed.
    pub deleted: bool,
    /// The model after the deletion.
    pub model: ModelStatus,
}

/// `llm.engine.status` result: the supervisor's row for the language
/// model engine plus what the running process reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineStatusResult {
    /// The binary name (`dettivo-engine-llm`).
    pub binary: String,
    /// Where it was found, when it was.
    pub path: Option<String>,
    /// The process runs.
    pub running: bool,
    /// A model is loaded in it.
    pub loaded: bool,
    /// It is serving a request.
    pub busy: bool,
    /// The loaded model file.
    pub model: Option<String>,
    /// The backend of the last load.
    pub backend: Option<EngineBackend>,
    /// Why that backend.
    pub reason: Option<String>,
    /// The GGUF LoRA adapter applied over the loaded model, when one is
    /// (a `gguf-lora` sideload, ADR 0032).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lora: Option<String>,
    /// Bytes in use on the backend device, when the engine reports them.
    pub memory_bytes: Option<u64>,
    /// Consecutive crashes.
    pub crashes: u32,
    /// Degraded after repeated crashes.
    pub degraded: bool,
    /// `[engines] llm_idle_seconds`: the idle after which the daemon
    /// unloads the engine.
    pub idle_seconds: u64,
}
