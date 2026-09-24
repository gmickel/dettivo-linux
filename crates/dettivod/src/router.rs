//! One request line in, one response line out. The router parses the
//! envelope, checks the token, validates params against the contract
//! catalog, dispatches `system.*` and `config.*`, and answers everything
//! else with the reserved `NOT_IMPLEMENTED` shape (R3).

use std::sync::Arc;

use dettivo_proto::catalog::{self, MethodStatus};
use dettivo_proto::envelope::{ErrorResponse, Request, RequestId, SuccessResponse};
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use serde_json::Value;

use crate::auth;
use crate::daemon::Daemon;
use crate::handlers;

/// Per-connection facts the router needs for every line.
pub struct Session {
    /// The shared daemon; the authentication mode and the expected
    /// token are read from it per request, so a rotation or a mode
    /// change reaches every open connection with the reload.
    pub daemon: Arc<Daemon>,
    /// The connection, for subscriptions.
    pub ctx: handlers::Ctx,
}

/// The daemon's answer to one line: a response to write, or nothing for
/// a notification.
pub enum Outcome {
    /// Write this line back.
    Reply(String),
    /// A notification: no reply.
    Silent,
}

fn invalid_params(message: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(AppCode::InvalidParams, message, ErrorDetails::empty())
}

fn error_line(id: RequestId, error: JsonRpcError) -> Outcome {
    let response = ErrorResponse::new(id, error);
    Outcome::Reply(serde_json::to_string(&response).unwrap_or_default())
}

/// The id a raw request carries when it is one of the three legal shapes,
/// so an error about the rest of the envelope still echoes it.
fn raw_id(raw: &Value) -> RequestId {
    match raw.get("id") {
        Some(Value::String(s)) => RequestId::Text(s.clone()),
        Some(Value::Number(n)) => n.as_i64().map(RequestId::Number).unwrap_or(RequestId::Null),
        _ => RequestId::Null,
    }
}

/// Answers an oversized line (R3): `INVALID_PARAMS`, id unknown.
pub fn oversized(max_bytes: u64) -> Outcome {
    error_line(
        RequestId::Null,
        invalid_params(format!("request line exceeds {max_bytes} bytes")),
    )
}

/// Handles one complete line.
pub fn handle_line(session: &Session, line: &[u8]) -> Outcome {
    let mut raw: Value = match serde_json::from_slice(line) {
        Ok(v) => v,
        Err(_) => return error_line(RequestId::Null, invalid_params("malformed request line")),
    };
    if !raw.is_object() {
        return error_line(
            RequestId::Null,
            invalid_params("request must be a JSON object"),
        );
    }
    let id = raw_id(&raw);
    let provided = auth::take_request_token(&mut raw);
    let request: Request = match serde_json::from_value(raw) {
        Ok(r) => r,
        Err(e) => return error_line(id, invalid_params(e.to_string())),
    };
    let is_notification = request.id.is_none();
    let id = request.id.clone().unwrap_or(RequestId::Null);
    let (mode, expected) = session.daemon.auth_snapshot();
    if let Err(e) = auth::check_request(mode, expected.as_deref(), provided.as_deref()) {
        tracing::info!(code = e.code, "request refused");
        return if is_notification {
            Outcome::Silent
        } else {
            error_line(id, e)
        };
    }
    let result = dispatch(session, &request);
    if is_notification {
        return Outcome::Silent;
    }
    match result {
        Ok(value) => {
            let response = SuccessResponse::new(id, value);
            Outcome::Reply(serde_json::to_string(&response).unwrap_or_default())
        }
        Err(e) => error_line(id, e),
    }
}

fn dispatch(session: &Session, request: &Request) -> Result<Value, JsonRpcError> {
    call(
        &session.daemon,
        &session.ctx,
        &request.method,
        &request.params,
    )
}

/// One authenticated call: the catalog lookup, the reserved and
/// not-yet-implemented answers, the handler and its single typed parameter decode.
/// The socket path and the in-process REST shim (`rest.rs`) both end here.
pub fn call(
    daemon: &Daemon,
    ctx: &handlers::Ctx,
    method: &str,
    params: &Value,
) -> Result<Value, JsonRpcError> {
    let Some(spec) = catalog::lookup(method) else {
        tracing::debug!("unknown method");
        return Err(invalid_params(format!("unknown method {method:?}")));
    };
    if spec.status == MethodStatus::Reserved {
        tracing::debug!(method = spec.name, "reserved method");
        return Err(JsonRpcError::not_implemented(spec.name));
    }
    let Some(handler) = handlers::resolve(spec.name) else {
        tracing::debug!(method = spec.name, "method not implemented yet");
        return Err(JsonRpcError::not_implemented(spec.name));
    };
    let outcome = handler(daemon, ctx, params.clone());
    match &outcome {
        Ok(_) => tracing::debug!(method = spec.name, "ok"),
        Err(e) => tracing::debug!(method = spec.name, code = e.code, "error"),
    }
    outcome
}
