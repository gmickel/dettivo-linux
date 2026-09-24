//! `speech.*`: the Windows-adopted `providers.list`, `selection.get` and
//! `selection.set`, the Linux additions `models.status|download|cancel|
//! delete`, and `speech.engines`.

use dettivo_core::config::edit;
use dettivo_engine_proto::Backend;
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::speech::{
    EngineBackend, EngineStatus, EnginesResult, ModelDeleteParams, ModelDeleteResult, ModelParams,
    ModelReadiness, ModelStatus, ModelsStatusParams, ModelsStatusResult, ProviderDescriptor,
    ProviderKind, ProviderModel, ProvidersListResult, ResolvedSelection, SelectionResult,
    SelectionSetParams,
};
use serde_json::Value;

use super::{json, params};

use crate::daemon::Daemon;
use crate::models::ModelError;

/// `INVALID_PARAMS` with a stable reason; the reason is part of the
/// message so clients can show it, the details stay empty as the fixtures
/// record.
fn invalid(message: String, _reason: &str) -> JsonRpcError {
    JsonRpcError::new(AppCode::InvalidParams, message, ErrorDetails::empty())
}

fn model_error(e: ModelError) -> JsonRpcError {
    let message = e.to_string();
    match e {
        ModelError::Unknown(_) => invalid(message, "unknown_model"),
        ModelError::Unavailable(_) => invalid(message, "model_unavailable"),
        ModelError::Selected(_) => {
            JsonRpcError::new(AppCode::Conflict, message, ErrorDetails::empty())
        }
        ModelError::Busy(_) => JsonRpcError::new(
            AppCode::Conflict,
            message,
            ErrorDetails::conflict_kind("downloadRunning"),
        ),
        ModelError::Io(_) => {
            JsonRpcError::new(AppCode::InternalError, message, ErrorDetails::empty())
        }
    }
}

/// Display name per provider id.
fn provider_display(id: &str) -> String {
    match id {
        "whisper" => "Whisper (local)".into(),
        "parakeet" => "Parakeet (local)".into(),
        other => other.to_string(),
    }
}

/// The dictation selection `(provider, model)` in force.
fn selected(daemon: &Daemon) -> (String, String) {
    let speech = daemon.config().config.speech;
    (speech.provider, speech.model)
}

/// `speech.engines`.
pub fn engines(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let engines = daemon
        .engines()
        .status()
        .into_iter()
        .map(|s| EngineStatus {
            binary: s.binary,
            directory: s
                .path
                .as_deref()
                .and_then(std::path::Path::parent)
                .map(|d| d.to_string_lossy().into_owned()),
            path: s.path.map(|p| p.to_string_lossy().into_owned()),
            running: s.running,
            model: s.model,
            backend: s.backend.map(|b| match b {
                Backend::Vulkan => EngineBackend::Vulkan,
                Backend::Cuda => EngineBackend::Cuda,
                Backend::Cpu => EngineBackend::Cpu,
            }),
            reason: s.reason,
            crashes: s.crashes,
            degraded: s.degraded,
            memory_bytes: s.memory_bytes,
        })
        .collect();
    json(EnginesResult { engines })
}

/// `speech.providers.list`: every provider with its models and their
/// readiness from the model service.
pub fn providers_list(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let (sel_provider, sel_model) = selected(daemon);
    let store = daemon.models().store();
    let catalogue = store.catalogue();
    let engine_rows = daemon.engines().status();
    let mut providers = Vec::new();
    for provider in catalogue.stt_providers() {
        let binary = format!("dettivo-engine-{provider}");
        let engine = engine_rows.iter().find(|e| e.binary == binary);
        let entries = catalogue.models_of(&provider);
        let available = entries.iter().any(|m| m.available);
        let runtime_available = available && engine.is_some_and(|e| e.path.is_some());
        let caps = dettivo_speech::engines::capabilities_of(&provider);
        let unavailable_reason = if !available {
            Some("no model is offered for download yet".to_string())
        } else if !runtime_available {
            Some(format!("{binary} not found"))
        } else if engine.is_some_and(|e| e.degraded) {
            Some(format!("{binary} is degraded after repeated crashes"))
        } else {
            None
        };
        let default = catalogue
            .default_for(&provider)
            .map(|m| m.id.clone())
            .unwrap_or_default();
        let models = daemon
            .models()
            .status(Some(&provider), None, (&sel_provider, &sel_model))
            .into_iter()
            .filter(|m| {
                entries
                    .iter()
                    .any(|e| e.id == m.id && e.kind == dettivo_speech::catalogue::ModelKind::Stt)
            })
            .map(|m| {
                let entry = catalogue.find(&m.provider, &m.id);
                ProviderModel {
                    id: m.id.clone(),
                    label: m.display_name.clone(),
                    is_downloaded: matches!(
                        m.readiness,
                        ModelReadiness::Ready | ModelReadiness::Unverified
                    ),
                    is_recommended: m.is_default,
                    approximate_size_bytes: Some(m.size_bytes),
                    required_free_bytes: Some(m.size_bytes + m.size_bytes / 10),
                    is_english_only: entry.is_some_and(|e| e.english_only),
                    path: m.path.clone(),
                }
            })
            .collect();
        providers.push(ProviderDescriptor {
            id: provider.clone(),
            display_name: provider_display(&provider),
            kind: ProviderKind::Local,
            runtime_available,
            supports_dictation: true,
            supports_meetings: dettivo_speech::engines::meeting_capable(&provider),
            supports_timestamps: caps.supports_timestamps,
            supports_streaming: caps.supports_streaming,
            supports_model_deletion: caps.supports_model_deletion,
            default_model_id: default,
            unavailable_reason,
            models,
        });
    }
    json(ProvidersListResult {
        providers,
        speaker_models: Vec::new(),
        default_provider_id: "whisper".into(),
        default_model_id: catalogue
            .default_for("whisper")
            .map(|m| m.id.clone())
            .unwrap_or_default(),
    })
}

