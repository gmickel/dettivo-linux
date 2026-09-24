//! The engine supervisor as the daemon holds it: settings from
//! `[engines]`, the speech engine and model from `[speech]` (the provider
//! picks the binary, the catalogue names the file), a preloader that
//! warms the engine at start and after a model change, the local language
//! model bound to `[llm] model` (ADR 0026), an idle reaper and the status
//! rows `speech.engines` and `dettivo doctor` show.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dettivo_core::config::Loaded;
use dettivo_core::config::schema::EngineBackend;
use dettivo_core::paths::Paths;
use dettivo_engine_proto::BackendPreference;
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_speech::engines;
use dettivo_speech::preload::Preloader;
use dettivo_speech::supervisor::{EngineStatus, Settings, Supervisor};
use dettivo_speech::{PreloadSource, SttEngine};

use crate::llm_engine::LocalLlm;
use crate::verify::Verifier;

/// The engine binaries the daemon knows about, in report order.
pub const BINARIES: &[&str] = &[
    "dettivo-engine-whisper",
    "dettivo-engine-parakeet",
    "dettivo-engine-llm",
    "dettivo-engine-diarize",
];

/// How often idle engines are checked.
pub const REAP_INTERVAL: Duration = Duration::from_secs(30);

/// The analysis engine: `local` itself while `[llm] analysis_model` is
/// empty or names the same model, else one bound to the other model.
fn analysis_llm(
    supervisor: &Arc<Supervisor>,
    paths: &Paths,
    loaded: &Loaded,
    local: &Arc<LocalLlm>,
    verifier: &Verifier,
) -> Arc<LocalLlm> {
    let model = loaded.config.llm.analysis_model();
    if model == local.model_id() {
        return local.clone();
    }
    Arc::new(LocalLlm::for_model(
        supervisor.clone(),
        paths,
        loaded,
        model,
        verifier.clone(),
    ))
}

/// The supervisor plus the currently selected speech model.
pub struct Engines {
    supervisor: Arc<Supervisor>,
    selected: Mutex<Selected>,
    local_llm: Mutex<Arc<LocalLlm>>,
    analysis_llm: Mutex<Arc<LocalLlm>>,
    /// The gate a catalogue model passes before an engine opens it.
    verifier: Verifier,
}

struct Selected {
    provider: String,
    model: PathBuf,
    /// `[engines.<provider>] backend`: the recognizer carries it, so a
    /// change rebuilds the recognizer like a model change does.
    backend: BackendPreference,
    engine: Arc<dyn SttEngine>,
    preloader: Arc<Preloader>,
}

impl Engines {
    /// Builds the supervisor from the configuration in force; `force_cpu`
    /// (`DETTIVO_FORCE_CPU=1`) makes every engine skip Vulkan and the
    /// tier report `cpu`.
    pub fn new(paths: &Paths, loaded: &Loaded, force_cpu: bool, verifier: Verifier) -> Self {
        let supervisor = Supervisor::new(settings(loaded, force_cpu));
        let provider = loaded.config.speech.provider.clone();
        let model = selected_model_path(paths, loaded);
        let backend = backend_of(loaded, &provider);
        let (engine, preloader) = preloader(&supervisor, &provider, &model, loaded);
        let local_llm = Arc::new(LocalLlm::new(
            supervisor.clone(),
            paths,
            loaded,
            verifier.clone(),
        ));
        let analysis_llm = analysis_llm(&supervisor, paths, loaded, &local_llm, &verifier);
        Self {
            supervisor,
            selected: Mutex::new(Selected {
                provider,
                model,
                backend,
                engine,
                preloader,
            }),
            local_llm: Mutex::new(local_llm),
            analysis_llm: Mutex::new(analysis_llm),
            verifier,
        }
    }

