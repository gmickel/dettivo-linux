//! One-line socket helpers the verbs share: a request over a fresh
//! connection, and the readiness probe a bound socket alone cannot give.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

/// One request line over a fresh connection, answered within ten seconds.
pub fn call(socket: &Path, line: &str) -> std::io::Result<Value> {
    call_within(socket, line, Duration::from_secs(10))
}

/// One request line over a fresh connection, answered within `timeout`
/// (a `polish.test` while a model loads needs more than ten seconds).
/// The budget is one deadline over the write and the read together; a
/// daemon that takes the line and never answers is `TimedOut`.
pub fn call_within(socket: &Path, line: &str, timeout: Duration) -> std::io::Result<Value> {
    let deadline = std::time::Instant::now() + timeout;
    let mut stream = UnixStream::connect(socket)?;
    stream.set_write_timeout(Some(timeout))?;
    stream.write_all(line.as_bytes())?;
    stream.write_all(b"\n")?;
    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
    if remaining.is_zero() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("the request was not written within {timeout:?}"),
        ));
    }
    stream.set_read_timeout(Some(remaining))?;
    let mut reader = BufReader::new(stream);
    let mut answer = String::new();
    match reader.read_line(&mut answer) {
        Ok(0) => Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "the daemon closed the connection without an answer",
        )),
        Ok(_) => serde_json::from_str(answer.trim_end()).map_err(std::io::Error::other),
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("no answer within {timeout:?}"),
        )),
        Err(e) => Err(e),
    }
}

/// True when the daemon on `socket` answers `system.ping` within a short
/// read timeout; a bound socket alone is not readiness.
pub fn answers_ping(socket: &Path) -> bool {
    let Ok(mut stream) = UnixStream::connect(socket) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let ping = "{\"jsonrpc\":\"2.0\",\"id\":\"ready\",\"method\":\"system.ping\",\"params\":{}}\n";
    if stream.write_all(ping.as_bytes()).is_err() {
        return false;
    }
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .is_ok_and(|n| n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// qa-rig/F17: a socket that accepts the line and never answers is a
    /// timeout at the deadline, named as one.
    #[test]
    fn a_silent_daemon_is_a_timeout_at_the_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("d.sock");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let held = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            std::thread::sleep(Duration::from_secs(3));
            drop(stream);
        });
        let started = std::time::Instant::now();
        let err = call_within(
            &path,
            "{\"jsonrpc\":\"2.0\",\"id\":\"1\",\"method\":\"system.ping\",\"params\":{}}",
            Duration::from_millis(200),
        )
        .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut, "{err}");
        assert!(err.to_string().contains("200ms"), "{err}");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        held.join().unwrap();
    }
}