fn resolved(provider: &str, model: &str) -> ResolvedSelection {
    ResolvedSelection {
        provider_id: provider.to_string(),
        model_id: model.to_string(),
        language: "auto".into(),
        is_cloud: false,
        is_parakeet: provider == "parakeet",
        is_local_whisper: provider == "whisper",
    }
}

/// `speech.selection.get`: the `[speech]` keys resolved for dictation and
/// meetings.
pub fn selection_get(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let speech = daemon.config().config.speech;
    let store = daemon.models().store();
    let catalogue = store.catalogue();
    let meeting_model = if speech.meeting_model.is_empty() {
        speech.model.clone()
    } else {
        speech.meeting_model.clone()
    };
    let whisper_model = if speech.provider == "whisper" {
        speech.model.clone()
    } else {
        catalogue
            .default_for("whisper")
            .map(|m| m.id.clone())
            .unwrap_or_default()
    };
    json(SelectionResult {
        provider: provider_display(&speech.provider),
        provider_id: speech.provider.clone(),
        model: speech.model.clone(),
        whisper_model_id: whisper_model,
        parakeet_model_id: speech.parakeet_model_id.clone(),
        meeting_speech_model_id: speech.meeting_model.clone(),
        meeting_effective_model_id: meeting_model.clone(),
        cloud_model_id: String::new(),
        dictation_provider_id: speech.provider.clone(),
        dictation_model_id: speech.model.clone(),
        dictation: resolved(&speech.provider, &speech.model),
        meeting: resolved(
            if speech.meeting_model.is_empty() {
                &speech.provider
            } else {
                "whisper"
            },
            &meeting_model,
        ),
    })
}

/// `speech.selection.set`: validates against the catalogue, writes the
/// `[speech]` keys through the config layer (comments kept), reloads (which
/// preloads a changed model when its file exists) and answers the
/// selection in force.
pub fn selection_set(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: SelectionSetParams = params(params_value)?;
    if p.is_empty() {
        return Err(invalid(
            "At least one speech selection field is required".into(),
            "empty_selection",
        ));
    }
    if p.cloud_model_id.is_some() {
        return Err(invalid(
            "cloud_model_id: Linux has no cloud providers".into(),
            "no_cloud_provider",
        ));
    }
    let store = daemon.models().store();
    let catalogue = store.catalogue();
    let current = daemon.config().config.speech;
    let provider = p.provider.clone().unwrap_or(current.provider.clone());
    if !catalogue.stt_providers().contains(&provider) {
        return Err(invalid(
            format!("unknown provider {provider}"),
            "unknown_provider",
        ));
    }
    let check = |id: &str, key: &str| -> Result<(), JsonRpcError> {
        let provider = if key == "meeting_model" {
            "whisper"
        } else {
            &provider
        };
        match catalogue.find(provider, id) {
            None => Err(invalid(
                format!("{key}: unknown model {provider}/{id}"),
                "unknown_model",
            )),
            Some(m) if !m.available => Err(invalid(
                format!("{key}: {provider}/{id} is not available yet"),
                "model_unavailable",
            )),
            Some(_) => Ok(()),
        }
    };
    let mut writes: Vec<(&str, String)> = Vec::new();
    if let Some(prov) = &p.provider {
        writes.push(("speech.provider", prov.clone()));
        if p.model.is_none() {
            // The current model stays when the new provider lists it;
            // otherwise `parakeet_model_id` for Parakeet, else the
            // provider's default, which must be available too.
            if check(&current.model, "model").is_err() {
                let remembered = (prov == "parakeet"
                    && check(&current.parakeet_model_id, "model").is_ok())
                .then(|| current.parakeet_model_id.clone());
                let d = remembered.unwrap_or_else(|| {
                    catalogue
                        .default_for(prov)
                        .map(|m| m.id.clone())
                        .unwrap_or_default()
                });
                check(&d, "model")?;
                writes.push(("speech.model", d));
            }
        }
    }
    if let Some(model) = &p.model {
        check(model, "model")?;
        writes.push(("speech.model", model.clone()));
    }
    if let Some(meeting) = &p.meeting_model {
        if !meeting.is_empty() {
            check(meeting, "meeting_model")?;
        }
        writes.push(("speech.meeting_model", meeting.clone()));
    }
    if let Some(pk) = &p.parakeet_model_id {
        if catalogue.find("parakeet", pk).is_none() {
            return Err(invalid(
                format!("parakeet_model_id: unknown model parakeet/{pk}"),
                "unknown_model",
            ));
        }
        writes.push(("speech.parakeet_model_id", pk.clone()));
    }
    super::config::edit_file(daemon, |mut text| {
        for (key, value) in &writes {
            text = edit::set(&text, key, &Value::String(value.clone()))
                .map_err(|e| invalid(format!("{key}: {}", e.message), "invalid_value"))?;
        }
        Ok(text)
    })?;
    tracing::info!(keys = ?writes.iter().map(|(k, _)| *k).collect::<Vec<_>>(), "speech selection written");
    selection_get(daemon)
}

