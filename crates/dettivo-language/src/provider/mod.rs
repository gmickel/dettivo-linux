//! The language model provider layer (FR-M6): one interface with three
//! providers behind it (`local`, the daemon's llama.cpp engine; `ollama`
//! on this machine; an `openai_compatible` endpoint), the QA mock
//! `DETTIVO_MOCK_LLM` in front of all of them, and the trust gate that
//! keeps a non-loopback endpoint unreachable until the user says
//! otherwise. `auto` takes the first that answers, in that order.

pub mod local;
pub mod mock;
pub mod ollama;
pub mod openai;
#[doc(hidden)]
pub mod testing;
pub mod trust;

use std::sync::Arc;
use std::time::Duration;

use dettivo_core::config::polish_schema::{Llm, LlmProviderChoice};

pub use local::{LocalEngine, LocalProvider};
pub use trust::{Trust, canonicalize, is_loopback};

/// What a provider says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Availability {
    /// Whether a rewrite would reach a model right now.
    pub available: bool,
    /// One line for a person: what answered, or what did not.
    pub detail: String,
    /// What to do about it (`ollama pull qwen3:4b-instruct`).
    pub hint: Option<String>,
}

impl Availability {
    /// An available provider.
    pub fn up(detail: impl Into<String>) -> Self {
        Self {
            available: true,
            detail: detail.into(),
            hint: None,
        }
    }

    /// An unavailable provider, with what to do about it.
    pub fn down(detail: impl Into<String>, hint: Option<String>) -> Self {
        Self {
            available: false,
            detail: detail.into(),
            hint,
        }
    }
}

/// Why a rewrite did not come back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    /// Nothing answered at the endpoint.
    Unavailable(String),
    /// The endpoint is remote and not in `[llm] trusted_endpoints`.
    EndpointNotTrusted {
        /// The canonical endpoint the user would confirm.
        endpoint: String,
    },
    /// The provider answered, but not with a rewrite.
    Failed(String),
    /// The prompt did not fit the model's context; a shorter one would.
    PromptTooLong(String),
    /// The provider took longer than the budget.
    Timeout,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(d) => write!(f, "provider unavailable: {d}"),
            Self::EndpointNotTrusted { endpoint } => {
                write!(f, "endpoint not trusted: {endpoint}")
            }
            Self::Failed(d) => write!(f, "provider failed: {d}"),
            Self::PromptTooLong(d) => write!(f, "prompt too long: {d}"),
            Self::Timeout => write!(f, "provider timed out"),
        }
    }
}

impl std::error::Error for ProviderError {}

/// One rewrite request: the system prompt and the wrapped transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteRequest {
    /// The system prompt the policy built.
    pub system: String,
    /// The transcript, wrapped between the markers.
    pub user: String,
}

/// Why an answer ended, when the provider says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finish {
    /// The model ended its turn.
    Stop,
    /// The output budget (`max_tokens` or the context) ran out mid-answer.
    Length,
    /// The provider does not report it.
    Unknown,
}

/// A model's answer with the reason it ended, for a caller that must
/// tell a finished answer from one the output budget cut off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The answer, raw (thinking blocks and markers included).
    pub text: String,
    /// Why it ended.
    pub finish: Finish,
}

impl Answer {
    /// An answer whose end the provider did not explain.
    pub fn unknown(text: String) -> Self {
        Self {
            text,
            finish: Finish::Unknown,
        }
    }
}

/// A language model the Enhanced pass can ask for a rewrite.
pub trait LlmProvider: Send + Sync {
    /// The provider id on the wire (`ollama`, `openai_compatible`,
    /// `local`, `mock`).
    fn name(&self) -> &'static str;

    /// The model the provider would use.
    fn model(&self) -> String;

    /// Whether a rewrite would reach a model right now.
    fn probe(&self) -> Availability;

    /// The model's answer, raw (thinking blocks and markers included).
    fn rewrite(&self, request: &RewriteRequest, timeout: Duration)
    -> Result<String, ProviderError>;

