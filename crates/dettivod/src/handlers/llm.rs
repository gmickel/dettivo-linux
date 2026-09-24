//! `llm.*` (Linux additions, ADR 0023): which language model providers
//! the Enhanced pass can reach, and the trust gate that keeps a
//! non-loopback endpoint unreachable until the user confirms it.

use std::sync::Arc;

use dettivo_core::config::edit;
use dettivo_language::provider::{self, LocalEngine, trust};
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::llm::{
    Endpoint, EndpointsListResult, EndpointsTrustParams, EndpointsTrustResult, Provider,
    ProvidersListResult,
};
use serde_json::Value;

use super::{json, params};

use crate::daemon::Daemon;
use crate::handlers::config::edit_values;

/// `llm.providers.list`: every provider `[llm]` describes, probed now.
pub fn providers_list(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let loaded = daemon.config();
    let local: Arc<dyn LocalEngine> = daemon.engines().local_llm();
    let providers: Vec<Provider> = provider::all(&loaded.config.llm, Some(&local))
        .iter()
        .map(|p| {
            let availability = p.probe();
            Provider {
                id: p.name().to_string(),
                model: p.model(),
                available: availability.available,
                detail: availability.detail,
                hint: availability.hint,
            }
        })
        .collect();
    let selected = providers.iter().find(|p| p.available).map(|p| p.id.clone());
    json(ProvidersListResult {
        providers,
        selected,
    })
}

/// `llm.endpoints.trust`: stores the endpoint in `[llm]
/// trusted_endpoints` in its canonical form. A loopback endpoint needs no
/// entry and is answered as already trusted.
pub fn endpoints_trust(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: EndpointsTrustParams = params(params_value)?;
    let canonical = trust::canonicalize(&p.url).map_err(|reason| {
        JsonRpcError::new(AppCode::InvalidParams, reason, ErrorDetails::empty())
    })?;
    if trust::is_loopback(&canonical) {
        return json(EndpointsTrustResult {
            url: canonical,
            trusted: true,
        });
    }
    // The list is read from the file under the edit lock, so two trusts
    // at once keep each other's endpoint.
    let url = canonical.clone();
    edit_values(daemon, |text, current| {
        let mut trusted = current.llm.trusted_endpoints;
        if trusted
            .iter()
            .any(|t| trust::canonicalize(t).ok().as_deref() == Some(url.as_str()))
        {
            return Ok(text);
        }
        trusted.push(url.clone());
        let endpoints = Value::Array(trusted.into_iter().map(Value::String).collect());
        tracing::info!("llm: remote endpoint trusted");
        edit::set(&text, "llm.trusted_endpoints", &endpoints).map_err(|e| {
            JsonRpcError::new(AppCode::InvalidParams, e.message, ErrorDetails::empty())
        })
    })?;
    json(EndpointsTrustResult {
        url: canonical,
        trusted: true,
    })
}

/// `llm.endpoints.list`: the confirmed endpoints, plus the configured
/// ones that answer on this machine and never needed confirming.
pub fn endpoints_list(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let llm = daemon.config().config.llm.clone();
    let mut endpoints: Vec<Endpoint> = Vec::new();
    let mut push = |raw: &str| {
        let Ok(url) = trust::canonicalize(raw) else {
            return;
        };
        if endpoints.iter().any(|e| e.url == url) {
            return;
        }
        let loopback = trust::is_loopback(&url);
        endpoints.push(Endpoint { url, loopback });
    };
    push(&llm.ollama_url);
    push(&llm.endpoint_url);
    for endpoint in &llm.trusted_endpoints {
        push(endpoint);
    }
    json(EndpointsListResult { endpoints })
}
