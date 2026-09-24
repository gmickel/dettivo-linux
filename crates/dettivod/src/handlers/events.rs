//! `events.subscribe` and `events.unsubscribe`: subscriptions belong to the
//! connection that made them and end with it.

use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::events::{
    SubscribeParams, SubscribeResult, UnsubscribeParams, UnsubscribeResult,
};
use serde_json::Value;

use super::{json, params};

use crate::daemon::Daemon;
use crate::handlers::Ctx;

/// `events.subscribe`.
pub fn subscribe(daemon: &Daemon, ctx: &Ctx, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: SubscribeParams = params(params_value)?;
    if p.topics.is_empty() {
        return Err(JsonRpcError::new(
            AppCode::InvalidParams,
            "topics must name at least one topic",
            ErrorDetails::empty(),
        ));
    }
    // Each connection's channel holds 256 lines and nothing smaller is
    // enforced yet, so the granted depth is that number whatever was asked.
    let buffer = 256;
    let sink = ctx.sink(buffer);
    let id = daemon.bus().subscribe(ctx.conn, p.topics, buffer, sink);
    json(SubscribeResult {
        subscription_id: id,
        buffer,
    })
}

/// `events.unsubscribe`.
pub fn unsubscribe(daemon: &Daemon, ctx: &Ctx, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: UnsubscribeParams = params(params_value)?;
    json(UnsubscribeResult {
        unsubscribed: daemon.bus().unsubscribe(ctx.conn, &p.subscription_id),
    })
}
