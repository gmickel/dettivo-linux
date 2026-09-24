//! The daemon's local language model (ADR 0026): `[llm] model` resolved
//! through the catalogue to a file under `<models>/llm/<id>/`, driven by
//! `dettivo-engine-llm` over the supervisor, and offered to the language
//! crate as its `LocalEngine` so the `local` provider, `polish.test`,
//! `dettivo llm test` and every Enhanced dictation reach the same
//! process. `[llm] polish_experiment` swaps the file for a sideloaded
//! fine-tune (ADR 0032): a fused GGUF loads in place of the catalogue
//! model, a `gguf-lora` adapter loads over its catalogue base, and a
//! manifest that does not resolve leaves Enhanced on `[llm] model` with
//! the error kept for `llm.models.status`. Availability is what
//! `llm.providers.list`, `dettivo doctor` and
//! `system.capabilities.llm.local_available` report: the file is on
//! disk, the binary is found and the engine is not degraded.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use dettivo_core::config::Loaded;
use dettivo_core::paths::Paths;
use dettivo_engine_proto::messages::FinishReason;
use dettivo_language::provider::{
    Answer, Availability, Finish, LocalEngine, ProviderError, RewriteRequest,
};
use dettivo_speech::EngineError;
use dettivo_speech::llm::sideload::{self, Format, Sideload, SideloadError};
use dettivo_speech::llm::{GenerateRequest, LlmEngine, LlmStatus};
use dettivo_speech::supervisor::Supervisor;

use crate::verify::Verifier;

/// The sampling temperature every rewrite runs at (the Ollama provider
/// asks for the same).
const TEMPERATURE: f64 = 0.2;

/// The local engine bound to the configured model.
pub struct LocalLlm {
    engine: LlmEngine,
    model_id: String,
    model_path: PathBuf,
    /// The catalogue id of the file at `model_path` (the base of an
    /// adapter, or `model_id` itself).
    base_id: String,
    max_tokens: u32,
    idle_seconds: u64,
    experiment: Option<Result<Sideload, SideloadError>>,
    /// The gate the catalogue file passes before the engine opens it.
    verifier: Verifier,
}

impl LocalLlm {
    /// Binds `[llm] model` (or the sideload `[llm] polish_experiment`
    /// names) and `[engines.llm]` to the supervisor.
    pub fn new(
        supervisor: Arc<Supervisor>,
        paths: &Paths,
        loaded: &Loaded,
        verifier: Verifier,
    ) -> Self {
        let llm = &loaded.config.llm;
        let experiment = llm.polish_experiment().map(|name| {
            let dir = llm.experiments_dir(&loaded.models_dir(paths));
            sideload::resolve(&dir, name)
        });
        Self::build(supervisor, paths, loaded, &llm.model, experiment, verifier)
    }

    /// Binds a catalogue model by id (`[llm] analysis_model` for the
    /// meeting analysis, ADR 0036) and `[engines.llm]` to the supervisor;
    /// the polish experiment never applies here.
    pub fn for_model(
        supervisor: Arc<Supervisor>,
        paths: &Paths,
        loaded: &Loaded,
        model: &str,
        verifier: Verifier,
    ) -> Self {
        Self::build(supervisor, paths, loaded, model, None, verifier)
    }

