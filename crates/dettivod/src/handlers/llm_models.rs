//! `llm.models.*` and `llm.engine.status` (Linux additions, ADR 0026):
//! the language model catalogue over the same model service the speech
//! methods use, with the provider fixed to `llm` and `is_selected`
//! marking `[llm] model`, and the local engine's state. A sideloaded
//! fine-tune (`[llm] polish_experiment`, ADR 0032) is one more row of
//! `llm.models.status` with `source = "sideload"`; it is never a
//! catalogue entry, so `download`, `cancel` and `delete` do not know it.

use dettivo_engine_proto::Backend;
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::llm::{
    EngineStatusResult, ModelDeleteParams, ModelDeleteResult, ModelParams, ModelsStatusParams,
    ModelsStatusResult,
};
use dettivo_proto::methods::speech::{EngineBackend, ModelReadiness, ModelStatus};
use dettivo_speech::catalogue::ModelEntry;
use dettivo_speech::llm::sideload::Format;
use serde_json::Value;

use super::{json, params};

use crate::daemon::Daemon;
use crate::models::ModelError;

/// The catalogue provider every language model sits under.
pub const PROVIDER: &str = "llm";

fn model_error(e: ModelError) -> JsonRpcError {
    let message = e.to_string();
    match e {
        ModelError::Unknown(_) | ModelError::Unavailable(_) => {
            JsonRpcError::new(AppCode::InvalidParams, message, ErrorDetails::empty())
        }
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

/// The selection `(provider, model)` the rows are marked against.
fn selected(daemon: &Daemon) -> (String, String) {
    (PROVIDER.to_string(), daemon.config().config.llm.model)
}

/// The catalogue entry, or `NOT_FOUND` naming what the catalogue lists.
fn entry(daemon: &Daemon, model: &str) -> Result<ModelEntry, JsonRpcError> {
    daemon.models().entry(PROVIDER, model).map_err(|_| {
        let store = daemon.models().store();
        let ids: Vec<String> = store
            .catalogue()
            .models_of(PROVIDER)
            .iter()
            .map(|m| m.id.clone())
            .collect();
        JsonRpcError::new(
            AppCode::NotFound,
            format!(
                "unknown model {PROVIDER}/{model}; the catalogue lists {}",
                ids.join(", ")
            ),
            ErrorDetails::empty(),
        )
    })
}

fn one(daemon: &Daemon, entry: &ModelEntry) -> ModelStatus {
    let (sp, sm) = selected(daemon);
    daemon.models().status_of(entry, (&sp, &sm))
}

/// The row for the sideload `[llm] polish_experiment` names: the resolved
/// fine-tune with its readiness (the adapter's base has to be on disk
/// too), or the manifest name with `manifest_error` when it did not
/// resolve. `None` when the key is empty.
pub fn sideload_row(daemon: &Daemon) -> Option<ModelStatus> {
    let local = daemon.engines().local_llm();
    let llm = daemon.config().config.llm;
    let name = llm.polish_experiment()?.to_string();
    let base = |id: &str, path: Option<String>, readiness, error, size: u64| ModelStatus {
        provider: PROVIDER.to_string(),
        id: id.to_string(),
        display_name: String::new(),
        size_bytes: size,
        readiness,
        bytes_done: 0,
        bytes_total: size,
        path,
        license: "private".into(),
        is_default: false,
        is_selected: true,
        available: false,
        error,
        kind: "llm".to_string(),
        languages: Vec::new(),
        recommended_for: None,
        source: Some("sideload".into()),
        format: None,
        base_model: None,
        manifest_error: None,
    };
    Some(match local.experiment()? {
        Ok(s) => {
            let file_present = s.gguf.is_file();
            let base_present = local.model_path().is_file();
            let (readiness, error) = if !file_present {
                (
                    ModelReadiness::Missing,
                    Some(format!("{} is gone", s.gguf.display())),
                )
            } else if !base_present {
                (
                    ModelReadiness::Missing,
                    Some(format!(
                        "the base llm/{} is not downloaded; run `{}`",
                        s.base_model.as_deref().unwrap_or_default(),
                        crate::llm_engine::download_command(
                            s.base_model.as_deref().unwrap_or_default()
                        )
                    )),
                )
            } else {
                (ModelReadiness::Ready, None)
            };
            let mut row = base(
                &s.id,
                file_present.then(|| s.gguf.to_string_lossy().into_owned()),
                readiness,
                error,
                s.size_bytes,
            );
            row.display_name = s.display_name.clone();
            row.bytes_done = if file_present { s.size_bytes } else { 0 };
            row.format = Some(s.format.as_str().into());
            row.base_model = (s.format == Format::GgufLora)
                .then(|| s.base_model.clone())
                .flatten();
            row
        }
        Err(e) => {
            let mut row = base(&name, None, ModelReadiness::Missing, None, 0);
            row.display_name = format!("polish experiment {name}");
            row.manifest_error = Some(e.message.clone());
            row
        }
    })
}

/// `llm.models.status`.
pub fn models_status(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: ModelsStatusParams = params(params_value)?;
    let sideload = sideload_row(daemon);
    let only_sideload = matches!((&p.model, &sideload), (Some(m), Some(s)) if m == &s.id);
    if let Some(model) = p.model.as_ref().filter(|_| !only_sideload) {
        entry(daemon, model)?;
    }
    let (sp, sm) = selected(daemon);
    let store = daemon.models().store();
    let llm = daemon.config().config.llm;
    let mut models = if only_sideload {
        Vec::new()
    } else {
        daemon
            .models()
            .status(Some(PROVIDER), p.model.as_deref(), (&sp, &sm))
    };
    // A sideload in force is what Enhanced runs on, so it carries the
    // selection mark and the catalogue rows do not.
    if let Some(row) = sideload.filter(|_| p.model.is_none() || only_sideload) {
        for m in &mut models {
            m.is_selected = false;
        }
        models.push(row);
    }
    for m in &mut models {
        if m.source.is_none() {
            m.source = Some("catalogue".into());
        }
    }
    json(ModelsStatusResult {
        models,
        models_dir: store.root().to_string_lossy().into_owned(),
        catalogue_version: store.catalogue().version,
        selected: llm.model.clone(),
        analysis_model: llm.analysis_model().to_string(),
        polish_experiment: llm.polish_experiment.trim().to_string(),
    })
}

/// `llm.models.download`: starts or joins the download and answers the
/// model's status.
pub fn models_download(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: ModelParams = params(params_value)?;
    let entry = entry(daemon, &p.model)?;
    daemon.models().download(&entry).map_err(model_error)?;
    json(one(daemon, &entry))
}

/// `llm.models.cancel`.
pub fn models_cancel(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: ModelParams = params(params_value)?;
    let entry = entry(daemon, &p.model)?;
    daemon.models().cancel(&entry);
    let _ = daemon
        .models()
        .wait_stopped(&entry, std::time::Duration::from_millis(500));
    json(one(daemon, &entry))
}

/// `llm.models.delete`: the selected model needs `force`.
pub fn models_delete(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: ModelDeleteParams = params(params_value)?;
    let entry = entry(daemon, &p.model)?;
    let (sp, sm) = selected(daemon);
    let deleted = daemon
        .models()
        .delete(&entry, (&sp, &sm), p.force)
        .map_err(model_error)?;
    tracing::info!(model = %p.model, deleted, "llm model deleted");
    json(ModelDeleteResult {
        deleted,
        model: one(daemon, &entry),
    })
}

/// `llm.engine.status`.
pub fn engine_status(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let local = daemon.engines().local_llm();
    let status = local.status();
    json(EngineStatusResult {
        binary: status.engine.binary,
        path: status.engine.path.map(|p| p.to_string_lossy().into_owned()),
        running: status.engine.running,
        loaded: status.loaded,
        busy: status.busy,
        model: status.engine.model,
        backend: status.engine.backend.map(|b| match b {
            Backend::Vulkan => EngineBackend::Vulkan,
            Backend::Cuda => EngineBackend::Cuda,
            Backend::Cpu => EngineBackend::Cpu,
        }),
        reason: status.engine.reason,
        lora: status.engine.lora,
        memory_bytes: status.memory_bytes,
        crashes: status.engine.crashes,
        degraded: status.engine.degraded,
        idle_seconds: local.idle_seconds(),
    })
}
