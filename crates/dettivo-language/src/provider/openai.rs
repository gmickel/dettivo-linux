//! The OpenAI-compatible provider (FR-M6): any endpoint that answers
//! `POST /v1/chat/completions`, with the key from the Secret Service or
//! `[llm] api_key_file`, and the trust gate in front of anything that is
//! not loopback.

use std::time::Duration;

use serde::Deserialize;

use super::{
    Availability, LlmProvider, ProviderError, RewriteRequest, api_key, off_runtime, trust,
};

/// How long detection waits before calling the endpoint absent.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// A chat endpoint, its model and the key that opens it.
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleProvider {
    endpoint: String,
    model: String,
    trusted: Vec<String>,
    api_key_file: String,
}

impl OpenAiCompatibleProvider {
    /// A provider for `endpoint`, asked for `model`, allowed to reach a
    /// non-loopback host only when `trusted` holds it.
    pub fn new(endpoint: &str, model: &str, trusted: Vec<String>, api_key_file: &str) -> Self {
        Self {
            endpoint: endpoint.trim().to_string(),
            model: model.trim().to_string(),
            trusted,
            api_key_file: api_key_file.trim().to_string(),
        }
    }

    /// The canonical base, or why the endpoint cannot be reached.
    pub fn base(&self) -> Result<String, ProviderError> {
        if self.endpoint.is_empty() {
            return Err(ProviderError::Unavailable(
                "[llm] endpoint_url is empty".into(),
            ));
        }
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

    fn call(&self, request: &RewriteRequest, timeout: Duration) -> Result<String, ProviderError> {
        let base = self.base()?;
        if self.model.is_empty() {
            return Err(ProviderError::Unavailable(
                "[llm] endpoint_model is empty".into(),
            ));
        }
        // The endpoint that passed the trust gate is the only one the
        // transcript goes to: a redirect is answered as a failure, never
        // followed to a host nobody confirmed.
        let client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| ProviderError::Failed(short(&e.to_string())))?;
        let body = serde_json::json!({
            "model": self.model,
            "stream": false,
            "temperature": 0.2,
            "messages": [
                {"role": "system", "content": request.system},
                {"role": "user", "content": request.user},
            ],
        });
        let mut call = client
            .post(format!("{base}/v1/chat/completions"))
            .header("content-type", "application/json")
            .body(body.to_string());
        if let Some(key) = api_key(&self.api_key_file) {
            call = call.bearer_auth(key);
        }
        let response = call.send().map_err(|e| {
            if e.is_timeout() {
                ProviderError::Timeout
            } else {
                ProviderError::Unavailable(short(&e.to_string()))
            }
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::Failed(format!(
                "{base}/v1/chat/completions answered {status}"
            )));
        }
        let payload = response
            .text()
            .map_err(|e| ProviderError::Failed(short(&e.to_string())))?;
        let completion: Completion = serde_json::from_str(&payload)
            .map_err(|e| ProviderError::Failed(short(&e.to_string())))?;
        let text = completion
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default();
        if text.trim().is_empty() {
            return Err(ProviderError::Failed("the model returned nothing".into()));
        }
        Ok(text)
    }
}

fn short(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect()
}

#[derive(Debug, Deserialize)]
struct Completion {
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: CompletionMessage,
}

#[derive(Debug, Deserialize)]
struct CompletionMessage {
    #[serde(default)]
    content: String,
}

impl OpenAiCompatibleProvider {
    /// The probe itself, run on a thread of its own by [`LlmProvider::probe`].
    fn probe_now(&self) -> Availability {
        let base = match self.base() {
            Ok(base) => base,
            Err(ProviderError::EndpointNotTrusted { endpoint }) => {
                return Availability::down(
                    format!("{endpoint} is not a trusted endpoint"),
                    Some(format!("dettivo llm trust {endpoint}")),
                );
            }
            Err(e) => return Availability::down(e.to_string(), None),
        };
        if self.model.is_empty() {
            return Availability::down(
                "[llm] endpoint_model is empty",
                Some("set [llm] endpoint_model".into()),
            );
        }
        let client = match reqwest::blocking::Client::builder()
            .timeout(PROBE_TIMEOUT)
            .build()
        {
            Ok(c) => c,
            Err(e) => return Availability::down(short(&e.to_string()), None),
        };
        let mut call = client.get(format!("{base}/v1/models"));
        if let Some(key) = api_key(&self.api_key_file) {
            call = call.bearer_auth(key);
        }
        match call.send() {
            Ok(r) if r.status().is_success() => {
                Availability::up(format!("{base} answers with {}", self.model))
            }
            Ok(r) => Availability::down(
                format!("{base}/v1/models answered {}", r.status()),
                Some("check [llm] endpoint_url and the API key".into()),
            ),
            Err(e) => Availability::down(
                short(&e.to_string()),
                Some("check [llm] endpoint_url".into()),
            ),
        }
    }
}