    fn build(
        supervisor: Arc<Supervisor>,
        paths: &Paths,
        loaded: &Loaded,
        model: &str,
        experiment: Option<Result<Sideload, SideloadError>>,
        verifier: Verifier,
    ) -> Self {
        let section = &loaded.config.engines.llm;
        let catalogue_path = |id: &str| {
            crate::engines::model_path(paths, loaded, "llm", id).unwrap_or_else(|| {
                loaded
                    .models_dir(paths)
                    .join("llm")
                    .join(id)
                    .join(format!("{id}.gguf"))
            })
        };
        let mut model_id = model.to_string();
        let mut base_id = model.to_string();
        let mut model_path = catalogue_path(model);
        let mut lora = None;
        match &experiment {
            Some(Ok(s)) => {
                model_id = s.id.clone();
                match (s.format, s.base_model.as_deref()) {
                    (Format::GgufLora, Some(base)) => {
                        base_id = base.to_string();
                        model_path = catalogue_path(base);
                        lora = Some(s.gguf.to_string_lossy().into_owned());
                    }
                    _ => {
                        base_id = s.id.clone();
                        model_path = s.gguf.clone();
                    }
                }
                tracing::info!(experiment = %s.id, format = s.format.as_str(), "polish experiment in force");
            }
            Some(Err(e)) => {
                tracing::warn!(error = %e, "polish experiment does not resolve; Enhanced runs on [llm] model");
            }
            None => {}
        }
        let engine = LlmEngine::new(
            supervisor,
            model_path.to_string_lossy().into_owned(),
            crate::engines::backend_of(loaded, "llm"),
            Some(section.context_length.max(256)),
        )
        .with_lora(lora);
        Self {
            engine,
            model_id,
            model_path,
            base_id,
            max_tokens: section.max_tokens.max(1),
            idle_seconds: loaded.config.engines.llm_idle_seconds,
            experiment,
            verifier,
        }
    }

    /// The model id a rewrite names: the catalogue id, or the sideload's.
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// The model file the engine loads (the base, for an adapter).
    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// The adapter applied over it, when one is.
    pub fn lora(&self) -> Option<&str> {
        self.engine.lora()
    }

    /// `[engines] llm_idle_seconds`.
    pub fn idle_seconds(&self) -> u64 {
        self.idle_seconds
    }

    /// The sideload `[llm] polish_experiment` names, resolved or not;
    /// `None` when the key is empty.
    pub fn experiment(&self) -> Option<&Result<Sideload, SideloadError>> {
        self.experiment.as_ref()
    }

    /// The engine's state without loading anything.
    pub fn status(&self) -> LlmStatus {
        self.engine.status()
    }

    /// Whether a rewrite would reach the model right now.
    pub fn available(&self) -> bool {
        self.probe().available
    }

    /// The missing file that keeps a rewrite from the model, worded with
    /// what to do: the catalogue file first (its download command), then
    /// the adapter.
    fn missing(&self) -> Option<(String, Option<String>)> {
        if !self.model_path.is_file() {
            let what = if self.base_id == self.model_id {
                format!("llm/{} is not downloaded", self.base_id)
            } else {
                format!(
                    "llm/{} (the base of experiment {}) is not downloaded",
                    self.base_id, self.model_id
                )
            };
            let hint = match &self.experiment {
                Some(Ok(s)) if s.format == Format::Gguf => Some(format!(
                    "{} names a file that is gone",
                    s.manifest.display()
                )),
                _ => Some(download_command(&self.base_id)),
            };
            return Some((what, hint));
        }
        match self.lora() {
            Some(path) if !Path::new(path).is_file() => Some((
                format!("adapter {path} of experiment {} is gone", self.model_id),
                Some("re-run scripts/models/convert-polish-finetune.sh".into()),
            )),
            _ => None,
        }
    }
}

/// The CLI command that fetches the model.
pub fn download_command(model: &str) -> String {
    format!("dettivo llm download --model {model}")
}

/// The engine's `finish_reason` as the provider layer reads it: a
/// generation the budget cut short is `Length`, so the analysis splits
/// its part instead of repairing an answer that could never have
/// finished. A generation a cancel cut short is no answer at all, and
/// comes back as the error an engine cancel raises, so nothing splits
/// or retries on it.
fn finish_of(reason: FinishReason) -> Result<Finish, ProviderError> {
    match reason {
        FinishReason::Stop => Ok(Finish::Stop),
        FinishReason::Length => Ok(Finish::Length),
        FinishReason::Cancelled => Err(provider_error(EngineError::Cancelled)),
    }
}

fn provider_error(e: EngineError) -> ProviderError {
    match e {
        EngineError::Transport(m) if m.contains("no answer") || m.contains("no response") => {
            ProviderError::Timeout
        }
        EngineError::Cancelled => ProviderError::Timeout,
        EngineError::Engine { code, message }
            if code == "bad_request" && message.contains("exceeds the context") =>
        {
            ProviderError::PromptTooLong(message)
        }
        EngineError::Engine { code, message } => {
            ProviderError::Failed(format!("{code}: {message}"))
        }
        EngineError::Busy => ProviderError::Failed("the engine is busy".into()),
        other => ProviderError::Unavailable(other.to_string()),
    }
}