    /// The local language model bound to `[llm] analysis_model`: the
    /// Enhanced engine itself while the two names agree, so one process
    /// serves both.
    pub fn analysis_llm(&self) -> Arc<LocalLlm> {
        self.analysis_llm
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// The local language model bound to the configuration in force.
    pub fn local_llm(&self) -> Arc<LocalLlm> {
        self.local_llm
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Applies a reloaded configuration: new idle timeouts and directory
    /// take effect at once, and a changed provider or model is preloaded
    /// (FR-E6 `model_selection`) when its file exists.
    pub fn apply(&self, paths: &Paths, loaded: &Loaded) {
        self.supervisor
            .update(settings(loaded, self.supervisor.force_cpu()));
        {
            let mut local = self.local_llm.lock().unwrap_or_else(|p| p.into_inner());
            let next = LocalLlm::new(
                self.supervisor.clone(),
                paths,
                loaded,
                self.verifier.clone(),
            );
            if next.model_path() != local.model_path() {
                tracing::info!(model = %next.model_id(), "llm model selection changed");
            }
            *local = Arc::new(next);
            *self.analysis_llm.lock().unwrap_or_else(|p| p.into_inner()) =
                analysis_llm(&self.supervisor, paths, loaded, &local, &self.verifier);
        }
        let provider = loaded.config.speech.provider.clone();
        let model = selected_model_path(paths, loaded);
        let backend = backend_of(loaded, &provider);
        let changed = {
            let mut s = self.selected.lock().unwrap_or_else(|p| p.into_inner());
            if s.model == model && s.provider == provider && s.backend == backend {
                None
            } else {
                s.model = model.clone();
                s.provider = provider.clone();
                s.backend = backend;
                let (engine, pre) = preloader(&self.supervisor, &provider, &model, loaded);
                s.engine = engine;
                s.preloader = pre;
                Some(s.preloader.clone())
            }
        };
        if let Some(preloader) = changed {
            self.preload(preloader, model, PreloadSource::ModelSelection);
        }
    }

    /// Warms the selected engine at daemon start (FR-E6 `startup`), off
    /// the async runtime; a missing model is logged, never an error.
    pub fn preload_startup(&self) {
        let (preloader, model) = {
            let s = self.selected.lock().unwrap_or_else(|p| p.into_inner());
            (s.preloader.clone(), s.model.clone())
        };
        self.preload(preloader, model, PreloadSource::Startup);
    }

    fn preload(&self, preloader: Arc<Preloader>, model: PathBuf, source: PreloadSource) {
        if !model.is_file() {
            // The model id only: the path is under the user's data directory,
            // which the log never names.
            let id = model
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            tracing::info!(
                model = %id,
                source = ?source,
                "speech model not downloaded; preload skipped"
            );
            return;
        }
        let verifier = self.verifier.clone();
        std::thread::Builder::new()
            .name("engine-preload".into())
            .spawn(move || {
                // The model verifies (or hashes now) before the engine
                // opens it; a quarantined one is never loaded.
                if let Err(why) = verifier.ensure(&model) {
                    tracing::warn!(source = ?source, error = %why, "preload skipped");
                    return;
                }
                preloader.request(source);
            })
            .map(|_| ())
            .unwrap_or_else(|e| tracing::warn!(error = %e, "preload thread not started"));
    }

    /// The selected model verified (hashed now when it has not been) so a
    /// session may load it; a quarantined or missing one is refused with
    /// the download command.
    pub fn ensure_selected_verified(&self) -> Result<(), JsonRpcError> {
        self.verifier
            .ensure(&self.model_path())
            .map_err(|why| JsonRpcError::new(AppCode::NotFound, why, ErrorDetails::empty()))
    }

    /// The engine for the selected model (frozen by a session at start).
    pub fn engine(&self) -> Arc<dyn SttEngine> {
        self.selected
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .engine
            .clone()
    }

    /// An engine for `provider/model` (re-runs and imports): the selected
    /// engine when it is that model, else a fresh recognizer over the same
    /// supervisor. A provider without an engine or a model not on disk is
    /// refused with the download command named.
    pub fn engine_for(
        &self,
        paths: &Paths,
        loaded: &Loaded,
        provider: &str,
        model: &str,
    ) -> Result<(Arc<dyn SttEngine>, PathBuf), JsonRpcError> {
        let Some(path) = model_path(paths, loaded, provider, model) else {
            return Err(JsonRpcError::new(
                AppCode::InvalidParams,
                format!("unknown model {provider}/{model}"),
                ErrorDetails::empty(),
            ));
        };
        if !path.is_file() {
            return Err(JsonRpcError::new(
                AppCode::NotFound,
                format!(
                    "model {provider}/{model} is not downloaded; run `{}`",
                    download_command(provider, model)
                ),
                ErrorDetails::empty(),
            ));
        }
        self.verifier
            .ensure(&path)
            .map_err(|why| JsonRpcError::new(AppCode::NotFound, why, ErrorDetails::empty()))?;
        let s = self.selected.lock().unwrap_or_else(|p| p.into_inner());
        if s.model == path {
            return Ok((s.engine.clone(), path));
        }
        let engine = engines::engine_for(
            provider,
            self.supervisor.clone(),
            path.to_string_lossy().into_owned(),
            backend_of(loaded, provider),
        )
        .ok_or_else(|| {
            JsonRpcError::new(
                AppCode::InvalidParams,
                format!("provider {provider} has no engine"),
                ErrorDetails::empty(),
            )
        })?;
        Ok((engine, path))
    }

    /// The selected model file.
    pub fn model_path(&self) -> PathBuf {
        self.selected
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .model
            .clone()
    }

    /// Warms the selected engine for a session that starts now (FR-E6
    /// `session_start`), coalesced with any preload in flight.
    pub fn preload_session_start(&self) {
        let (preloader, model) = {
            let s = self.selected.lock().unwrap_or_else(|p| p.into_inner());
            (s.preloader.clone(), s.model.clone())
        };
        self.preload(preloader, model, PreloadSource::SessionStart);
    }

    /// The supervisor itself, for an engine built outside this service
    /// (the diarization pass, ADR 0035).
    pub fn supervisor(&self) -> Arc<Supervisor> {
        self.supervisor.clone()
    }

    /// Installs the engine transition hook.
    pub fn set_hook(&self, hook: dettivo_speech::supervisor::StateHook) {
        self.supervisor.set_hook(hook);
    }

    /// Unloads engines idle past their timeout; returns how many stopped.
    pub fn reap_idle(&self) -> usize {
        self.supervisor.reap_idle()
    }

    /// One row per known engine binary.
    pub fn status(&self) -> Vec<EngineStatus> {
        self.supervisor.status(BINARIES)
    }

    /// The hardware tier (S-16) the benchmarks and the doctor report.
    pub fn tier(&self) -> dettivo_speech::tier::TierReport {
        self.supervisor.tier(BINARIES)
    }

    /// Terminates every engine process.
    pub fn shutdown(&self) {
        self.supervisor.shutdown();
    }
}

fn settings(loaded: &Loaded, force_cpu: bool) -> Settings {
    let engines = &loaded.config.engines;
    let directory = if engines.directory.is_empty() {
        None
    } else {
        Some(PathBuf::from(&engines.directory))
    };
    Settings {
        directory,
        stt_idle: Duration::from_secs(engines.stt_idle_seconds),
        llm_idle: Duration::from_secs(engines.llm_idle_seconds),
        force_cpu,
        ..Settings::default()
    }
}

/// The CLI command that fetches `provider/model` (the CLI's default
/// provider is whisper, so only another provider is spelled out).
pub fn download_command(provider: &str, model: &str) -> String {
    if provider == "whisper" {
        format!("dettivo speech download --model {model}")
    } else {
        format!("dettivo speech download --provider {provider} --model {model}")
    }
}

/// `[engines.<provider>] backend` as the protocol names it.
pub fn backend_of(loaded: &Loaded, provider: &str) -> BackendPreference {
    let section = match provider {
        "parakeet" => loaded.config.engines.parakeet.backend,
        "llm" => loaded.config.engines.llm.backend,
        _ => loaded.config.engines.whisper.backend,
    };
    match section {
        EngineBackend::Auto => BackendPreference::Auto,
        EngineBackend::Vulkan => BackendPreference::Vulkan,
        EngineBackend::Cpu => BackendPreference::Cpu,
    }
}

/// `<models>/<provider>/<model>/<file>` with the file name the catalogue
/// records for the model (`ggml-<model>.bin` for a Whisper model the
/// catalogue does not list, so a local file still resolves); the
/// directory itself for a model set the engine loads whole.
pub fn model_path(paths: &Paths, loaded: &Loaded, provider: &str, model: &str) -> Option<PathBuf> {
    let dir = loaded.models_dir(paths).join(provider).join(model);
    let catalogue = crate::models::catalogue_for(loaded);
    match catalogue.find(provider, model) {
        Some(entry) if entry.loads_directory() => Some(dir),
        Some(entry) => Some(dir.join(&entry.file_name)),
        None if provider == "whisper" => Some(dir.join(format!("ggml-{model}.bin"))),
        None => None,
    }
}

/// The providers with an engine; anything else preloads through Whisper.
const KNOWN_PROVIDERS: &[&str] = &["whisper", "parakeet"];

fn selected_model_path(paths: &Paths, loaded: &Loaded) -> PathBuf {
    let speech = &loaded.config.speech;
    // An unknown provider preloads through the Whisper engine, so its
    // path is resolved as Whisper's too; the mismatch is logged once here
    // instead of surfacing as a puzzling preload failure.
    let provider = if KNOWN_PROVIDERS.contains(&speech.provider.as_str()) {
        speech.provider.as_str()
    } else {
        tracing::warn!(
            provider = %speech.provider,
            "speech.provider is not a known engine; resolving the model as whisper"
        );
        "whisper"
    };
    model_path(paths, loaded, provider, &speech.model).unwrap_or_else(|| {
        loaded
            .models_dir(paths)
            .join(provider)
            .join(&speech.model)
            .join(format!("{}.gguf", speech.model))
    })
}

fn preloader(
    supervisor: &Arc<Supervisor>,
    provider: &str,
    model: &std::path::Path,
    loaded: &Loaded,
) -> (Arc<dyn SttEngine>, Arc<Preloader>) {
    let engine = engines::engine_for(
        provider,
        supervisor.clone(),
        model.to_string_lossy().into_owned(),
        backend_of(loaded, provider),
    )
    .unwrap_or_else(|| {
        tracing::warn!(
            provider,
            "no engine for the configured provider; using whisper"
        );
        engines::engine_for(
            "whisper",
            supervisor.clone(),
            model.to_string_lossy().into_owned(),
            backend_of(loaded, "whisper"),
        )
        .expect("whisper always has an engine")
    });
    (engine.clone(), Preloader::new(engine))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded(dir: &std::path::Path, config: &str) -> (Paths, Loaded) {
        let home = dir.to_string_lossy().into_owned();
        let paths = Paths::from_env(|k| match k {
            "HOME" => Some(home.clone().into()),
            _ => None,
        });
        std::fs::create_dir_all(paths.config_file.parent().unwrap()).unwrap();
        std::fs::write(&paths.config_file, config).unwrap();
        let loaded = Loaded::load(&paths.config_file, |_| None);
        (paths, loaded)
    }

    /// daemon/F13: a backend-only change (same provider, same model)
    /// rebuilds the selected recognizer, so the next session loads with
    /// the new backend; an unchanged configuration keeps the warm one.
    #[test]
    fn a_backend_only_change_rebuilds_the_selected_engine() {
        let dir = tempfile::tempdir().unwrap();
        let (paths, first) = loaded(dir.path(), "[engines.whisper]\nbackend = \"auto\"\n");
        let verifier = crate::models::Models::new(&paths, &first).verifier();
        let engines = Engines::new(&paths, &first, true, verifier);
        let warm = engines.engine();
        engines.apply(&paths, &first);
        assert!(
            Arc::ptr_eq(&warm, &engines.engine()),
            "an unchanged selection keeps its engine"
        );
        let (_, second) = loaded(dir.path(), "[engines.whisper]\nbackend = \"cpu\"\n");
        engines.apply(&paths, &second);
        assert!(
            !Arc::ptr_eq(&warm, &engines.engine()),
            "the changed backend rebuilds the engine"
        );
    }
}
