//! `speech.*`: provider discovery and selection, adopted from the Windows
//! port so an agent written against either port drives model selection the
//! same way (ADR 0008; registered in `docs/api/linux-deltas.md` with the
//! Windows commit pin). The shapes mirror the Windows payload builders in
//! `Dettivo.Windows/Services/SpeechProviderCatalogService.cs` and
//! `LocalRestRpcService.cs`.

use serde::{Deserialize, Serialize};

/// Where a speech provider runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Runs on this machine.
    Local,
    /// Calls a hosted API.
    Cloud,
}

/// One selectable model under a provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderModel {
    /// Model id.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Whether the model is on disk and ready to use.
    pub is_downloaded: bool,
    /// Whether this is the recommended default for its provider.
    pub is_recommended: bool,
    /// Approximate on-disk size in bytes, when known.
    pub approximate_size_bytes: Option<u64>,
    /// Free disk space a download needs, when known.
    pub required_free_bytes: Option<u64>,
    /// Whether the model is English-only.
    pub is_english_only: bool,
    /// Local path of the model file, when downloaded.
    pub path: Option<String>,
}

/// One speech provider entry from `speech.providers.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderDescriptor {
    /// Provider id, for example `"local"` or `"parakeet"`.
    pub id: String,
    /// Display name.
    pub display_name: String,
    /// Local or cloud.
    pub kind: ProviderKind,
    /// Whether the provider's runtime is usable right now.
    pub runtime_available: bool,
    /// Whether the provider serves dictation.
    pub supports_dictation: bool,
    /// Whether the provider serves meeting transcription.
    pub supports_meetings: bool,
    /// Whether the provider emits segment timestamps.
    pub supports_timestamps: bool,
    /// Whether the provider supports streaming recognition.
    pub supports_streaming: bool,
    /// Whether models may be deleted from disk.
    pub supports_model_deletion: bool,
    /// The provider's default model id.
    pub default_model_id: String,
    /// Why the provider is unavailable, when it is.
    pub unavailable_reason: Option<String>,
    /// Selectable models under this provider.
    pub models: Vec<ProviderModel>,
}

/// One speaker-diarization model from `speech.providers.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeakerModel {
    /// Model id.
    pub id: String,
    /// Display name.
    pub display_name: String,
    /// One-line description.
    pub detail: String,
    /// Which platform serves it.
    pub platform: String,
    /// Whether the diarization runtime is usable right now.
    pub runtime_available: bool,
    /// Whether the model is on disk.
    pub is_downloaded: bool,
    /// Whether the model can be downloaded through the daemon.
    pub supports_download: bool,
    /// Whether fallback speaker labels can be refreshed with it.
    pub can_refresh_fallback_labels: bool,
    /// On-disk size in bytes, when known.
    pub size_bytes: Option<u64>,
    /// Local path of the model, when downloaded.
    pub path: Option<String>,
    /// Why the model is unavailable, when it is.
    pub unavailable_reason: Option<String>,
    /// What happens when the model is unavailable.
    pub fallback_behavior: Option<String>,
    /// What the user should do to make it available.
    pub recommended_action: Option<String>,
}

/// `speech.providers.list` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvidersListResult {
    /// Available speech providers.
    pub providers: Vec<ProviderDescriptor>,
    /// Available speaker-diarization models.
    pub speaker_models: Vec<SpeakerModel>,
    /// The default provider id.
    pub default_provider_id: String,
    /// The default model id.
    pub default_model_id: String,
}

/// `speech.selection.set` params. Every field is optional and the daemon
/// requires at least one, answering `INVALID_PARAMS` otherwise.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionSetParams {
    /// New provider id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// New primary (Whisper) model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// New Parakeet model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parakeet_model_id: Option<String>,
    /// New meeting-specific model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meeting_model: Option<String>,
    /// New cloud model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_model_id: Option<String>,
}

impl SelectionSetParams {
    /// Whether any field is set; the daemon rejects an empty request.
    pub fn is_empty(&self) -> bool {
        self.provider.is_none()
            && self.model.is_none()
            && self.parakeet_model_id.is_none()
            && self.meeting_model.is_none()
            && self.cloud_model_id.is_none()
    }
}

/// The resolved provider and model for one usage (dictation or meeting).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedSelection {
    /// Resolved provider id.
    pub provider_id: String,
    /// Resolved model id.
    pub model_id: String,
    /// Resolved language tag.
    pub language: String,
    /// Whether the resolved provider is hosted.
    pub is_cloud: bool,
    /// Whether the resolved provider is Parakeet.
    pub is_parakeet: bool,
    /// Whether the resolved provider is local Whisper.
    pub is_local_whisper: bool,
}

