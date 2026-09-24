//! The engine supervisor (FR-D7): spawn on first need, keep warm, unload
//! and terminate after the idle timeout, restart with backoff after a
//! crash, degrade after three crashes in a row, clear on a successful
//! load. One instance per engine binary, requests serialised per engine.
//! The `SttEngine` implementations over it live in `engines`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dettivo_engine_proto::{BackendPreference, LoadParams, LoadedResult};
use serde_json::{Value, json};

use crate::EngineError;
pub use crate::engines::{
    DIARIZE_BINARY, LLM_BINARY, PARAKEET_BINARY, ParakeetEngine, WHISPER_BINARY, WhisperEngine,
};
use crate::process::{EngineProcess, find_binary};
pub use crate::supervisor_types::{EngineStatus, EngineTransition, LlmLoad, Settings, StateHook};

#[path = "supervisor_status.rs"]
mod status;

struct Slot {
    process: Option<EngineProcess>,
    loaded: Option<LoadedResult>,
    /// The complete `load` the engine holds; any difference reloads.
    params: Option<LoadParams>,
    /// Automatic Whisper load that must stay on CPU until idle unload or a new selection.
    cpu_fallback: Option<LoadParams>,
    crashes: u32,
    next_allowed: Instant,
}

/// The supervisor.
pub struct Supervisor {
    settings: Mutex<Settings>,
    slots: Mutex<HashMap<String, Arc<Mutex<Slot>>>>,
    snapshots: Mutex<HashMap<String, EngineStatus>>,
    hook: Mutex<Option<StateHook>>,
}

impl Supervisor {
    /// A supervisor with `settings`.
    pub fn new(settings: Settings) -> Arc<Self> {
        Arc::new(Self {
            settings: Mutex::new(settings),
            slots: Mutex::new(HashMap::new()),
            snapshots: Mutex::new(HashMap::new()),
            hook: Mutex::new(None),
        })
    }

    /// Installs the transition hook (one; the daemon's event bus).
    pub fn set_hook(&self, hook: StateHook) {
        *self.hook.lock().unwrap_or_else(|p| p.into_inner()) = Some(hook);
    }

    fn fire(&self, t: EngineTransition) {
        let hook = self.hook.lock().unwrap_or_else(|p| p.into_inner()).clone();
        if let Some(h) = hook {
            h(t);
        }
    }

    fn transition(&self, s: &mut Slot, t: EngineTransition) {
        self.remember(&t.binary, s);
        self.fire(t);
    }

    /// Applies new settings (config reload).
    pub fn update(&self, settings: Settings) {
        *self.settings.lock().unwrap_or_else(|p| p.into_inner()) = settings;
    }

