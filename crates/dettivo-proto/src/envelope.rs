//! The JSON-RPC 2.0 envelope: request, success response, error response and
//! server-to-client notification, one message per line in both directions
//! (`docs/api/dettivo-ipc-v1.md` sections 2.1 and 4).
//!
//! The macOS server echoes whatever `id` a client sent, string or number,
//! and treats a request without an `id` as a notification that gets no
//! response. [`RequestId`] carries exactly those three shapes so a Linux
//! client written against either port sees the same behaviour.

use crate::error::JsonRpcError;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// The fixed `"2.0"` protocol version string every envelope carries.
pub const JSONRPC_VERSION: &str = "2.0";

/// The `jsonrpc` member: always `"2.0"`. Any other value is rejected at
/// deserialisation, so an envelope from another protocol version never
/// parses as a valid message.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct JsonRpcVersion;

impl JsonRpcVersion {
    /// The wire string.
    pub fn as_str(self) -> &'static str {
        JSONRPC_VERSION
    }
}

impl Serialize for JsonRpcVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(JSONRPC_VERSION)
    }
}

impl<'de> Deserialize<'de> for JsonRpcVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        if raw == JSONRPC_VERSION {
            Ok(Self)
        } else {
            Err(D::Error::custom(format!(
                "jsonrpc must be {JSONRPC_VERSION:?}, got {raw:?}"
            )))
        }
    }
}

/// A JSON-RPC request id: a string, an integer, or JSON `null`.
///
/// JSON-RPC 2.0 allows all three; the contract's examples use strings and
/// the macOS server echoes the value back untouched. Floating-point ids are
/// rejected at deserialisation, as the specification recommends.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    /// A string id such as `"req-123"`.
    Text(String),
    /// An integer id.
    Number(i64),
    /// JSON `null`; the server uses it when a request could not be parsed.
    Null,
}

impl From<&str> for RequestId {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<i64> for RequestId {
    fn from(value: i64) -> Self {
        Self::Number(value)
    }
}

/// A JSON-RPC request: `{"jsonrpc","id","method","params"}`.
///
/// A request without an `id` is a notification: the server executes it and
/// sends no response, matching the macOS server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Always `"2.0"`.
    pub jsonrpc: JsonRpcVersion,
    /// Request id, echoed back on the response; absent for a notification.
    #[serde(
        default,
        deserialize_with = "present_id",
        skip_serializing_if = "Option::is_none"
    )]
    pub id: Option<RequestId>,
    /// Dotted method name, for example `"system.health"`.
    pub method: String,
    /// Method params; an empty object when the method takes none.
    #[serde(default = "empty_object")]
    pub params: Value,
}

fn present_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<RequestId>, D::Error> {
    RequestId::deserialize(deserializer).map(Some)
}

fn empty_object() -> Value {
    Value::Object(Default::default())
}

impl Request {
    /// Builds a request with `jsonrpc` set to [`JSONRPC_VERSION`].
    pub fn new(id: impl Into<RequestId>, method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id: Some(id.into()),
            method: method.into(),
            params,
        }
    }

    /// Builds a notification: a request without an `id` that expects no
    /// response.
    pub fn notification(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id: None,
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC success response: `{"jsonrpc","id","result"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuccessResponse {
    /// Always `"2.0"`.
    pub jsonrpc: JsonRpcVersion,
    /// Echoes the request id.
    pub id: RequestId,
    /// Method result.
    pub result: Value,
}

impl SuccessResponse {
    /// Builds a success response with `jsonrpc` set to [`JSONRPC_VERSION`].
    pub fn new(id: impl Into<RequestId>, result: Value) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id: id.into(),
            result,
        }
    }
}

/// A JSON-RPC error response: `{"jsonrpc","id","error"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorResponse {
    /// Always `"2.0"`.
    pub jsonrpc: JsonRpcVersion,
    /// Echoes the request id, or `null` when the request could not be read.
    pub id: RequestId,
    /// The structured error object.
    pub error: JsonRpcError,
}

impl ErrorResponse {
    /// Builds an error response with `jsonrpc` set to [`JSONRPC_VERSION`].
    pub fn new(id: impl Into<RequestId>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id: id.into(),
            error,
        }
    }
}

/// A response is either a success or an error, told apart by whether the
/// wire object carries `result` or `error`. Untagged so a client reading a
/// raw line does not have to decide first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Response {
    /// A successful method result.
    Success(SuccessResponse),
    /// A method failure.
    Error(ErrorResponse),
}

