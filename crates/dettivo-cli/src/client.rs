//! A blocking JSON-RPC client over the Unix socket: one request line, one
//! response line, with the connection failures mapped to the contract's
//! exit codes.

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;

use crate::Cli;
use crate::exit::{Exit, Failure};

/// The daemon's socket unit, named in the "unavailable" message so a user
/// knows what to start.
pub const SOCKET_UNIT: &str = "dettivod.socket";

/// Connection settings resolved from flags and environment.
#[derive(Debug, Clone)]
pub struct Client {
    /// The socket to connect to.
    pub socket: PathBuf,
    /// The shared token to send, when one was given.
    pub token: Option<String>,
    /// Per-request timeout.
    pub timeout: Duration,
}

impl Client {
    /// Resolves the socket (`--socket`, `DETTIVO_IPC_SOCKET`, default) and
    /// the token (`--token`, `--token-file`, `DETTIVO_IPC_TOKEN`).
    pub fn from_cli(cli: &Cli) -> Result<Self, Failure> {
        let env = |k: &str| std::env::var_os(k);
        let socket = crate::paths::socket(cli.socket.as_ref(), env);
        let token = if let Some(t) = &cli.token {
            Some(t.clone())
        } else if let Some(path) = &cli.token_file {
            let text = std::fs::read_to_string(path).map_err(|e| {
                Failure::new(
                    Exit::InvalidArgs,
                    format!("cannot read token file {}: {e}", path.display()),
                )
            })?;
            Some(text.trim().to_string())
        } else {
            std::env::var("DETTIVO_IPC_TOKEN")
                .ok()
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
        };
        Ok(Self {
            socket,
            token,
            timeout: Duration::from_millis(cli.timeout_ms.max(1)),
        })
    }

    /// Sends one request and returns its result.
    pub fn call(&self, method: &str, params: Value) -> Result<Value, Failure> {
        dettivo_proto::transport::call(
            &self.socket,
            self.token.as_deref(),
            self.timeout,
            "1",
            method,
            params,
        )
        .map_err(|e| self.transport_failure(e))
    }

    /// Reads bounded events after checking the initial response within the request deadline.
    pub fn stream(
        &self,
        method: &str,
        params: Value,
        mut on_line: impl FnMut(&str) -> bool,
    ) -> Result<(), Failure> {
        let (_, mut reader) = dettivo_proto::transport::request(
            &self.socket,
            self.token.as_deref(),
            self.timeout,
            "1",
            method,
            params,
        )
        .map_err(|e| self.transport_failure(e))?;
        loop {
            let mut line = String::new();
            let n = reader
                .read_line(&mut line)
                .map_err(|e| self.io_failure("read", &e))?;
            if n == 0 || !on_line(line.trim_end()) {
                return Ok(());
            }
        }
    }

    /// Opens a subscription; the acknowledgment uses the request timeout, events the polling timeout.
    pub fn subscribe(
        &self,
        topics: &[&str],
        read_timeout: Duration,
    ) -> Result<dettivo_proto::transport::Reader, Failure> {
        let (_, mut reader) = dettivo_proto::transport::request(
            &self.socket,
            self.token.as_deref(),
            self.timeout,
            "1",
            "events.subscribe",
            serde_json::json!({"topics": topics, "buffer": 256}),
        )
        .map_err(|e| self.transport_failure(e))?;
        reader.set_timeout(Some(read_timeout));
        Ok(reader)
    }

    fn transport_failure(&self, error: dettivo_proto::transport::Error) -> Failure {
        use dettivo_proto::transport::Error;
        match error {
            Error::Connect(e) => self.connect_failure(&e),
            Error::Setup(e) => Failure::new(Exit::Failure, format!("socket setup: {e}")),
            Error::Encode(e) => Failure::new(Exit::Failure, format!("cannot encode request: {e}")),
            Error::Io(stage, e) => self.io_failure(stage, &e),
            Error::Closed => Failure::new(
                Exit::Unavailable,
                format!(
                    "the daemon closed the connection on {}",
                    self.socket.display()
                ),
            ),
            Error::Protocol(e) => Failure::new(Exit::Failure, format!("unreadable response: {e}")),
            Error::Rpc(e) => Failure::rpc(e),
        }
    }

    fn connect_failure(&self, e: &io::Error) -> Failure {
        match e.kind() {
            io::ErrorKind::PermissionDenied => Failure::new(
                Exit::PermissionDenied,
                format!("permission denied on {}", self.socket.display()),
            ),
            _ => Failure::new(
                Exit::Unavailable,
                format!(
                    "the daemon is not reachable at {} ({e}); start it with `systemctl --user start {SOCKET_UNIT}` or run dettivod",
                    self.socket.display()
                ),
            ),
        }
    }

    fn io_failure(&self, what: &str, e: &io::Error) -> Failure {
        match e.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => Failure::new(
                Exit::Timeout,
                format!(
                    "timed out after {} ms waiting to {what} on {}",
                    self.timeout.as_millis(),
                    self.socket.display()
                ),
            ),
            _ => Failure::new(
                Exit::Unavailable,
                format!("{what} on {} failed: {e}", self.socket.display()),
            ),
        }
    }
}

#[cfg(test)]
mod deadline_regressions {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    #[test]
    fn slowly_delivered_response_obeys_total_request_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ipc");
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut request)
                .unwrap();
            for byte in b"{\"jsonrpc\":\"2.0\",\"id\":\"1\",\"result\":{}}\n" {
                if stream.write_all(&[*byte]).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        let result = Client {
            socket: path,
            token: None,
            timeout: Duration::from_millis(30),
        }
        .call("system.ping", serde_json::json!({}));
        server.join().unwrap();
        assert!(
            matches!(
                result,
                Err(Failure {
                    exit: Exit::Timeout,
                    ..
                })
            ),
            "{result:?}"
        );
    }
}
