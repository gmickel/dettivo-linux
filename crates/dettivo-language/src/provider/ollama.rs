//! The Ollama provider (FR-M6): detected through `GET /api/tags` on the
//! configured endpoint (loopback by default), asked for a rewrite through
//! `POST /api/chat` with the prompt profile, and reporting the pull
//! command when the endpoint answers but the model is not there.

use std::time::Duration;

use serde::Deserialize;

use super::{Availability, LlmProvider, ProviderError, RewriteRequest, off_runtime, trust};

/// The models the settings screens recommend, best first.
pub const RECOMMENDED_MODELS: &[&str] =
    &["qwen3:4b-instruct", "qwen3:1.7b", "qwen3:4b", "qwen3:8b"];

/// How long detection waits before calling the endpoint absent.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Ollama at an endpoint, asked for one model.
#[derive(Debug, Clone)]
pub struct OllamaProvider {
    endpoint: String,
    model: String,
    trusted: Vec<String>,
}

impl OllamaProvider {
    /// A provider for `endpoint` and `model`. A non-loopback endpoint
    /// still has to pass the trust gate, with `trusted` empty by default.
    pub fn new(endpoint: &str, model: &str) -> Self {
        Self {
            endpoint: endpoint.trim().to_string(),
            model: model.trim().to_string(),
            trusted: Vec::new(),
        }
    }

    /// The same provider with the endpoints the user confirmed.
    pub fn trusting(mut self, trusted: Vec<String>) -> Self {
        self.trusted = trusted;
        self
    }

    fn base(&self) -> Result<String, ProviderError> {
        match trust::check(&self.endpoint, &self.trusted) {
            trust::Trust::Allowed => {
                trust::canonicalize(&self.endpoint).map_err(ProviderError::Unavailable)
            }
            trust::Trust::NeedsConfirmation { endpoint } => {
                Err(ProviderError::EndpointNotTrusted { endpoint })
            }
            trust::Trust::Invalid { reason } => Err(ProviderError::Unavailable(reason)),
        }
    }

    /// The models the endpoint holds.
    pub fn models(&self) -> Result<Vec<String>, ProviderError> {
        off_runtime("the Ollama tags request", || self.tags())?
    }

    fn tags(&self) -> Result<Vec<String>, ProviderError> {
        let base = self.base()?;
        let client = client(PROBE_TIMEOUT)?;
        let response = client
            .get(format!("{base}/api/tags"))
            .send()
            .map_err(|e| ProviderError::Unavailable(short(&e.to_string())))?;
        let status = response.status();
        if status.as_u16() == 404 {
            return Err(ProviderError::Unavailable(format!(
                "{base} is not an Ollama endpoint"
            )));
        }
        if !status.is_success() {
            return Err(ProviderError::Unavailable(format!(
                "{base} answered {status}"
            )));
        }
        let body = response
            .text()
            .map_err(|e| ProviderError::Failed(short(&e.to_string())))?;
        let tags: TagsResponse = serde_json::from_str(&body)
            .map_err(|e| ProviderError::Failed(short(&e.to_string())))?;
        Ok(tags.models.into_iter().map(|m| m.name).collect())
    }
}

/// A client that never follows a redirect: the endpoint that passed the
/// trust gate is the only host a request body reaches.
fn client(timeout: Duration) -> Result<reqwest::blocking::Client, ProviderError> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| ProviderError::Failed(short(&e.to_string())))
}

/// The first line of an error, so a log never carries a whole body.
fn short(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect()
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagEntry>,
}

#[derive(Debug, Deserialize)]
struct TagEntry {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    #[serde(default)]
    message: Option<ChatMessage>,
    #[serde(default)]
    response: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: String,
}

/// True when `installed` holds `model`, allowing the `:latest` suffix
/// Ollama adds to a tagless name.
pub fn holds_model(installed: &[String], model: &str) -> bool {
    installed
        .iter()
        .any(|m| m == model || m.trim_end_matches(":latest") == model.trim_end_matches(":latest"))
}

