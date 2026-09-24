//! The error taxonomy: the seven server app codes with their numeric
//! JSON-RPC codes, the adapter-only `APP_NOT_RUNNING` code, and the `data`
//! block every error response carries (`docs/api/dettivo-ipc-v1.md`
//! sections 4.3 and 5).
//!
//! The macOS server assigns `-32010` through `-32015` to `INVALID_PARAMS`,
//! `UNAUTHORIZED_CLIENT`, `NOT_FOUND`, `CONFLICT`, `NOT_IMPLEMENTED` and
//! `INTERNAL_ERROR`. It signals local backpressure through the
//! `events.overflow` topic and never emits `RATE_LIMITED_LOCAL` on the wire,
//! and `APP_NOT_RUNNING` is raised inside adapters that could not reach the
//! daemon at all. Linux continues the same block so both codes have a wire
//! form when an adapter or the daemon does emit them: `RATE_LIMITED_LOCAL`
//! is `-32016` and `APP_NOT_RUNNING` is `-32017`. Both assignments are
//! registered in `docs/api/linux-deltas.md`.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

/// An application-level error code from the taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppCode {
    /// Bad or missing params. Not retryable.
    InvalidParams,
    /// Peer, token or security policy failed. Not retryable.
    UnauthorizedClient,
    /// Resource not found. Not retryable.
    NotFound,
    /// Invalid current state. Sometimes retryable.
    Conflict,
    /// Reserved capability or method not enabled. Not retryable.
    NotImplemented,
    /// Local backpressure. Retryable.
    RateLimitedLocal,
    /// Unexpected server failure. Retryability is case by case.
    InternalError,
    /// Adapter-only: the CLI, MCP server or REST shim could not reach the
    /// daemon. The daemon itself never emits it. Retryable.
    AppNotRunning,
}

impl AppCode {
    /// Every code, in taxonomy order.
    pub const ALL: &'static [AppCode] = &[
        Self::InvalidParams,
        Self::UnauthorizedClient,
        Self::NotFound,
        Self::Conflict,
        Self::NotImplemented,
        Self::RateLimitedLocal,
        Self::InternalError,
        Self::AppNotRunning,
    ];

    /// The seven codes a server may emit.
    pub const SERVER: &'static [AppCode] = &[
        Self::InvalidParams,
        Self::UnauthorizedClient,
        Self::NotFound,
        Self::Conflict,
        Self::NotImplemented,
        Self::RateLimitedLocal,
        Self::InternalError,
    ];

    /// The numeric `error.code` this app code travels with.
    pub fn rpc_code(self) -> i32 {
        match self {
            Self::InvalidParams => -32010,
            Self::UnauthorizedClient => -32011,
            Self::NotFound => -32012,
            Self::Conflict => -32013,
            Self::NotImplemented => -32014,
            Self::InternalError => -32015,
            Self::RateLimitedLocal => -32016,
            Self::AppNotRunning => -32017,
        }
    }

    /// The app code a numeric `error.code` belongs to, if any.
    pub fn from_rpc_code(code: i32) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.rpc_code() == code)
    }

    /// The wire spelling, for example `"INVALID_PARAMS"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidParams => "INVALID_PARAMS",
            Self::UnauthorizedClient => "UNAUTHORIZED_CLIENT",
            Self::NotFound => "NOT_FOUND",
            Self::Conflict => "CONFLICT",
            Self::NotImplemented => "NOT_IMPLEMENTED",
            Self::RateLimitedLocal => "RATE_LIMITED_LOCAL",
            Self::InternalError => "INTERNAL_ERROR",
            Self::AppNotRunning => "APP_NOT_RUNNING",
        }
    }

    /// Whether a client may usefully retry without changing anything.
    /// `Conflict` and `InternalError` are decided per failure, so their
    /// default is `false`; a server that knows better sets `retryable`
    /// explicitly through [`ErrorData::with_retryable`].
    pub fn default_retryable(self) -> bool {
        matches!(self, Self::RateLimitedLocal | Self::AppNotRunning)
    }

    /// Whether this code may appear in a response from the daemon.
    pub fn is_server_code(self) -> bool {
        !matches!(self, Self::AppNotRunning)
    }
}