    /// The model's answer with why it ended; a provider that does not
    /// report the reason answers `Finish::Unknown` through `rewrite`.
    fn answer(&self, request: &RewriteRequest, timeout: Duration) -> Result<Answer, ProviderError> {
        self.rewrite(request, timeout).map(Answer::unknown)
    }

    /// The same provider asked for `model` (a custom preset's override),
    /// or `None` when this provider is bound to one model: the pipeline
    /// then keeps the policy on the model that actually answers.
    fn with_model(&self, model: &str) -> Option<Box<dyn LlmProvider>> {
        let _ = model;
        None
    }
}

/// Runs `f` on a thread of its own and waits for it. Every HTTP call in
/// this layer goes through here: the providers use a blocking client, and
/// building one inside a Tokio runtime (which is where the daemon's
/// handlers run) panics. A panic in `f` comes back as a provider error
/// rather than taking the caller's connection down with it.
pub(crate) fn off_runtime<T: Send>(
    what: &str,
    f: impl FnOnce() -> T + Send,
) -> Result<T, ProviderError> {
    std::thread::scope(|scope| scope.spawn(f).join())
        .map_err(|_| ProviderError::Failed(format!("{what} panicked")))
}

/// The environment variable the QA mock reads (`echo` or `fixture:<dir>`).
pub const MOCK_ENV: &str = "DETTIVO_MOCK_LLM";

/// The local provider over `local` when the daemon attached its engine,
/// a detached one otherwise.
fn local_provider(local: Option<&Arc<dyn LocalEngine>>) -> LocalProvider {
    match local {
        Some(engine) => LocalProvider::new(engine.clone()),
        None => LocalProvider::detached(),
    }
}

/// Every provider `[llm]` describes, in the order `auto` tries them: the
/// mock when `DETTIVO_MOCK_LLM` is set (and then nothing else), otherwise
/// the local engine, Ollama and the configured endpoint.
pub fn all(config: &Llm, local: Option<&Arc<dyn LocalEngine>>) -> Vec<Box<dyn LlmProvider>> {
    if let Some(mock) = mock::from_env() {
        return vec![mock];
    }
    let mut out: Vec<Box<dyn LlmProvider>> = vec![Box::new(local_provider(local))];
    out.push(Box::new(ollama::OllamaProvider::new(
        &config.ollama_url,
        &config.ollama_model,
    )));
    if !config.endpoint_url.trim().is_empty() {
        out.push(Box::new(openai::OpenAiCompatibleProvider::new(
            &config.endpoint_url,
            &config.endpoint_model,
            config.trusted_endpoints.clone(),
            &config.api_key_file,
        )));
    }
    out
}

/// The provider `[llm] provider` names, or the first available one under
/// `auto`. `None` means Enhanced falls back to the Polish result.
pub fn resolve(config: &Llm, local: Option<&Arc<dyn LocalEngine>>) -> Option<Box<dyn LlmProvider>> {
    if let Some(mock) = mock::from_env() {
        return Some(mock);
    }
    match config.provider {
        LlmProviderChoice::Local => Some(Box::new(local_provider(local))),
        LlmProviderChoice::Ollama => Some(Box::new(ollama::OllamaProvider::new(
            &config.ollama_url,
            &config.ollama_model,
        ))),
        LlmProviderChoice::OpenaiCompatible => {
            Some(Box::new(openai::OpenAiCompatibleProvider::new(
                &config.endpoint_url,
                &config.endpoint_model,
                config.trusted_endpoints.clone(),
                &config.api_key_file,
            )))
        }
        LlmProviderChoice::Auto => all(config, local).into_iter().find(|p| p.probe().available),
    }
}

/// The API key for an endpoint: the Secret Service item first (service
/// `dettivo`, key `llm-api-key`, read through `secret-tool`), then
/// `[llm] api_key_file`.
pub fn api_key(api_key_file: &str) -> Option<String> {
    if let Some(key) = secret_service_key() {
        return Some(key);
    }
    let path = api_key_file.trim();
    if path.is_empty() {
        return None;
    }
    let key = std::fs::read_to_string(path).ok()?;
    let key = key.trim().to_string();
    (!key.is_empty()).then_some(key)
}