/// `speech.selection.get` and `speech.selection.set` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionResult {
    /// Selected provider, display form.
    pub provider: String,
    /// Selected provider id.
    pub provider_id: String,
    /// Selected primary model id.
    pub model: String,
    /// Selected Whisper model id.
    pub whisper_model_id: String,
    /// Selected Parakeet model id.
    pub parakeet_model_id: String,
    /// Selected meeting-specific model id.
    pub meeting_speech_model_id: String,
    /// The model id meetings resolve to.
    pub meeting_effective_model_id: String,
    /// Selected cloud model id.
    pub cloud_model_id: String,
    /// Resolved dictation provider id.
    pub dictation_provider_id: String,
    /// Resolved dictation model id.
    pub dictation_model_id: String,
    /// Full resolved dictation selection.
    pub dictation: ResolvedSelection,
    /// Full resolved meeting selection.
    pub meeting: ResolvedSelection,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_set_params_are_all_optional() {
        let params: SelectionSetParams = serde_json::from_str("{}").unwrap();
        assert!(params.is_empty());
        let params: SelectionSetParams =
            serde_json::from_str(r#"{"provider":"parakeet"}"#).unwrap();
        assert!(!params.is_empty());
    }
}

/// The compute backend an engine process runs on (Linux addition).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineBackend {
    /// ggml Vulkan.
    Vulkan,
    /// ONNX Runtime CUDA (diarization).
    Cuda,
    /// CPU.
    Cpu,
}

/// One engine process as the supervisor sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineStatus {
    /// Binary name (`dettivo-engine-whisper`).
    pub binary: String,
    /// Resolved path, when found.
    pub path: Option<String>,
    /// Linux addition (S-31): the directory that answered the search
    /// (`[engines] directory`, the CUDA drop-in, the package's engine
    /// directory, the daemon's directory or a `PATH` entry); `null` when
    /// the binary was not found.
    pub directory: Option<String>,
    /// The process is running right now.
    pub running: bool,
    /// The loaded model path.
    pub model: Option<String>,
    /// The backend of the last load.
    pub backend: Option<EngineBackend>,
    /// Why that backend.
    pub reason: Option<String>,
    /// Consecutive crashes.
    pub crashes: u32,
    /// Three consecutive crashes; cleared by a successful load.
    pub degraded: bool,
    /// Linux addition (S-16): the process's resident set in bytes while it
    /// runs, `null` otherwise.
    pub memory_bytes: Option<u64>,
}

/// `speech.engines` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnginesResult {
    /// Every engine the daemon knows, found or not.
    pub engines: Vec<EngineStatus>,
}

/// Where a catalogue model stands on disk (Linux addition).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelReadiness {
    /// On disk and verified against the catalogue checksum.
    Ready,
    /// On disk, not verified yet.
    Unverified,
    /// A download runs.
    Downloading,
    /// A partial file waits for a resume.
    Partial,
    /// Not on disk.
    Missing,
    /// Failed verification and was moved aside.
    Quarantined,
}

/// One catalogue model with its readiness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelStatus {
    /// The provider (`whisper`, `parakeet`).
    pub provider: String,
    /// The model id, portable across the ports.
    pub id: String,
    /// The name to show.
    pub display_name: String,
    /// The catalogue size.
    pub size_bytes: u64,
    /// Readiness.
    pub readiness: ModelReadiness,
    /// Bytes on disk for a download or a partial file.
    pub bytes_done: u64,
    /// Bytes expected.
    pub bytes_total: u64,
    /// The file when it is on disk.
    pub path: Option<String>,
    /// The license identifier.
    pub license: String,
    /// The provider's default model.
    pub is_default: bool,
    /// Selected for dictation.
    pub is_selected: bool,
    /// Offered for download (`false` until the engine that reads it ships).
    pub available: bool,
    /// The last failure, when any.
    pub error: Option<String>,
    /// What the model is for: `stt`, `vad`, `speaker` or `llm`.
    #[serde(default = "stt")]
    pub kind: String,
    /// The languages the catalogue lists (`en`, `multilingual`).
    #[serde(default)]
    pub languages: Vec<String>,
    /// The catalogue's one-line recommendation, on the models first run
    /// offers; absent on the rest.
    #[serde(default)]
    pub recommended_for: Option<String>,
    /// Linux addition on `llm.models.status` rows: `catalogue` for a
    /// catalogue model, `sideload` for the fine-tune `[llm]
    /// polish_experiment` names (ADR 0032); absent on the speech rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Linux addition on a sideload row: `gguf` (fused) or `gguf-lora`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Linux addition on a `gguf-lora` sideload row: the catalogue base
    /// the adapter applies over.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_model: Option<String>,
    /// Linux addition on a sideload row: why the manifest did not
    /// resolve, when it did not; Enhanced runs on `[llm] model` then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_error: Option<String>,
}

fn stt() -> String {
    "stt".to_string()
}

/// `speech.models.status` params: narrow to a provider or one model.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelsStatusParams {
    /// Only this provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Only this model (with `provider`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// `speech.models.status` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelsStatusResult {
    /// Every model asked for, catalogue order.
    pub models: Vec<ModelStatus>,
    /// The models directory.
    pub models_dir: String,
    /// The catalogue version.
    pub catalogue_version: u32,
}

/// `speech.models.download` and `speech.models.cancel` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelParams {
    /// The provider.
    pub provider: String,
    /// The model id.
    pub model: String,
}

/// `speech.models.delete` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelDeleteParams {
    /// The provider.
    pub provider: String,
    /// The model id.
    pub model: String,
    /// Delete even the selected model.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub force: bool,
}

/// `speech.models.delete` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelDeleteResult {
    /// A file was removed.
    pub deleted: bool,
    /// The model after the deletion.
    pub model: ModelStatus,
}
