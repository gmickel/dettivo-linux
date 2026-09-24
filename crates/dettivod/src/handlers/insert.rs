//! `insert.perform`, `insert.undo`, `insert.target` (ADR 0007) and
//! `insert.allow_self_target` (ADR 0024). The chain
//! lives in `dettivo-insert`; this module maps the wire params to it
//! (literal text, or an item's final text read from the history store by
//! `source_ref`), runs it off the async runtime (Wayland, X11 and helper
//! commands block), and shapes the errors. Nothing here logs the text.

use dettivo_insert::chain::Guards;
use dettivo_insert::{Inserter, Request};
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::insert::{AllowSelfTargetParams, AllowSelfTargetResult, PerformParams};
use dettivo_proto::runtime::RefKind;
use serde_json::{Map, Value};

use super::{json, params};

use crate::daemon::{Daemon, blocking};
use crate::handlers::{Ctx, transcripts::lookup};
use crate::self_target::APP_IDS;

/// `NOT_FOUND` in the contract's shape for a `source_ref` nothing answers to.
fn not_found(kind: RefKind) -> JsonRpcError {
    let mut details = Map::new();
    details.insert(
        "kind".into(),
        serde_json::to_value(kind).unwrap_or(Value::Null),
    );
    JsonRpcError::new(
        AppCode::NotFound,
        "Transcript not found",
        ErrorDetails(details),
    )
}

/// `insert.perform`.
pub fn perform(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: PerformParams = params(params_value)?;
    let text = match (p.text, p.source_ref) {
        (Some(text), _) => text,
        (None, Some(reference)) => {
            // Nothing stores meetings yet, so a meeting reference resolves
            // to nothing; the contract's wording covers both kinds.
            if reference.kind == RefKind::Meeting {
                return Err(not_found(reference.kind));
            }
            lookup(daemon, &reference)
                .map_err(|e| match e.app_code() {
                    AppCode::NotFound => not_found(reference.kind),
                    _ => e,
                })?
                .final_text
        }
        (None, None) => {
            return Err(JsonRpcError::new(
                AppCode::InvalidParams,
                "text or source_ref is required",
                ErrorDetails::empty(),
            ));
        }
    };
    let settings = daemon.insert_settings();
    let request = Request {
        text: &text,
        mode: p.mode,
        guards: Guards {
            app_id: p.expected_target_bundle_id.as_deref(),
            pid: p.expected_target_pid.as_deref(),
            window: None,
        },
        allow_self_target: p.allow_self_target && daemon.self_target().armed(),
    };
    tracing::debug!(chars = text.chars().count(), mode = ?p.mode, "insert.perform");
    let result = blocking(|| daemon.insert.insert(&settings, &request));
    match result {
        Ok(result) => json(result),
        Err(message) => Err(JsonRpcError::new(
            AppCode::Conflict,
            message,
            ErrorDetails::empty(),
        )),
    }
}

/// `insert.allow_self_target`: arms the allowance on this connection
/// when the peer is the app (or the daemon runs in QA mode), disarms it
/// otherwise; a peer that is not the app is `CONFLICT` with kind
/// `notTheApp`.
pub fn allow_self_target(
    daemon: &Daemon,
    ctx: &Ctx,
    params_value: Value,
) -> Result<Value, JsonRpcError> {
    let p: AllowSelfTargetParams = params(params_value)?;
    if p.enabled {
        daemon
            .self_target()
            .arm(ctx.conn, ctx.peer_pid, daemon.qa_mode())
            .map_err(|e| {
                JsonRpcError::new(
                    AppCode::Conflict,
                    format!(
                        "only dettivo-app may allow insertion into itself: {}",
                        e.detail
                    ),
                    ErrorDetails::conflict_kind("notTheApp"),
                )
            })?;
    } else {
        daemon.self_target().disarm(ctx.conn);
    }
    json(AllowSelfTargetResult {
        enabled: daemon.self_target().armed(),
        app_ids: APP_IDS.iter().map(|s| s.to_string()).collect(),
    })
}

/// `insert.undo`.
pub fn undo(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let settings = daemon.insert_settings();
    json(blocking(|| daemon.insert.undo(&settings)))
}

/// `insert.target`.
pub fn target(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let settings = daemon.insert_settings();
    json(blocking(|| daemon.insert.target(&settings)))
}
