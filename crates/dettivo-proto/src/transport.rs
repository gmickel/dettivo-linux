//! One bounded Unix-socket transport shared by the CLI, MCP and REST adapters.
use crate::envelope::{Request, Response};
use crate::error::JsonRpcError;
use serde_json::Value;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};
#[path = "transport_connect.rs"]
mod connect;

/// Maximum encoded response or event line, including its newline.
pub const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// Transport stages remain typed so adapters retain their own error contracts.
#[derive(Debug)]
pub enum Error {
    /// Opening the socket failed.
    Connect(io::Error),
    /// Configuring the socket failed.
    Setup(io::Error),
    /// Encoding the request failed.
    Encode(serde_json::Error),
    /// Reading or writing failed, including an expired deadline.
    Io(&'static str, io::Error),
    /// The peer closed before responding.
    Closed,
    /// The envelope is malformed or too large.
    Protocol(String),
    /// The daemon returned a JSON-RPC error.
    Rpc(JsonRpcError),
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "request deadline expired"))
}

/// A bounded event reader. Partial lines survive polling timeouts.
pub struct Reader {
    reader: BufReader<UnixStream>,
    partial: Vec<u8>,
    timeout: Option<Duration>,
}
impl Reader {
    /// Sets the timeout for each subsequent event read; `None` waits for events.
    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
    }

    /// Reads one complete bounded event line, retaining an incomplete line on timeout.
    pub fn read_line(&mut self, out: &mut String) -> io::Result<usize> {
        self.read_until(out, self.timeout.map(|d| Instant::now() + d))
    }

    fn read_until(&mut self, out: &mut String, deadline: Option<Instant>) -> io::Result<usize> {
        loop {
            self.reader
                .get_ref()
                .set_read_timeout(deadline.map(remaining).transpose()?)?;
            let available = self.reader.fill_buf()?;
            if available.is_empty() {
                if self.partial.is_empty() {
                    return Ok(0);
                }
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "connection closed mid-line",
                ));
            }
            let newline = available.iter().position(|b| *b == b'\n');
            let n = newline.map_or(available.len(), |p| p + 1);
            if self.partial.len() + n > MAX_RESPONSE_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("response exceeds {MAX_RESPONSE_BYTES} bytes"),
                ));
            }
            self.partial.extend_from_slice(&available[..n]);
            self.reader.consume(n);
            if newline.is_some() {
                let line = std::str::from_utf8(&self.partial)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                out.push_str(line);
                let n = self.partial.len();
                self.partial.clear();
                return Ok(n);
            }
        }
    }
}

/// Sends one request and consumes its acknowledgment under one absolute deadline.
pub fn request(
    socket: &Path,
    token: Option<&str>,
    timeout: Duration,
    id: &str,
    method: &str,
    params: Value,
) -> Result<(Value, Reader), Error> {
    let deadline = Instant::now() + timeout;
    let mut stream = connect::connect(socket, deadline).map_err(|e| {
        if matches!(
            e.kind(),
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
        ) {
            Error::Io("connect", e)
        } else {
            Error::Connect(e)
        }
    })?;
    let mut request =
        serde_json::to_value(Request::new(id, method, params)).map_err(Error::Encode)?;
    if let Some(token) = token {
        request["auth_token"] = Value::String(token.into());
    }
    let mut bytes = serde_json::to_vec(&request).map_err(Error::Encode)?;
    bytes.push(b'\n');
    let mut offset = 0;
    while offset < bytes.len() {
        stream
            .set_write_timeout(Some(
                remaining(deadline).map_err(|e| Error::Io("write", e))?,
            ))
            .map_err(Error::Setup)?;
        let n = stream
            .write(&bytes[offset..])
            .map_err(|e| Error::Io("write", e))?;
        if n == 0 {
            return Err(Error::Io("write", io::ErrorKind::WriteZero.into()));
        }
        offset += n;
    }
    let mut reader = Reader {
        reader: BufReader::new(stream),
        partial: Vec::new(),
        timeout: None,
    };
    let mut answer = String::new();
    let n = reader
        .read_until(&mut answer, Some(deadline))
        .map_err(|e| {
            if matches!(
                e.kind(),
                io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof
            ) {
                Error::Protocol(e.to_string())
            } else {
                Error::Io("read", e)
            }
        })?;
    if n == 0 {
        return Err(Error::Closed);
    }
    let response: Response =
        serde_json::from_str(answer.trim_end()).map_err(|e| Error::Protocol(e.to_string()))?;
    match response {
        Response::Success(s) => Ok((s.result, reader)),
        Response::Error(e) => Err(Error::Rpc(e.error)),
    }
}

/// Sends one request and returns its result, closing the connection afterwards.
pub fn call(
    socket: &Path,
    token: Option<&str>,
    timeout: Duration,
    id: &str,
    method: &str,
    params: Value,
) -> Result<Value, Error> {
    request(socket, token, timeout, id, method, params).map(|(value, _)| value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    #[test]
    fn closed_malformed_oversized_and_delayed_ack_are_distinct_failures() {
        for case in ["eof", "partial", "malformed", "oversized", "delayed"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("ipc");
            let listener = UnixListener::bind(&path).unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut line)
                    .unwrap();
                match case {
                    "partial" => {
                        let _ = stream.write_all(b"{");
                    }
                    "malformed" => {
                        let _ = stream.write_all(b"not JSON\n");
                    }
                    "oversized" => {
                        let _ = stream.write_all(&vec![b'x'; MAX_RESPONSE_BYTES + 1]);
                    }
                    "delayed" => {
                        std::thread::sleep(Duration::from_millis(100));
                        let _ =
                            stream.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":\"1\",\"result\":{}}\n");
                    }
                    _ => {}
                }
            });
            let timeout = if case == "delayed" {
                Duration::from_millis(20)
            } else {
                Duration::from_secs(2)
            };
            let result = request(
                &path,
                None,
                timeout,
                "1",
                "events.subscribe",
                serde_json::json!({}),
            );
            server.join().unwrap();
            match (case, result) {
                ("eof", Err(Error::Closed))
                | ("partial" | "malformed" | "oversized", Err(Error::Protocol(_))) => {}
                ("delayed", Err(Error::Io("read", e)))
                    if matches!(
                        e.kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    ) => {}
                _ => panic!("unexpected transport result for {case}"),
            }
        }
    }
}