impl Response {
    /// The id both response shapes carry.
    pub fn id(&self) -> &RequestId {
        match self {
            Self::Success(r) => &r.id,
            Self::Error(r) => &r.id,
        }
    }
}

/// A server-to-client notification, `{"jsonrpc","method","params"}` with no
/// `id`. The events subsystem sends these as `events.notify` (section 8.8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notification {
    /// Always `"2.0"`.
    pub jsonrpc: JsonRpcVersion,
    /// The notification method, `"events.notify"` for events.
    pub method: String,
    /// Notification params.
    pub params: Value,
}

impl Notification {
    /// Builds a notification with `jsonrpc` set to [`JSONRPC_VERSION`].
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            method: method.into(),
            params,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_round_trips_with_string_id() {
        let req = Request::new("req-123", "system.health", json!({}));
        let s = serde_json::to_string(&req).unwrap();
        assert_eq!(
            s,
            r#"{"jsonrpc":"2.0","id":"req-123","method":"system.health","params":{}}"#
        );
        let back: Request = serde_json::from_str(&s).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn numeric_and_null_ids_are_accepted_and_echoed() {
        let req: Request = serde_json::from_value(
            json!({"jsonrpc":"2.0","id":7,"method":"system.ping","params":{}}),
        )
        .unwrap();
        assert_eq!(req.id, Some(RequestId::Number(7)));
        let resp: Response =
            serde_json::from_value(json!({"jsonrpc":"2.0","id":null,"result":{"ok":true}}))
                .unwrap();
        assert_eq!(resp.id(), &RequestId::Null);
    }

    #[test]
    fn float_ids_are_rejected() {
        let err = serde_json::from_value::<RequestId>(json!(1.5)).unwrap_err();
        assert!(err.to_string().contains("untagged"));
    }

    #[test]
    fn missing_id_is_a_notification_and_stays_absent() {
        let v = json!({"jsonrpc":"2.0","method":"events.notify","params":{}});
        let req: Request = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(req.id, None);
        assert_eq!(serde_json::to_value(&req).unwrap(), v);
    }

    #[test]
    fn other_protocol_versions_are_rejected() {
        let err = serde_json::from_value::<Request>(
            json!({"jsonrpc":"1.0","id":"1","method":"system.ping","params":{}}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("jsonrpc must be \"2.0\""), "{err}");
        assert_eq!(serde_json::to_value(JsonRpcVersion).unwrap(), json!("2.0"));
    }

    #[test]
    fn unknown_envelope_field_is_rejected_by_name() {
        let err = serde_json::from_value::<Request>(
            json!({"jsonrpc":"2.0","id":"1","method":"system.ping","params":{},"extra":1}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("extra"));
    }

    #[test]
    fn response_untagged_picks_success_or_error() {
        let ok = json!({"jsonrpc":"2.0","id":"1","result":{"ok":true}});
        assert!(matches!(
            serde_json::from_value::<Response>(ok).unwrap(),
            Response::Success(_)
        ));
        let err = json!({
            "jsonrpc":"2.0","id":"1",
            "error":{"code":-32010,"message":"bad","data":{"app_code":"INVALID_PARAMS","retryable":false,"details":{}}}
        });
        assert!(matches!(
            serde_json::from_value::<Response>(err).unwrap(),
            Response::Error(_)
        ));
    }
}
