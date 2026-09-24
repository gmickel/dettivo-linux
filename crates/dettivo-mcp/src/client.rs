//! The daemon client: one connection per request over the Unix socket,
//! one request line, one response line, with the socket and token
//! resolved the way the `dettivo` command resolves them (`--socket`,
//! `DETTIVO_IPC_SOCKET`, the default; `--token`, `--token-file`,
//! `DETTIVO_IPC_TOKEN`). Failures keep the class an agent needs to act
//! on: unreachable, refused, timed out, or the daemon's own error.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dettivo_proto::error::{AppCode, JsonRpcError};
use serde_json::Value;

/// The daemon's socket unit, named when it is not running.
pub const SOCKET_UNIT: &str = "dettivod.socket";

/// Why a call failed.
#[derive(Debug)]
pub enum ClientError {
    /// The daemon is not reachable on the socket.
    Unavailable(String),
    /// The socket refused the peer, or the daemon refused the token.
    PermissionDenied(String),
    /// No answer within the timeout.
    Timeout(String),
    /// The daemon answered with an error.
    Rpc(JsonRpcError),
    /// The answer could not be read.
    Protocol(String),
}

impl ClientError {
    /// The message an agent sees before the action line.
    pub fn message(&self) -> String {
        match self {
            Self::Unavailable(m)
            | Self::PermissionDenied(m)
            | Self::Timeout(m)
            | Self::Protocol(m) => m.clone(),
            Self::Rpc(e) => format!("{} ({})", e.message, e.app_code().as_str()),
        }
    }

    /// The daemon's app code when the daemon answered.
    pub fn app_code(&self) -> Option<AppCode> {
        match self {
            Self::Rpc(e) => Some(e.app_code()),
            _ => None,
        }
    }
}

/// Connection settings.
#[derive(Debug, Clone)]
pub struct Client {
    /// The socket to connect to.
    pub socket: PathBuf,
    /// The shared token to send, when one is set.
    pub token: Option<String>,
    /// Per-request timeout.
    pub timeout: Duration,
}

/// The socket: `flag`, then `DETTIVO_IPC_SOCKET`, then
/// `$XDG_RUNTIME_DIR/dettivo/dettivo.sock` (or `/tmp/dettivo-$USER`).
pub fn socket_path(flag: Option<&Path>, env: impl Fn(&str) -> Option<OsString>) -> PathBuf {
    if let Some(p) = flag {
        return p.to_path_buf();
    }
    if let Some(v) = env("DETTIVO_IPC_SOCKET").filter(|v| !v.is_empty()) {
        return PathBuf::from(v);
    }
    match env("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        Some(v) => PathBuf::from(v).join("dettivo").join("dettivo.sock"),
        None => {
            let user = env("USER")
                .and_then(|u| u.into_string().ok())
                .unwrap_or_else(|| "user".into());
            PathBuf::from(format!("/tmp/dettivo-{user}")).join("dettivo.sock")
        }
    }
}

impl Client {
    /// Resolves the socket and the token from flags and the environment.
    pub fn resolve(
        socket: Option<&Path>,
        token: Option<&str>,
        token_file: Option<&Path>,
        timeout_ms: u64,
    ) -> Result<Self, ClientError> {
        let token = if let Some(t) = token {
            Some(t.to_string())
        } else if let Some(path) = token_file {
            let text = std::fs::read_to_string(path).map_err(|e| {
                ClientError::Protocol(format!("cannot read token file {}: {e}", path.display()))
            })?;
            Some(text.trim().to_string())
        } else {
            std::env::var("DETTIVO_IPC_TOKEN")
                .ok()
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
        };
        Ok(Self {
            socket: socket_path(socket, |k| std::env::var_os(k)),
            token,
            timeout: Duration::from_millis(timeout_ms.max(1)),
        })
    }

    /// Sends one request and returns its result.
    pub fn call(&self, method: &str, params: Value) -> Result<Value, ClientError> {
        dettivo_proto::transport::call(
            &self.socket,
            self.token.as_deref(),
            self.timeout,
            "1",
            method,
            params,
        )
        .map_err(|e| {
            use dettivo_proto::transport::Error;
            match e {
                Error::Connect(e) => self.connect_failure(&e),
                Error::Setup(e) => ClientError::Protocol(format!("socket setup: {e}")),
                Error::Encode(e) => ClientError::Protocol(format!("cannot encode request: {e}")),
                Error::Io(stage, e) => self.io_failure(stage, &e),
                Error::Closed => ClientError::Unavailable(format!(
                    "IPC server closed the connection (socket: {})",
                    self.socket.display()
                )),
                Error::Protocol(e) => ClientError::Protocol(format!("unreadable response: {e}")),
                Error::Rpc(e) if e.app_code() == AppCode::UnauthorizedClient => {
                    ClientError::PermissionDenied(format!("{} (UNAUTHORIZED_CLIENT)", e.message))
                }
                Error::Rpc(e) => ClientError::Rpc(e),
            }
        })
    }

    fn connect_failure(&self, e: &io::Error) -> ClientError {
        match e.kind() {
            io::ErrorKind::PermissionDenied => ClientError::PermissionDenied(format!(
                "Permission denied connecting to IPC socket (socket: {})",
                self.socket.display()
            )),
            _ => ClientError::Unavailable(format!(
                "Dettivo daemon/IPC unavailable (socket: {}, {e})",
                self.socket.display()
            )),
        }
    }

    fn io_failure(&self, what: &str, e: &io::Error) -> ClientError {
        match e.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => ClientError::Timeout(format!(
                "IPC {what} timed out after {} ms (socket: {})",
                self.timeout.as_millis(),
                self.socket.display()
            )),
            _ => ClientError::Unavailable(format!(
                "IPC {what} failed (socket: {}): {e}",
                self.socket.display()
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_resolution_order() {
        let env = |k: &str| match k {
            "DETTIVO_IPC_SOCKET" => Some(OsString::from("/tmp/e.sock")),
            "XDG_RUNTIME_DIR" => Some(OsString::from("/run/user/7")),
            _ => None,
        };
        assert_eq!(
            socket_path(Some(Path::new("/f.sock")), env),
            PathBuf::from("/f.sock")
        );
        assert_eq!(socket_path(None, env), PathBuf::from("/tmp/e.sock"));
        let env = |k: &str| (k == "XDG_RUNTIME_DIR").then(|| OsString::from("/run/user/7"));
        assert_eq!(
            socket_path(None, env),
            PathBuf::from("/run/user/7/dettivo/dettivo.sock")
        );
    }

    #[test]
    fn an_absent_socket_is_unavailable_with_the_path() {
        let client = Client {
            socket: PathBuf::from("/nonexistent/dettivo.sock"),
            token: None,
            timeout: Duration::from_millis(100),
        };
        match client.call("system.ping", serde_json::json!({})) {
            Err(ClientError::Unavailable(m)) => {
                assert!(m.contains("/nonexistent/dettivo.sock"), "{m}")
            }
            other => panic!("{other:?}"),
        }
    }
}
