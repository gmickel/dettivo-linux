//! A small blocking HTTP/1.1 client for the harness and `dettivo rest
//! status`: one request over one connection, the response read by its
//! `Content-Length`. Enough to replay a fixture; not a general client.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// One response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// The status code.
    pub status: u16,
    /// Header names lower-cased, in order.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Vec<u8>,
}

impl Reply {
    /// The first header with `name` (lower-case).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// The body as JSON, when it is.
    pub fn json(&self) -> Option<serde_json::Value> {
        serde_json::from_slice(&self.body).ok()
    }
}

/// Sends `method` `target` with `headers` and `body` to `addr` and reads
/// the answer. `declared_length` overrides the `Content-Length` sent (to
/// provoke a `413` without sending the bytes).
pub fn request(
    addr: SocketAddr,
    method: &str,
    target: &str,
    headers: &[(String, String)],
    body: &[u8],
    declared_length: Option<u64>,
    timeout: Duration,
) -> Result<Reply, String> {
    let mut stream =
        TcpStream::connect_timeout(&addr, timeout).map_err(|e| format!("connect {addr}: {e}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .map_err(|e| format!("socket setup: {e}"))?;
    let mut head = format!("{method} {target} HTTP/1.1\r\nConnection: close\r\n");
    if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("host")) {
        head.push_str(&format!("Host: {addr}\r\n"));
    }
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    let length = declared_length.unwrap_or(body.len() as u64);
    head.push_str(&format!("Content-Length: {length}\r\n\r\n"));
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(body))
        .map_err(|e| format!("write: {e}"))?;
    let mut raw = Vec::new();
    if let Err(e) = stream.read_to_end(&mut raw) {
        // A server that answered and closed early (413) may reset the
        // read; whatever arrived before is the answer.
        if raw.is_empty() {
            return Err(format!("read: {e}"));
        }
    }
    parse(&raw)
}

/// Parses a raw response.
pub fn parse(raw: &[u8]) -> Result<Reply, String> {
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut parsed = httparse::Response::new(&mut headers);
    let head_len = match parsed.parse(raw).map_err(|e| format!("response: {e}"))? {
        httparse::Status::Complete(n) => n,
        httparse::Status::Partial => return Err("incomplete response head".into()),
    };
    let status = parsed.code.ok_or("response without a status")?;
    let headers: Vec<(String, String)> = parsed
        .headers
        .iter()
        .map(|h| {
            (
                h.name.to_ascii_lowercase(),
                String::from_utf8_lossy(h.value).trim().to_string(),
            )
        })
        .collect();
    let length: Option<usize> = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse().ok());
    let rest = &raw[head_len..];
    let body = match length {
        Some(n) if n <= rest.len() => rest[..n].to_vec(),
        _ => rest.to_vec(),
    };
    Ok(Reply {
        status,
        headers,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_raw_response_is_parsed() {
        let reply = parse(
            b"HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
        )
        .unwrap();
        assert_eq!(reply.status, 404);
        assert_eq!(reply.header("content-type"), Some("application/json"));
        assert_eq!(reply.body, b"{}");
        assert_eq!(reply.json(), Some(serde_json::json!({})));
    }
}