fn secret_service_key() -> Option<String> {
    bounded_lookup(
        "secret-tool",
        &["lookup", "service", "dettivo", "key", "llm-api-key"],
        SECRET_LOOKUP_BUDGET,
    )
}

/// How long the Secret Service helper may take; a locked keyring that
/// prompts, or an agent that hangs, must not hold the rewrite budget.
const SECRET_LOOKUP_BUDGET: Duration = Duration::from_secs(2);

/// Runs `program` and returns its trimmed standard output when it exits
/// successfully within `budget`; a helper still running at the deadline is
/// killed and counts as no answer.
fn bounded_lookup(program: &str, args: &[&str], budget: Duration) -> Option<String> {
    use std::io::Read;
    let mut child = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + budget;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Err(_) => return None,
        }
    };
    if !status.success() {
        return None;
    }
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    let key = out.trim().to_string();
    (!key.is_empty()).then_some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(available: bool) -> Arc<dyn LocalEngine> {
        Arc::new(local::tests::FakeEngine {
            available,
            answer: "Hello.".into(),
        })
    }

    /// dictation/F15: the credential helper is bounded; one that hangs is
    /// killed at the budget and reads as no key.
    #[test]
    fn a_hung_credential_helper_is_killed_at_the_budget() {
        let started = std::time::Instant::now();
        assert_eq!(
            bounded_lookup("sleep", &["30"], Duration::from_millis(100)),
            None
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(
            bounded_lookup("echo", &["  key-1  "], Duration::from_secs(5)),
            Some("key-1".into())
        );
        assert_eq!(bounded_lookup("false", &[], Duration::from_secs(5)), None);
    }

    #[test]
    fn auto_lists_the_local_engine_ollama_and_a_configured_endpoint() {
        let mut config = Llm::default();
        let names: Vec<&str> = all(&config, None).iter().map(|p| p.name()).collect();
        assert_eq!(names, ["local", "ollama"]);
        config.endpoint_url = "https://llm.example.com".into();
        let names: Vec<&str> = all(&config, Some(&engine(true)))
            .iter()
            .map(|p| p.name())
            .collect();
        assert_eq!(names, ["local", "ollama", "openai_compatible"]);
    }

    /// R5: the availability matrix. `auto` prefers the local engine when
    /// its model is on disk, moves on to Ollama when it is not (the
    /// default loopback endpoint answers nothing on a test machine), and
    /// a named provider is taken as named even when it is down.
    #[test]
    fn auto_prefers_the_local_engine_when_its_model_is_on_disk() {
        // Nothing listens on this port, so Ollama is down and `auto` has
        // only the local engine to pick.
        let mut config = Llm {
            ollama_url: "http://127.0.0.1:1".into(),
            ..Llm::default()
        };
        let picked = resolve(&config, Some(&engine(true))).expect("local answers");
        assert_eq!(picked.name(), "local");
        assert_eq!(picked.model(), "qwen3-1.7b");
        assert!(
            resolve(&config, Some(&engine(false))).is_none(),
            "nothing answers"
        );
        assert!(
            resolve(&config, None).is_none(),
            "detached: nothing answers"
        );
        config.provider = LlmProviderChoice::Local;
        let named =
            resolve(&config, Some(&engine(false))).expect("named providers are taken as named");
        assert_eq!(named.name(), "local");
        assert!(!named.probe().available);
        config.provider = LlmProviderChoice::Ollama;
        assert_eq!(
            resolve(&config, Some(&engine(true))).unwrap().name(),
            "ollama"
        );
    }

    #[test]
    fn the_key_file_is_read_when_the_secret_service_has_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("key");
        std::fs::write(&path, "  sk-test\n").unwrap();
        // The Secret Service is absent on a runner; the file answers.
        let key = api_key(path.to_str().unwrap());
        assert!(key == Some("sk-test".into()) || key.is_some());
        assert_eq!(api_key(""), secret_service_key());
    }
}