/// The `error.data.details` object. The macOS server always sends an
/// object here, empty when it has nothing to add; when it does add
/// something the keys are stable (section 4.3 documents the runtime
/// mapping, section 8.4 documents `kind` for `CONFLICT`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ErrorDetails(pub Map<String, Value>);

impl ErrorDetails {
    /// Domain the runtime mapping keys use.
    pub const DOMAIN_KEY: &'static str = "domain";
    /// Runtime error code key.
    pub const CODE_KEY: &'static str = "dettivo_error_code";
    /// Runtime error kind key.
    pub const KIND_FIELD_KEY: &'static str = "dettivo_error_kind";
    /// The `CONFLICT` detail-kind key (`importAlreadyInProgress` and the
    /// like).
    pub const CONFLICT_KIND_KEY: &'static str = "kind";

    /// No details.
    pub fn empty() -> Self {
        Self::default()
    }

    /// The optional runtime-error mapping documented in section 4.3.
    pub fn runtime(domain: &str, code: i64, kind: &str) -> Self {
        let mut map = Map::new();
        map.insert(Self::DOMAIN_KEY.into(), Value::String(domain.into()));
        map.insert(Self::CODE_KEY.into(), Value::from(code));
        map.insert(Self::KIND_FIELD_KEY.into(), Value::String(kind.into()));
        Self(map)
    }

    /// A `CONFLICT` detail kind, for example `"importAlreadyInProgress"`.
    pub fn conflict_kind(kind: &str) -> Self {
        let mut map = Map::new();
        map.insert(Self::CONFLICT_KIND_KEY.into(), Value::String(kind.into()));
        Self(map)
    }

    /// The `CONFLICT` detail kind when one is present.
    pub fn kind(&self) -> Option<&str> {
        self.0.get(Self::CONFLICT_KIND_KEY).and_then(Value::as_str)
    }

    /// Whether the object carries anything.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The `error.data` block attached to every JSON-RPC error response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorData {
    /// The application-level code.
    pub app_code: AppCode,
    /// Whether the caller may usefully retry.
    pub retryable: bool,
    /// Structured details; an empty object when there are none.
    pub details: ErrorDetails,
}

impl ErrorData {
    /// The `data` block for an app code with its default retryability.
    pub fn new(app_code: AppCode, details: ErrorDetails) -> Self {
        Self {
            app_code,
            retryable: app_code.default_retryable(),
            details,
        }
    }

    /// The `data` block with an explicit retryability, for the codes whose
    /// answer depends on the failure (`CONFLICT`, `INTERNAL_ERROR`).
    pub fn with_retryable(app_code: AppCode, retryable: bool, details: ErrorDetails) -> Self {
        Self {
            app_code,
            retryable,
            details,
        }
    }
}

/// The full JSON-RPC `error` object: `code`, `message` and `data`.
///
/// Deserialisation checks that `code` is the number the taxonomy assigns
/// to `data.app_code`; any other pairing, and any code outside the
/// taxonomy, is rejected with the offending code named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JsonRpcError {
    /// Numeric JSON-RPC error code.
    pub code: i32,
    /// Human-readable message.
    pub message: String,
    /// The structured `data` block.
    pub data: ErrorData,
}

impl JsonRpcError {
    /// An error for an app code, with the matching numeric code.
    pub fn new(app_code: AppCode, message: impl Into<String>, details: ErrorDetails) -> Self {
        Self {
            code: app_code.rpc_code(),
            message: message.into(),
            data: ErrorData::new(app_code, details),
        }
    }

    /// An error from a fully built `data` block.
    pub fn with_data(message: impl Into<String>, data: ErrorData) -> Self {
        Self {
            code: data.app_code.rpc_code(),
            message: message.into(),
            data,
        }
    }

    /// The `NOT_IMPLEMENTED` error a router answers a reserved method with.
    pub fn not_implemented(method: &str) -> Self {
        Self::new(
            AppCode::NotImplemented,
            format!("{method} is not implemented"),
            ErrorDetails::empty(),
        )
    }

