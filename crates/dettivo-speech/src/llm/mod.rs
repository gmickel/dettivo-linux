//! The language model engine over the supervisor (ADR 0026): one
//! `generate` per rewrite with the prompt profile, bounded by the
//! caller's timeout, and a `cancel` sent when that timeout runs out so
//! the process is free for the next request instead of finishing an
//! answer nobody waits for. Requests are serialised by the supervisor's
//! slot, so two rewrites at once queue, the second inside its own
//! timeout. `status` reads the process without loading anything.
//! `sideload` resolves a fine-tune manifest to the file the engine loads.

pub mod sideload;

use std::sync::Arc;
use std::time::Duration;

use dettivo_engine_proto::{
    Backend, BackendPreference, GenerateParams, GenerateResult, PromptProfile, StatusResult,
};
use serde_json::{Value, json};

use crate::EngineError;
use crate::engines::LLM_BINARY;
use crate::supervisor::{EngineStatus, LlmLoad, Supervisor};

/// One rewrite request.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateRequest {
    /// The system prompt (the Polish profile); `None` for the raw profile.
    pub system: Option<String>,
    /// The user turn, or the whole prompt under the raw profile.
    pub user: String,
    /// The most tokens the answer may have.
    pub max_tokens: u32,
    /// Sampling temperature; 0 is greedy.
    pub temperature: f64,
    /// Strings that end the answer.
    pub stop: Vec<String>,
    /// Feed `user` as is, without the chat template.
    pub raw: bool,
}

/// What `llm.engine.status` reports: the supervisor's row plus what the
/// running process says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmStatus {
    /// The supervisor's row (found, running, loaded model, crashes).
    pub engine: EngineStatus,
    /// A model is loaded in the process.
    pub loaded: bool,
    /// The process is serving a request.
    pub busy: bool,
    /// Bytes in use on the backend device, when the engine reports them.
    pub memory_bytes: Option<u64>,
}

/// How long a `cancel` after a timed-out generation may take to be
/// acknowledged before the process is given up on.
const CANCEL_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a `status` probe of the running process may take.
const STATUS_TIMEOUT: Duration = Duration::from_secs(2);

/// The language model engine for one model file, with an optional GGUF
/// LoRA adapter applied over it at load (a sideloaded fine-tune, ADR
/// 0032).
pub struct LlmEngine {
    supervisor: Arc<Supervisor>,
    model: String,
    backend: BackendPreference,
    load: LlmLoad,
}

impl LlmEngine {
    /// An engine over `supervisor` for the model file `model`.
    pub fn new(
        supervisor: Arc<Supervisor>,
        model: String,
        backend: BackendPreference,
        context_length: Option<u32>,
    ) -> Self {
        Self {
            supervisor,
            model,
            backend,
            load: LlmLoad {
                context_length,
                lora: None,
            },
        }
    }

    /// The same engine with a GGUF LoRA adapter applied over the model.
    pub fn with_lora(mut self, lora: Option<String>) -> Self {
        self.load.lora = lora;
        self
    }

    /// The model file this engine loads.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// The adapter applied over it, when one is.
    pub fn lora(&self) -> Option<&str> {
        self.load.lora.as_deref()
    }

    /// Loads the model so the first rewrite is fast; never downloads.
    pub fn preload(&self) -> Result<(), EngineError> {
        self.supervisor.with_engine_on(
            LLM_BINARY,
            &self.model,
            None,
            self.backend,
            self.load.clone(),
            |_, _| Ok(()),
        )
    }

    /// Generates one answer within `timeout`. When the timeout runs out
    /// the generation is cancelled and the error names the timeout.
    pub fn generate(
        &self,
        request: &GenerateRequest,
        timeout: Duration,
    ) -> Result<GenerateResult, EngineError> {
        let params = GenerateParams {
            prompt_profile: if request.raw {
                PromptProfile::Raw
            } else {
                PromptProfile::Polish
            },
            system: request.system.clone(),
            user: request.user.clone(),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            stop: request.stop.clone(),
        };
        self.supervisor.with_engine_on(
            LLM_BINARY,
            &self.model,
            None,
            self.backend,
            self.load.clone(),
            |process, _| {
                let result = process.call(
                    "generate",
                    serde_json::to_value(&params).unwrap_or(Value::Null),
                    &[],
                    timeout,
                    |event| tracing::debug!(event = %event.name, "engine event"),
                );
                match result {
                    Ok(v) => serde_json::from_value(v)
                        .map_err(|e| EngineError::Transport(format!("generate response: {e}"))),
                    Err(EngineError::Transport(detail)) if detail.contains("no response") => {
                        let id = process.last_request_id();
                        tracing::warn!(request = id, "generation timed out; cancelling it");
                        if let Err(e) = process.call(
                            "cancel",
                            json!({ "request_id": id }),
                            &[],
                            CANCEL_TIMEOUT,
                            |_| {},
                        ) {
                            // An engine that cannot even answer the cancel
                            // is stuck: it is terminated here, so the next
                            // request restarts it instead of queueing
                            // behind the answer it will never give.
                            tracing::warn!(error = %e, "the engine did not acknowledge the cancel; terminating it");
                            process.terminate();
                        }
                        Err(EngineError::Transport(format!(
                            "generate: no answer within {timeout:?}; cancelled"
                        )))
                    }
                    Err(e) => Err(e),
                }
            },
        )
    }

    /// The backend of the loaded model, when loaded.
    pub fn backend(&self) -> Option<Backend> {
        self.supervisor
            .status(&[LLM_BINARY])
            .first()
            .and_then(|s| s.backend)
    }

    /// The engine's state without loading anything: the supervisor's row
    /// and, when the process runs and is free, its own `status`.
    pub fn status(&self) -> LlmStatus {
        let engine = self
            .supervisor
            .status(&[LLM_BINARY])
            .into_iter()
            .next()
            .expect("one row per binary asked for");
        let probe = self.supervisor.with_running(LLM_BINARY, |process| {
            process.call("status", json!({}), &[], STATUS_TIMEOUT, |_| {})
        });
        match probe {
            Some(Ok(Ok(v))) => {
                let s: StatusResult = serde_json::from_value(v).unwrap_or(StatusResult {
                    loaded: engine.model.is_some(),
                    model: engine.model.clone(),
                    backend: engine.backend,
                    busy: false,
                    memory_bytes: None,
                });
                LlmStatus {
                    loaded: s.loaded,
                    busy: s.busy,
                    memory_bytes: s.memory_bytes,
                    engine,
                }
            }
            Some(Err(EngineError::Busy)) => LlmStatus {
                loaded: engine.model.is_some(),
                busy: true,
                memory_bytes: None,
                engine,
            },
            _ => LlmStatus {
                loaded: false,
                busy: false,
                memory_bytes: None,
                engine,
            },
        }
    }
}