/// `speech.models.status`.
pub fn models_status(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: ModelsStatusParams = if params_value.is_null() {
        ModelsStatusParams::default()
    } else {
        params(params_value)?
    };
    let (sp, sm) = selected(daemon);
    let store = daemon.models().store();
    let stt = store.catalogue().stt_providers();
    json(ModelsStatusResult {
        models: daemon
            .models()
            .status(p.provider.as_deref(), p.model.as_deref(), (&sp, &sm))
            .into_iter()
            .filter(|m| stt.contains(&m.provider))
            .collect(),
        models_dir: store.root().to_string_lossy().into_owned(),
        catalogue_version: store.catalogue().version,
    })
}

/// The speech methods know the speech providers and the diarization
/// model set (ADR 0035); a language model goes through `llm.models.*`.
fn speech_entry(
    daemon: &Daemon,
    provider: &str,
    model: &str,
) -> Result<dettivo_speech::catalogue::ModelEntry, JsonRpcError> {
    let store = daemon.models().store();
    if !store
        .catalogue()
        .download_providers()
        .iter()
        .any(|p| p == provider)
    {
        return Err(model_error(ModelError::Unknown(format!(
            "{provider}/{model}"
        ))));
    }
    daemon.models().entry(provider, model).map_err(model_error)
}

fn one(daemon: &Daemon, provider: &str, model: &str) -> Result<ModelStatus, JsonRpcError> {
    let entry = speech_entry(daemon, provider, model)?;
    let (sp, sm) = selected(daemon);
    Ok(daemon.models().status_of(&entry, (&sp, &sm)))
}

/// `speech.models.download`: starts or joins the download and answers the
/// model's status.
pub fn models_download(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: ModelParams = params(params_value)?;
    let entry = speech_entry(daemon, &p.provider, &p.model)?;
    daemon.models().download(&entry).map_err(model_error)?;
    json(one(daemon, &p.provider, &p.model)?)
}

/// `speech.models.cancel`.
pub fn models_cancel(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: ModelParams = params(params_value)?;
    let entry = speech_entry(daemon, &p.provider, &p.model)?;
    daemon.models().cancel(&entry);
    // Give this model's transfer a moment to stop so the answer is the
    // partial file; other downloads are not waited for.
    let _ = daemon
        .models()
        .wait_stopped(&entry, std::time::Duration::from_millis(500));
    json(one(daemon, &p.provider, &p.model)?)
}

/// `speech.models.delete`: the selected model needs `force`.
pub fn models_delete(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: ModelDeleteParams = params(params_value)?;
    let entry = speech_entry(daemon, &p.provider, &p.model)?;
    let (sp, sm) = selected(daemon);
    let deleted = daemon
        .models()
        .delete(&entry, (&sp, &sm), p.force)
        .map_err(model_error)?;
    tracing::info!(provider = %p.provider, model = %p.model, deleted, "model deleted");
    json(ModelDeleteResult {
        deleted,
        model: one(daemon, &p.provider, &p.model)?,
    })
}
