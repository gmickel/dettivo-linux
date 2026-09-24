//! `transfer.begin`, `chunk`, `pull`, `commit` and `cancel`: thin wrappers
//! over the transfer service.

use dettivo_proto::error::JsonRpcError;
use dettivo_proto::methods::transfer::{
    BeginParams, CancelParams, ChunkParams, CommitParams, PullParams,
};
use serde_json::Value;

use super::{json, params};

use crate::daemon::Daemon;

/// `transfer.begin`.
pub fn begin(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: BeginParams = params(params_value)?;
    json(
        daemon
            .transfers()
            .begin(p.direction, &p.content_type, p.size_hint)?,
    )
}

/// `transfer.chunk`.
pub fn chunk(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: ChunkParams = params(params_value)?;
    json(
        daemon
            .transfers()
            .chunk(&p.transfer_id, p.seq, &p.data_b64)?,
    )
}

/// `transfer.pull`.
pub fn pull(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: PullParams = params(params_value)?;
    json(daemon.transfers().pull(&p.transfer_id, p.seq)?)
}

/// `transfer.commit`.
pub fn commit(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: CommitParams = params(params_value)?;
    json(
        daemon
            .transfers()
            .commit(&p.transfer_id, p.total_chunks, &p.sha256)?,
    )
}

/// `transfer.cancel`.
pub fn cancel(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: CancelParams = params(params_value)?;
    json(daemon.transfers().cancel(&p.transfer_id, &p.reason)?)
}
