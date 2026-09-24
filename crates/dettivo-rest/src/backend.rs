//! Where a route's call goes: the daemon over its Unix socket (one
//! connection per request, the way the CLI and the MCP server call it),
//! or the daemon's own router when the daemon hosts the shim. A daemon
//! that cannot be reached is the adapter-only `APP_NOT_RUNNING` (`503`)
//! with the systemd hint.

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use serde_json::Value;

/// The daemon's socket unit, named when it is not running.
pub const SOCKET_UNIT: &str = "dettivod.socket";

/// One contract call.
pub trait Backend: Send + Sync {
    /// Calls `method` with validated-by-the-daemon `params`.
    fn call(&self, method: &str, params: Value) -> Result<Value, JsonRpcError>;

    /// Upload size constrained by the daemon's raw and IPC limits.
    fn upload_chunk_bytes(&self, transfer_id: &str, raw_limit: u64) -> Result<usize, JsonRpcError> {
        upload_chunk_bytes(self, transfer_id, raw_limit, None)
    }
}

fn upload_chunk_bytes<B: Backend + ?Sized>(
    backend: &B,
    transfer_id: &str,
    raw_limit: u64,
    token: Option<&str>,
) -> Result<usize, JsonRpcError> {
    let config = backend.call(
        "config.get",
        serde_json::json!({"key":"ipc.max_line_bytes"}),
    )?;
    let cap = config["entries"][0]["value"]
        .as_u64()
        .ok_or_else(|| internal("config.get did not return ipc.max_line_bytes".into()))?;
    dettivo_proto::upload::chunk_bytes(raw_limit, cap, "rest", transfer_id, token)
        .ok_or_else(|| internal("upload limits cannot fit a transfer.chunk request".into()))
}

/// The daemon over its Unix socket.
#[derive(Debug, Clone)]
pub struct SocketBackend {
    /// The socket to connect to.
    pub socket: PathBuf,
    /// The IPC token to send in `peer_token` mode, when one is set.
    pub token: Option<String>,
    /// Per-request timeout.
    pub timeout: Duration,
}

/// The `APP_NOT_RUNNING` error for a daemon that did not answer.
pub fn app_not_running(message: String) -> JsonRpcError {
    JsonRpcError::new(AppCode::AppNotRunning, message, ErrorDetails::empty())
}

fn internal(message: String) -> JsonRpcError {
    JsonRpcError::new(AppCode::InternalError, message, ErrorDetails::empty())
}

impl SocketBackend {
    fn connect_failure(&self, e: &io::Error) -> JsonRpcError {
        app_not_running(format!(
            "the daemon is not reachable at {} ({e}); start it with `systemctl --user start {SOCKET_UNIT}` or run dettivod",
            self.socket.display()
        ))
    }

    fn io_failure(&self, what: &str, e: &io::Error) -> JsonRpcError {
        match e.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => app_not_running(format!(
                "timed out after {} ms waiting to {what} on {}",
                self.timeout.as_millis(),
                self.socket.display()
            )),
            _ => app_not_running(format!("{what} on {} failed: {e}", self.socket.display())),
        }
    }
}

impl Backend for SocketBackend {
    fn upload_chunk_bytes(&self, transfer_id: &str, raw_limit: u64) -> Result<usize, JsonRpcError> {
        upload_chunk_bytes(self, transfer_id, raw_limit, self.token.as_deref())
    }

    fn call(&self, method: &str, params: Value) -> Result<Value, JsonRpcError> {
        dettivo_proto::transport::call(
            &self.socket,
            self.token.as_deref(),
            self.timeout,
            "rest",
            method,
            params,
        )
        .map_err(|e| {
            use dettivo_proto::transport::Error;
            match e {
                Error::Connect(e) => self.connect_failure(&e),
                Error::Setup(e) => internal(format!("socket setup: {e}")),
                Error::Encode(e) => internal(format!("cannot encode request: {e}")),
                Error::Io(stage, e) => self.io_failure(stage, &e),
                Error::Closed => app_not_running(format!(
                    "the daemon closed the connection on {}",
                    self.socket.display()
                )),
                Error::Protocol(e) => internal(format!("unreadable daemon response: {e}")),
                Error::Rpc(e) => e,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_socket_is_app_not_running_with_the_hint() {
        let backend = SocketBackend {
            socket: PathBuf::from("/nonexistent/dettivo.sock"),
            token: None,
            timeout: Duration::from_millis(100),
        };
        let err = backend
            .call("system.ping", serde_json::json!({}))
            .unwrap_err();
        assert_eq!(err.app_code(), AppCode::AppNotRunning);
        assert!(err.message.contains(SOCKET_UNIT), "{}", err.message);
        assert!(err.message.contains("/nonexistent/dettivo.sock"));
    }
}