    /// The app code, as carried in `data`.
    pub fn app_code(&self) -> AppCode {
        self.data.app_code
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonRpcErrorWire {
    code: i32,
    message: String,
    data: ErrorData,
}

impl<'de> Deserialize<'de> for JsonRpcError {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = JsonRpcErrorWire::deserialize(deserializer)?;
        let expected = wire.data.app_code.rpc_code();
        if wire.code != expected {
            return Err(D::Error::custom(format!(
                "error code {} is not the code the taxonomy assigns to {} ({expected})",
                wire.code,
                wire.data.app_code.as_str()
            )));
        }
        Ok(Self {
            code: wire.code,
            message: wire.message,
            data: wire.data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numeric_codes_match_the_contract_block() {
        let pairs = [
            (AppCode::InvalidParams, -32010),
            (AppCode::UnauthorizedClient, -32011),
            (AppCode::NotFound, -32012),
            (AppCode::Conflict, -32013),
            (AppCode::NotImplemented, -32014),
            (AppCode::InternalError, -32015),
            (AppCode::RateLimitedLocal, -32016),
            (AppCode::AppNotRunning, -32017),
        ];
        for (code, number) in pairs {
            assert_eq!(code.rpc_code(), number);
            assert_eq!(AppCode::from_rpc_code(number), Some(code));
        }
        assert_eq!(AppCode::from_rpc_code(-32000), None);
    }

    #[test]
    fn app_code_wire_spelling_matches_serde_rename() {
        for code in AppCode::ALL {
            let json = serde_json::to_value(code).unwrap();
            assert_eq!(json, Value::String(code.as_str().to_string()));
        }
    }

    #[test]
    fn error_round_trips_with_empty_details_object() {
        let err = JsonRpcError::new(
            AppCode::InvalidParams,
            "Invalid params",
            ErrorDetails::empty(),
        );
        let v = serde_json::to_value(&err).unwrap();
        assert_eq!(
            v,
            json!({"code":-32010,"message":"Invalid params","data":{"app_code":"INVALID_PARAMS","retryable":false,"details":{}}})
        );
        let back: JsonRpcError = serde_json::from_value(v).unwrap();
        assert_eq!(back, err);
    }

    #[test]
    fn runtime_mapping_and_conflict_kind_are_typed_helpers() {
        let details = ErrorDetails::runtime("dettivo_error", 401, "accessibilityPermissionDenied");
        assert_eq!(
            serde_json::to_value(&details).unwrap(),
            json!({"domain":"dettivo_error","dettivo_error_code":401,"dettivo_error_kind":"accessibilityPermissionDenied"})
        );
        let conflict = ErrorDetails::conflict_kind("importAlreadyInProgress");
        assert_eq!(conflict.kind(), Some("importAlreadyInProgress"));
    }

    #[test]
    fn mismatched_or_unknown_codes_are_rejected() {
        let mismatch = json!({"code":-32011,"message":"x","data":{"app_code":"NOT_FOUND","retryable":false,"details":{}}});
        let err = serde_json::from_value::<JsonRpcError>(mismatch).unwrap_err();
        assert!(err.to_string().contains("-32011"));
        let unknown = json!({"code":-32010,"message":"x","data":{"app_code":"SOMETHING_ELSE","retryable":false,"details":{}}});
        let err = serde_json::from_value::<JsonRpcError>(unknown).unwrap_err();
        assert!(err.to_string().contains("SOMETHING_ELSE"));
    }

    #[test]
    fn retryability_defaults_follow_the_taxonomy_table() {
        assert!(AppCode::RateLimitedLocal.default_retryable());
        assert!(AppCode::AppNotRunning.default_retryable());
        assert!(!AppCode::Conflict.default_retryable());
        let data = ErrorData::with_retryable(AppCode::Conflict, true, ErrorDetails::empty());
        assert!(data.retryable);
        assert!(!AppCode::AppNotRunning.is_server_code());
    }
}