impl LocalEngine for LocalLlm {
    fn model(&self) -> String {
        self.model_id.clone()
    }

    fn probe(&self) -> Availability {
        if let Some((what, hint)) = self.missing() {
            return Availability::down(what, hint);
        }
        if let Err(why) = self.verifier.check(&self.model_path) {
            return Availability::down(why, None);
        }
        let status = self.engine.status();
        let Some(path) = status.engine.path.as_ref() else {
            return Availability::down(
                format!("{} not found", status.engine.binary),
                Some("install dettivo-engine-llm or set [engines] directory".into()),
            );
        };
        if status.engine.degraded {
            return Availability::down(
                format!(
                    "{} is degraded after {} crashes",
                    status.engine.binary, status.engine.crashes
                ),
                Some("check the daemon log; the next request retries".into()),
            );
        }
        Availability::up(format!(
            "{} on disk; {}{}",
            match &self.experiment {
                Some(Ok(s)) => format!("experiment {} ({})", s.id, s.format.as_str()),
                _ => format!("llm/{}", self.model_id),
            },
            path.display(),
            match status.engine.backend {
                Some(b) if status.engine.running => format!(" running on {b:?}").to_lowercase(),
                _ => String::new(),
            }
        ))
    }

    fn generate(
        &self,
        request: &RewriteRequest,
        timeout: Duration,
    ) -> Result<Answer, ProviderError> {
        self.verifier
            .ensure(&self.model_path)
            .map_err(ProviderError::Unavailable)?;
        if let Some((what, hint)) = self.missing() {
            return Err(ProviderError::Unavailable(match hint {
                Some(h) => format!("{what}; run `{h}`"),
                None => what,
            }));
        }
        let result = self
            .engine
            .generate(
                &GenerateRequest {
                    system: Some(request.system.clone()),
                    user: request.user.clone(),
                    max_tokens: self.max_tokens,
                    temperature: TEMPERATURE,
                    stop: Vec::new(),
                    raw: false,
                },
                timeout,
            )
            .map_err(provider_error)?;
        let finish = finish_of(result.finish_reason)?;
        if result.text.trim().is_empty() {
            return Err(ProviderError::Failed("the model returned nothing".into()));
        }
        Ok(Answer {
            text: result.text,
            finish,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_errors_map_onto_the_provider_layer() {
        assert_eq!(
            provider_error(EngineError::Transport(
                "generate: no answer within 8s; cancelled".into()
            )),
            ProviderError::Timeout
        );
        assert_eq!(
            provider_error(EngineError::Engine {
                code: "bad_request".into(),
                message: "prompt too long".into()
            }),
            ProviderError::Failed("bad_request: prompt too long".into())
        );
        assert_eq!(
            provider_error(EngineError::Engine {
                code: "bad_request".into(),
                message: "prompt of 4100 tokens exceeds the context of 4096".into()
            }),
            ProviderError::PromptTooLong(
                "prompt of 4100 tokens exceeds the context of 4096".into()
            )
        );
        assert_eq!(finish_of(FinishReason::Stop), Ok(Finish::Stop));
        assert_eq!(finish_of(FinishReason::Length), Ok(Finish::Length));
        assert_eq!(
            finish_of(FinishReason::Cancelled),
            Err(provider_error(EngineError::Cancelled))
        );
        assert_eq!(
            finish_of(FinishReason::Cancelled),
            Err(ProviderError::Timeout)
        );
        assert!(matches!(
            provider_error(EngineError::Crashed("boom".into())),
            ProviderError::Unavailable(_)
        ));
        assert!(matches!(
            provider_error(EngineError::Degraded),
            ProviderError::Unavailable(_)
        ));
        assert_eq!(
            download_command("qwen3-1.7b"),
            "dettivo llm download --model qwen3-1.7b"
        );
    }
}
