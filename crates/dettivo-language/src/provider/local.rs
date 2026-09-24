//! The `local` provider (ADR 0026): the daemon's llama.cpp engine behind
//! the same `LlmProvider` interface as Ollama and an endpoint. This crate
//! never spawns a process; the daemon attaches its engine through the
//! `LocalEngine` trait, and a provider with nothing attached reports
//! itself unavailable so the Enhanced pass falls back.

use std::sync::Arc;
use std::time::Duration;

use super::{Answer, Availability, LlmProvider, ProviderError, RewriteRequest};

/// What the daemon attaches: its engine over the supervisor, bound to
/// the `[llm] model` on disk.
pub trait LocalEngine: Send + Sync {
    /// The catalogue model id the engine loads.
    fn model(&self) -> String;
    /// Whether a rewrite would reach the model right now: the file is on
    /// disk, the engine binary is found and the engine is not degraded.
    fn probe(&self) -> Availability;
    /// The model's answer, raw, within `timeout`, with why it ended (the
    /// engine's `finish_reason`).
    fn generate(
        &self,
        request: &RewriteRequest,
        timeout: Duration,
    ) -> Result<Answer, ProviderError>;
}

/// The `local` slot of the provider layer.
#[derive(Clone, Default)]
pub struct LocalProvider {
    engine: Option<Arc<dyn LocalEngine>>,
}

impl std::fmt::Debug for LocalProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalProvider")
            .field("attached", &self.engine.is_some())
            .finish()
    }
}

impl LocalProvider {
    /// A provider over the daemon's engine.
    pub fn new(engine: Arc<dyn LocalEngine>) -> Self {
        Self {
            engine: Some(engine),
        }
    }

    /// A provider with no engine attached (a process without the daemon's
    /// supervisor, such as a test); it reports itself unavailable.
    pub fn detached() -> Self {
        Self { engine: None }
    }
}

/// What a detached provider says.
pub const DETACHED: &str = "the local engine is not attached to this process";

impl LlmProvider for LocalProvider {
    fn name(&self) -> &'static str {
        "local"
    }

    fn model(&self) -> String {
        self.engine
            .as_ref()
            .map(|e| e.model())
            .unwrap_or_else(|| "local".into())
    }

    fn probe(&self) -> Availability {
        match &self.engine {
            Some(e) => e.probe(),
            None => Availability::down(
                DETACHED,
                Some("run the rewrite through the daemon (dettivo llm test)".into()),
            ),
        }
    }

    fn rewrite(
        &self,
        request: &RewriteRequest,
        timeout: Duration,
    ) -> Result<String, ProviderError> {
        self.answer(request, timeout).map(|a| a.text)
    }

    fn answer(&self, request: &RewriteRequest, timeout: Duration) -> Result<Answer, ProviderError> {
        match &self.engine {
            Some(e) => e.generate(request, timeout),
            None => Err(ProviderError::Unavailable(DETACHED.into())),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::provider::Finish;

    /// A scripted engine for the availability matrix and the pipeline.
    pub struct FakeEngine {
        pub available: bool,
        pub answer: String,
    }

    impl LocalEngine for FakeEngine {
        fn model(&self) -> String {
            "qwen3-1.7b".into()
        }

        fn probe(&self) -> Availability {
            if self.available {
                Availability::up("qwen3-1.7b is on disk")
            } else {
                Availability::down(
                    "llm/qwen3-1.7b is not downloaded",
                    Some("dettivo llm download --model qwen3-1.7b".into()),
                )
            }
        }

        fn generate(
            &self,
            _request: &RewriteRequest,
            _timeout: Duration,
        ) -> Result<Answer, ProviderError> {
            if self.available {
                Ok(Answer {
                    text: self.answer.clone(),
                    finish: Finish::Stop,
                })
            } else {
                Err(ProviderError::Unavailable("not downloaded".into()))
            }
        }
    }

    #[test]
    fn a_detached_provider_is_unavailable_and_an_attached_one_answers() {
        let detached = LocalProvider::detached();
        assert_eq!(detached.name(), "local");
        assert_eq!(detached.model(), "local");
        assert!(!detached.probe().available);
        let request = RewriteRequest {
            system: String::new(),
            user: String::new(),
        };
        assert_eq!(
            detached.rewrite(&request, Duration::from_secs(1)),
            Err(ProviderError::Unavailable(DETACHED.into()))
        );
        let attached = LocalProvider::new(Arc::new(FakeEngine {
            available: true,
            answer: "Hello.".into(),
        }));
        assert_eq!(attached.model(), "qwen3-1.7b");
        assert!(attached.probe().available);
        assert_eq!(
            attached.rewrite(&request, Duration::from_secs(1)).unwrap(),
            "Hello."
        );
        assert_eq!(
            attached.answer(&request, Duration::from_secs(1)).unwrap(),
            Answer {
                text: "Hello.".into(),
                finish: Finish::Stop,
            }
        );
        assert_eq!(
            detached.answer(&request, Duration::from_secs(1)),
            Err(ProviderError::Unavailable(DETACHED.into()))
        );
        let missing = LocalProvider::new(Arc::new(FakeEngine {
            available: false,
            answer: String::new(),
        }));
        let availability = missing.probe();
        assert!(!availability.available);
        assert_eq!(
            availability.hint.as_deref(),
            Some("dettivo llm download --model qwen3-1.7b")
        );
    }
}
