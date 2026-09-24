//! The `app_code` to HTTP status map the contract fixes (`docs/api/
//! dettivo-rest-v1.md` section 6) and the two response shapes: a JSON
//! result, and the JSON-RPC error object under `error`.

use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use serde_json::{Value, json};

use crate::http::{Body, Response};

/// The content type every JSON answer carries (the macOS spelling).
pub const JSON: &str = "application/json; charset=utf-8";

/// The HTTP status for an app code: `400`, `401`, `404`, `409`, `429`,
/// `501`, `503`, else `500`.
pub fn http_status(code: AppCode) -> u16 {
    match code {
        AppCode::InvalidParams => 400,
        AppCode::UnauthorizedClient => 401,
        AppCode::NotFound => 404,
        AppCode::Conflict => 409,
        AppCode::RateLimitedLocal => 429,
        AppCode::NotImplemented => 501,
        AppCode::AppNotRunning => 503,
        AppCode::InternalError => 500,
    }
}

/// A `200` with a JSON body.
pub fn ok(value: &Value) -> Response {
    json_response(200, value)
}

/// Any status with a JSON body.
pub fn json_response(status: u16, value: &Value) -> Response {
    Response {
        status,
        headers: vec![("Content-Type".into(), JSON.into())],
        body: Body::Bytes(serde_json::to_vec_pretty(value).unwrap_or_default()),
    }
}

/// The error response: the status from the app code, the body
/// `{"error": <JSON-RPC error object>}`.
pub fn error(err: &JsonRpcError) -> Response {
    json_response(http_status(err.app_code()), &json!({ "error": err }))
}

/// A `413` for a body over the cap: `INVALID_PARAMS` naming the cap,
/// the only case where the status is not the app code's.
pub fn too_large(declared: u64, cap: u64) -> Response {
    let err = JsonRpcError::new(
        AppCode::InvalidParams,
        format!(
            "request body of {declared} bytes exceeds the REST body limit of {cap} bytes (rest.max_body_bytes); use `dettivo history import` for larger files"
        ),
        ErrorDetails::empty(),
    );
    json_response(413, &json!({ "error": err }))
}

/// An `INVALID_PARAMS` error.
pub fn invalid(message: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(AppCode::InvalidParams, message, ErrorDetails::empty())
}

/// A `NOT_FOUND` error.
pub fn not_found(message: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(AppCode::NotFound, message, ErrorDetails::empty())
}

/// An `INTERNAL_ERROR`.
pub fn internal(message: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(AppCode::InternalError, message, ErrorDetails::empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_map_is_the_contracts() {
        let cases = [
            (AppCode::InvalidParams, 400),
            (AppCode::UnauthorizedClient, 401),
            (AppCode::NotFound, 404),
            (AppCode::Conflict, 409),
            (AppCode::RateLimitedLocal, 429),
            (AppCode::NotImplemented, 501),
            (AppCode::AppNotRunning, 503),
            (AppCode::InternalError, 500),
        ];
        for (code, status) in cases {
            assert_eq!(http_status(code), status, "{code:?}");
        }
        let response = error(&JsonRpcError::not_implemented("knowledge.search"));
        assert_eq!(response.status, 501);
        let Body::Bytes(bytes) = response.body else {
            panic!("JSON body must be bytes")
        };
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["error"]["code"], -32014);
        assert_eq!(body["error"]["data"]["app_code"], "NOT_IMPLEMENTED");
        assert_eq!(body["error"]["data"]["retryable"], false);
        assert_eq!(too_large(1, 0).status, 413);
    }
}