    fn settings(&self) -> Settings {
        self.settings
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn slot(&self, binary: &str) -> Arc<Mutex<Slot>> {
        self.slots
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entry(binary.to_string())
            .or_insert_with(|| {
                Arc::new(Mutex::new(Slot {
                    process: None,
                    loaded: None,
                    params: None,
                    cpu_fallback: None,
                    crashes: 0,
                    next_allowed: Instant::now(),
                }))
            })
            .clone()
    }

    /// Ensures `binary` runs with `model` loaded, spawning or restarting as
    /// needed, then runs `f` against it. A crash inside `f` is recorded.
    pub fn with_engine<T>(
        &self,
        binary: &str,
        model: &str,
        vad_model: Option<&str>,
        f: impl FnOnce(&mut EngineProcess, &LoadedResult) -> Result<T, EngineError>,
    ) -> Result<T, EngineError> {
        self.with_engine_on(
            binary,
            model,
            vad_model,
            BackendPreference::Auto,
            LlmLoad::default(),
            f,
        )
    }

    /// `with_engine` with the backend preference the `load` carries
    /// (`[engines.<name>] backend`) and, for the language model engine,
    /// the context length and the LoRA adapter.
    pub fn with_engine_on<T>(
        &self,
        binary: &str,
        model: &str,
        vad_model: Option<&str>,
        backend_preference: BackendPreference,
        llm: LlmLoad,
        f: impl FnOnce(&mut EngineProcess, &LoadedResult) -> Result<T, EngineError>,
    ) -> Result<T, EngineError> {
        self.with_engine_load(
            binary,
            LoadParams {
                model: model.to_string(),
                backend_preference,
                vad_model: vad_model.map(str::to_string),
                context_length: llm.context_length,
                lora: llm.lora,
                threads: None,
            },
            f,
        )
    }

    /// The general form: ensures `binary` runs with `params.model` loaded
    /// (sending `params` as the `load`), spawning or restarting as needed,
    /// then runs `f` against it. A crash inside `f` is recorded.
    pub fn with_engine_load<T>(
        &self,
        binary: &str,
        params: LoadParams,
        f: impl FnOnce(&mut EngineProcess, &LoadedResult) -> Result<T, EngineError>,
    ) -> Result<T, EngineError> {
        let settings = self.settings();
        let slot = self.slot(binary);
        let mut s = slot.lock().unwrap_or_else(|p| p.into_inner());
        let mut params = params;
        if s.cpu_fallback.as_ref() == Some(&params) {
            params.backend_preference = BackendPreference::Cpu;
        } else {
            s.cpu_fallback = None;
        }
        let result = self.with_locked(binary, params, &settings, &mut s, f);
        self.remember(binary, &mut s);
        result
    }

    fn with_locked<T>(
        &self,
        binary: &str,
        params: LoadParams,
        settings: &Settings,
        s: &mut Slot,
        f: impl FnOnce(&mut EngineProcess, &LoadedResult) -> Result<T, EngineError>,
    ) -> Result<T, EngineError> {
        if s.crashes >= 3 && s.process.is_none() && Instant::now() < s.next_allowed {
            return Err(EngineError::Degraded);
        }
        if let Some(p) = s.process.as_mut() {
            if !p.alive() {
                tracing::warn!(engine = binary, "engine process is gone; restarting");
                s.process = None;
                s.loaded = None;
                s.params = None;
            }
        }
        if s.process.is_none() {
            if Instant::now() < s.next_allowed {
                std::thread::sleep(s.next_allowed.saturating_duration_since(Instant::now()));
            }
            let path = find_binary(binary, settings.directory.as_deref())?;
            tracing::info!(engine = binary, path = %path.display(), "spawning engine");
            s.process = Some(EngineProcess::spawn(&path, settings.force_cpu)?);
            s.loaded = None;
            s.params = None;
            self.transition(
                s,
                EngineTransition {
                    binary: binary.to_string(),
                    state: "spawned",
                    model: None,
                    backend: None,
                    reason: None,
                },
            );
        }
        let needs_load = s.loaded.is_none() || s.params.as_ref() != Some(&params);
        if needs_load {
            let process = s.process.as_mut().expect("spawned above");
            let result = process.call(
                "load",
                serde_json::to_value(&params).unwrap_or(Value::Null),
                &[],
                settings.load_timeout,
                |_| {},
            );
            match result {
                Err(e @ (EngineError::Crashed(_) | EngineError::Transport(_)))
                    if binary == WHISPER_BINARY
                        && params.backend_preference == BackendPreference::Auto
                        && !settings.force_cpu =>
                {
                    // ggml may abort instead of returning a load error. The
                    // supervisor survives, so retry in a fresh CPU process.
                    self.discard_uncertain(s, binary, &e);
                    s.cpu_fallback = Some(params.clone());
                    let mut cpu = params;
                    cpu.backend_preference = BackendPreference::Cpu;
                    tracing::warn!(engine = binary, error = %e, "automatic load failed; retrying on CPU");
                    return self.with_locked(binary, cpu, settings, s, f);
                }
                Ok(v) => {
                    let mut loaded: LoadedResult = serde_json::from_value(v)
                        .map_err(|e| EngineError::Transport(format!("load response: {e}")))?;
                    if s.cpu_fallback.is_some()
                        && loaded.backend == dettivo_engine_proto::Backend::Cpu
                    {
                        loaded.reason = format!(
                            "CPU fallback after automatic engine load failed; {}",
                            loaded.reason
                        );
                    }
                    tracing::info!(engine = binary, backend = ?loaded.backend, "engine loaded");
                    s.crashes = 0;
                    let transition = EngineTransition {
                        binary: binary.to_string(),
                        state: "loaded",
                        model: Some(loaded.model.clone()),
                        backend: Some(loaded.backend),
                        reason: Some(loaded.reason.clone()),
                    };
                    s.loaded = Some(loaded);
                    s.params = Some(params);
                    self.transition(s, transition);
                }
                Err(EngineError::Crashed(tail)) => {
                    let transition = record_crash(s, binary);
                    self.transition(s, transition);
                    return Err(EngineError::Crashed(tail));
                }
                Err(e) => {
                    // A load that did not answer may still finish later; a
                    // request for the previous model would then run against
                    // this one. The process goes, and the next request
                    // loads afresh.
                    self.discard_uncertain(s, binary, &e);
                    if matches!(e, EngineError::Transport(_)) {
                        let transition = record_crash(s, binary);
                        self.transition(s, transition);
                    }
                    return Err(e);
                }
            }
        }
        let loaded = s.loaded.clone().expect("loaded above");
        let process = s.process.as_mut().expect("spawned above");
        match f(process, &loaded) {
            Err(EngineError::Crashed(tail)) => {
                let transition = record_crash(s, binary);
                self.transition(s, transition);
                Err(EngineError::Crashed(tail))
            }
            Err(e @ EngineError::Transport(_)) => {
                self.discard_uncertain(s, binary, &e);
                Err(e)
            }
            other => other,
        }
    }

    /// Terminates an engine whose state is no longer known (a timed-out
    /// or broken exchange) and forgets what it had loaded, so nothing
    /// reuses it; the engine reports itself `unloaded` with the reason.
    fn discard_uncertain(&self, s: &mut Slot, binary: &str, error: &EngineError) {
        tracing::warn!(engine = binary, error = %error, "engine state uncertain; terminating it");
        if let Some(mut p) = s.process.take() {
            p.terminate();
        }
        s.loaded = None;
        s.params = None;
        self.transition(
            s,
            EngineTransition {
                binary: binary.to_string(),
                state: "unloaded",
                model: None,
                backend: None,
                reason: Some(format!("terminated after {error}")),
            },
        );
    }

    /// Runs `f` against `binary`'s process when one is alive and not
    /// serving a request right now; `None` when it is not running, `Busy`
    /// when a request holds it.
    pub fn with_running<T>(
        &self,
        binary: &str,
        f: impl FnOnce(&mut EngineProcess) -> T,
    ) -> Option<Result<T, EngineError>> {
        let slot = self
            .slots
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(binary)
            .cloned()?;
        let Ok(mut s) = slot.try_lock() else {
            return Some(Err(EngineError::Busy));
        };
        let process = s.process.as_mut()?;
        if !process.alive() {
            return None;
        }
        Some(Ok(f(process)))
    }

    /// Unloads and terminates engines idle longer than their timeout
    /// (`llm_idle` for the language model engine, `stt_idle` for the
    /// rest); call it periodically. Returns how many were stopped. An
    /// engine that does not answer `unload` is terminated all the same,
    /// and the log says so.
    pub fn reap_idle(&self) -> usize {
        let settings = self.settings();
        let slots: Vec<(String, Arc<Mutex<Slot>>)> = self
            .slots
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut stopped = 0;
        for (binary, slot) in slots {
            let Ok(mut s) = slot.try_lock() else {
                continue;
            };
            let idle = s
                .process
                .as_ref()
                .map(|p| p.last_used.elapsed())
                .unwrap_or(Duration::ZERO);
            let limit = if binary == LLM_BINARY {
                settings.llm_idle
            } else {
                settings.stt_idle
            };
            if s.process.is_some() && idle >= limit {
                tracing::info!(
                    engine = binary,
                    idle_seconds = idle.as_secs(),
                    "unloading idle engine"
                );
                if let Some(mut p) = s.process.take() {
                    if let Err(e) = p.call("unload", json!({}), &[], Duration::from_secs(5), |_| {})
                    {
                        tracing::warn!(engine = binary, error = %e, "engine ignored unload; terminating it");
                    }
                    p.terminate();
                }
                s.loaded = None;
                s.params = None;
                s.cpu_fallback = None;
                stopped += 1;
                self.transition(
                    &mut s,
                    EngineTransition {
                        binary: binary.clone(),
                        state: "unloaded",
                        model: None,
                        backend: None,
                        reason: Some(format!("idle for {} s", idle.as_secs())),
                    },
                );
            }
        }
        stopped
    }

    /// Stops every engine (daemon shutdown).
    pub fn shutdown(&self) {
        for (_, slot) in self.slots.lock().unwrap_or_else(|p| p.into_inner()).drain() {
            let mut s = slot.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(mut p) = s.process.take() {
                p.terminate();
            }
        }
        self.snapshots
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
    }

    /// Whether the engines are spawned with `DETTIVO_FORCE_CPU=1`.
    pub fn force_cpu(&self) -> bool {
        self.settings().force_cpu
    }

    /// The hardware tier (S-16) from the override and the engines that
    /// have loaded.
    pub fn tier(&self, binaries: &[&str]) -> crate::tier::TierReport {
        let loaded: Vec<crate::tier::LoadedEngine> = self
            .status(binaries)
            .into_iter()
            .filter(|s| s.running)
            .filter_map(|s| {
                Some(crate::tier::LoadedEngine {
                    binary: s.binary,
                    backend: s.backend?,
                    reason: s.reason.unwrap_or_default(),
                })
            })
            .collect();
        crate::tier::detect(self.force_cpu(), &loaded)
    }
}

fn record_crash(s: &mut Slot, binary: &str) -> EngineTransition {
    s.crashes += 1;
    s.process = None;
    s.loaded = None;
    s.params = None;
    let backoff = Duration::from_secs(1 << (s.crashes.min(4) - 1));
    s.next_allowed = Instant::now() + backoff;
    if s.crashes >= 3 {
        tracing::error!(
            engine = binary,
            crashes = s.crashes,
            "engine degraded after repeated crashes"
        );
        EngineTransition {
            binary: binary.to_string(),
            state: "degraded",
            model: None,
            backend: None,
            reason: Some(format!("{} crashes in a row", s.crashes)),
        }
    } else {
        tracing::warn!(
            engine = binary,
            crashes = s.crashes,
            backoff_seconds = backoff.as_secs(),
            "engine crashed"
        );
        EngineTransition {
            binary: binary.to_string(),
            state: "crashed",
            model: None,
            backend: None,
            reason: Some(format!("restart in {} s", backoff.as_secs())),
        }
    }
}