impl LlmProvider for OpenAiCompatibleProvider {
    fn name(&self) -> &'static str {
        "openai_compatible"
    }

    fn model(&self) -> String {
        self.model.clone()
    }

    fn probe(&self) -> Availability {
        match off_runtime("the endpoint probe", || self.probe_now()) {
            Ok(availability) => availability,
            Err(e) => Availability::down(e.to_string(), None),
        }
    }

    fn rewrite(
        &self,
        request: &RewriteRequest,
        timeout: Duration,
    ) -> Result<String, ProviderError> {
        off_runtime("the endpoint chat request", || self.call(request, timeout))?
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
            "/v1/chat/completions".into(),
            200,
            r#"{"choices":[{"message":{"role":"assistant","content":"Stolen."}}]}"#.into(),
        )]);
        let endpoint = MockServer::start(vec![(
            "/v1/chat/completions".into(),
            307,
            format!("{}/v1/chat/completions", receiver.url()),
        )]);
        let provider = OpenAiCompatibleProvider::new(&endpoint.url(), "m", Vec::new(), "");
        let err = provider
            .rewrite(&request(), Duration::from_secs(5))
            .unwrap_err();
        assert!(
            matches!(err, ProviderError::Failed(ref m) if m.contains("307")),
            "{err:?}"
        );
        assert!(receiver.requests().is_empty(), "the receiver saw nothing");
        assert_eq!(endpoint.requests().len(), 1);
    }

    #[test]
    fn a_model_override_binds_the_model_the_request_carries() {
        let server = MockServer::start(vec![(
            "/v1/chat/completions".into(),
            200,
            r#"{"choices":[{"message":{"role":"assistant","content":"Hi."}}]}"#.into(),
        )]);
        let base = OpenAiCompatibleProvider::new(&server.url(), "base", Vec::new(), "");
        let bound = base.with_model("special").unwrap();
        assert_eq!(bound.model(), "special");
        bound.rewrite(&request(), Duration::from_secs(5)).unwrap();
        let (_, body) = server.requests().pop().unwrap();
        let body: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["model"], "special");
    }

    fn request() -> RewriteRequest {
        RewriteRequest {
            system: "s".into(),
            user: "u".into(),
        }
    }

    #[test]
    fn a_local_endpoint_rewrites_through_chat_completions() {
        let server = MockServer::start(vec![
            ("/v1/models".into(), 200, r#"{"data":[]}"#.into()),
            (
                "/v1/chat/completions".into(),
                200,
                r#"{"choices":[{"message":{"role":"assistant","content":"Hello there."}}]}"#.into(),
            ),
        ]);
        let provider = OpenAiCompatibleProvider::new(&server.url(), "local-model", Vec::new(), "");
        assert!(provider.probe().available);
        assert_eq!(
            provider
                .rewrite(&request(), Duration::from_secs(5))
                .unwrap(),
            "Hello there."
        );
    }

    #[test]
    fn a_remote_endpoint_is_refused_until_it_is_trusted() {
        let remote =
            OpenAiCompatibleProvider::new("https://llm.example.com/v1", "m", Vec::new(), "");
        assert_eq!(
            remote.rewrite(&request(), Duration::from_secs(1)),
            Err(ProviderError::EndpointNotTrusted {
                endpoint: "https://llm.example.com".into()
            })
        );
        let availability = remote.probe();
        assert!(!availability.available);
        assert_eq!(
            availability.hint.as_deref(),
            Some("dettivo llm trust https://llm.example.com")
        );
    }

    #[test]
    fn an_endpoint_that_errors_is_reported_rather_than_inserted() {
        let server = MockServer::start(vec![("/v1/chat/completions".into(), 500, "boom".into())]);
        let provider = OpenAiCompatibleProvider::new(&server.url(), "m", Vec::new(), "");
        assert!(matches!(
            provider.rewrite(&request(), Duration::from_secs(5)),
            Err(ProviderError::Failed(_))
        ));
        let empty = OpenAiCompatibleProvider::new("", "m", Vec::new(), "");
        assert!(matches!(
            empty.rewrite(&request(), Duration::from_secs(1)),
            Err(ProviderError::Unavailable(_))
        ));
    }
}