impl LlmProvider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn model(&self) -> String {
        self.model.clone()
    }

    fn probe(&self) -> Availability {
        match self.models() {
            Ok(models) if holds_model(&models, &self.model) => {
                Availability::up(format!("{} has {}", self.endpoint, self.model))
            }
            Ok(_) => Availability::down(
                format!("{} does not have {}", self.endpoint, self.model),
                Some(format!("ollama pull {}", self.model)),
            ),
            Err(ProviderError::EndpointNotTrusted { endpoint }) => Availability::down(
                format!("{endpoint} is not a trusted endpoint"),
                Some(format!("dettivo llm trust {endpoint}")),
            ),
            Err(e) => Availability::down(
                e.to_string(),
                Some("start Ollama, or set [llm] ollama_url".into()),
            ),
        }
    }

    fn rewrite(
        &self,
        request: &RewriteRequest,
        timeout: Duration,
    ) -> Result<String, ProviderError> {
        let base = self.base()?;
        let client = client(timeout)?;
        let body = serde_json::json!({
            "model": self.model,
            "stream": false,
            "options": {"temperature": 0.2},
            "messages": [
                {"role": "system", "content": request.system},
                {"role": "user", "content": request.user},
            ],
        });
        let response = client
            .post(format!("{base}/api/chat"))
            .header("content-type", "application/json")
            .body(body.to_string())
            .send()
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::Timeout
                } else {
                    ProviderError::Unavailable(short(&e.to_string()))
                }
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::Failed(format!(
                "{base}/api/chat answered {status}"
            )));
        }
        let payload = response
            .text()
            .map_err(|e| ProviderError::Failed(short(&e.to_string())))?;
        let chat: ChatResponse = serde_json::from_str(&payload)
            .map_err(|e| ProviderError::Failed(short(&e.to_string())))?;
        let text = chat
            .message
            .map(|m| m.content)
            .or(chat.response)
            .unwrap_or_default();
        if text.trim().is_empty() {
            return Err(ProviderError::Failed("the model returned nothing".into()));
        }
        Ok(text)
    }

    fn with_model(&self, model: &str) -> Option<Box<dyn LlmProvider>> {
        Some(Box::new(Self {
            model: model.trim().to_string(),
            ..self.clone()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::testing::MockServer;

    #[test]
    fn a_redirect_from_the_endpoint_is_a_failure_and_the_transcript_never_follows_it() {
        let receiver = MockServer::start(vec![(
            "/api/chat".into(),
            200,
            r#"{"message":{"role":"assistant","content":"Stolen."}}"#.into(),
        )]);
        let endpoint = MockServer::start(vec![(
            "/api/chat".into(),
            308,
            format!("{}/api/chat", receiver.url()),
        )]);
        let provider = OllamaProvider::new(&endpoint.url(), "qwen3:4b");
        let err = provider
            .rewrite(
                &RewriteRequest {
                    system: "s".into(),
                    user: "the transcript".into(),
                },
                Duration::from_secs(5),
            )
            .unwrap_err();
        assert!(
            matches!(err, ProviderError::Failed(ref m) if m.contains("308")),
            "{err:?}"
        );
        assert!(receiver.requests().is_empty(), "the receiver saw nothing");
        let bound = provider.with_model("llama3").unwrap();
        assert_eq!(bound.model(), "llama3");
    }

    #[test]
    fn a_model_the_endpoint_holds_is_available_and_rewrites() {
        let server = MockServer::start(vec![
            (
                "/api/tags".into(),
                200,
                r#"{"models":[{"name":"qwen3:4b-instruct"}]}"#.into(),
            ),
            (
                "/api/chat".into(),
                200,
                r#"{"message":{"role":"assistant","content":"Hello there."}}"#.into(),
            ),
        ]);
        let provider = OllamaProvider::new(&server.url(), "qwen3:4b-instruct");
        assert!(provider.probe().available);
        assert_eq!(
            provider
                .rewrite(
                    &RewriteRequest {
                        system: "s".into(),
                        user: "u".into()
                    },
                    Duration::from_secs(5)
                )
                .unwrap(),
            "Hello there."
        );
    }

    #[test]
    fn a_missing_model_names_the_pull_command() {
        let server = MockServer::start(vec![(
            "/api/tags".into(),
            200,
            r#"{"models":[{"name":"llama3:8b"}]}"#.into(),
        )]);
        let availability = OllamaProvider::new(&server.url(), "qwen3:4b-instruct").probe();
        assert!(!availability.available);
        assert_eq!(
            availability.hint.as_deref(),
            Some("ollama pull qwen3:4b-instruct")
        );
    }

    #[test]
    fn a_remote_endpoint_is_refused_until_it_is_trusted() {
        let provider = OllamaProvider::new("https://ollama.example.com", "qwen3:4b");
        assert_eq!(
            provider.models(),
            Err(ProviderError::EndpointNotTrusted {
                endpoint: "https://ollama.example.com".into()
            })
        );
        assert!(holds_model(&["qwen3:4b".to_string()], "qwen3:4b"));
        assert!(holds_model(
            &["qwen3:4b:latest".to_string()],
            "qwen3:4b:latest"
        ));
        assert!(!holds_model(&["llama3".to_string()], "qwen3:4b"));
    }
}
