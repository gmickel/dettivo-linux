//! HTTP/1.1 over a `tokio` stream with `httparse`: one request head (at
//! most 16 KiB), a body of exactly `Content-Length` bytes up to the cap,
//! and one response with `Content-Length`. Keep-alive is HTTP/1.1's
//! default unless the client says `Connection: close`; a body over the
//! cap is answered `413` before it is read and the connection closes.

use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

/// The largest request head (request line plus headers) accepted.
pub const MAX_HEAD_BYTES: usize = 16 * 1024;

/// One parsed request. Header names are lower-cased.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// `GET`, `POST`, ...
    pub method: String,
    /// The path without the query.
    pub path: String,
    /// The raw query string after `?`, empty when absent.
    pub query: String,
    /// `(name, value)` with the name lower-cased, in order.
    pub headers: Vec<(String, String)>,
    /// The body, exactly `Content-Length` bytes.
    pub body: Vec<u8>,
    /// Whether the client wants the connection kept open afterwards.
    pub keep_alive: bool,
}

impl Request {
    /// The first header with `name` (lower-case).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Response storage; exports remain on disk until the client consumes them.
#[derive(Debug)]
pub enum Body {
    /// Small JSON responses.
    Bytes(Vec<u8>),
    /// An anonymous temporary file and its completed length.
    File(std::fs::File, u64),
}

impl Body {
    fn len(&self) -> u64 {
        match self {
            Self::Bytes(bytes) => bytes.len() as u64,
            Self::File(_, len) => *len,
        }
    }
}

/// One response.
#[derive(Debug)]
pub struct Response {
    /// The status code.
    pub status: u16,
    /// Extra headers; `Content-Length` and `Connection` are added on write.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Body,
}

/// Why a request could not be read.
#[derive(Debug, PartialEq, Eq)]
pub enum ReadError {
    /// The client closed the connection between requests.
    Closed,
    /// The head is not HTTP the parser accepts.
    Malformed(String),
    /// The head passed `MAX_HEAD_BYTES`.
    HeadTooLarge,
    /// `Content-Length` is over the cap; the body was not read.
    BodyTooLarge {
        /// The declared length.
        declared: u64,
        /// The cap in force.
        cap: u64,
    },
    /// The stream failed.
    Io(String),
}

/// Reads one request; `max_body` caps the body. `carry` holds the bytes
/// read past the previous request on the same connection (a pipelined
/// request) and receives the bytes past this one.
pub async fn read_request<R: AsyncReadExt + Unpin>(
    reader: &mut R,
    max_body: u64,
    carry: &mut Vec<u8>,
) -> Result<Request, ReadError> {
    let mut buf: Vec<u8> = std::mem::take(carry);
    let (head_len, mut request) = loop {
        if let Some(parsed) = parse_head(&buf)? {
            break parsed;
        }
        if buf.len() >= MAX_HEAD_BYTES {
            return Err(ReadError::HeadTooLarge);
        }
        let mut chunk = [0u8; 4096];
        let n = reader
            .read(&mut chunk)
            .await
            .map_err(|e| ReadError::Io(e.to_string()))?;
        if n == 0 {
            return if buf.is_empty() {
                Err(ReadError::Closed)
            } else {
                Err(ReadError::Malformed("connection closed mid-head".into()))
            };
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    if request
        .header("transfer-encoding")
        .is_some_and(|v| !v.eq_ignore_ascii_case("identity"))
    {
        return Err(ReadError::Malformed(
            "chunked request bodies are not supported; send Content-Length".into(),
        ));
    }
    let declared: u64 = match request.header("content-length") {
        Some(v) => v
            .trim()
            .parse()
            .map_err(|_| ReadError::Malformed(format!("Content-Length {v:?} is not a number")))?,
        None => 0,
    };
    if declared > max_body {
        return Err(ReadError::BodyTooLarge {
            declared,
            cap: max_body,
        });
    }
    let declared = usize::try_from(declared).map_err(|_| ReadError::BodyTooLarge {
        declared,
        cap: max_body,
    })?;
    let mut body = buf.split_off(head_len);
    if body.len() > declared {
        *carry = body.split_off(declared);
    }
    while body.len() < declared {
        let mut chunk = vec![0u8; (declared - body.len()).min(64 * 1024)];
        let n = reader
            .read(&mut chunk)
            .await
            .map_err(|e| ReadError::Io(e.to_string()))?;
        if n == 0 {
            return Err(ReadError::Malformed("connection closed mid-body".into()));
        }
        body.extend_from_slice(&chunk[..n]);
    }
    request.body = body;
    Ok(request)
}

/// Parses a complete head out of `buf`: the head length and the request
/// without its body, or `None` while the head is still incomplete.
fn parse_head(buf: &[u8]) -> Result<Option<(usize, Request)>, ReadError> {
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut parsed = httparse::Request::new(&mut headers);
    let status = parsed
        .parse(buf)
        .map_err(|e| ReadError::Malformed(e.to_string()))?;
    let httparse::Status::Complete(head_len) = status else {
        return Ok(None);
    };
    let method = parsed.method.unwrap_or("").to_string();
    let target = parsed.path.unwrap_or("");
    let version = parsed.version.unwrap_or(1);
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target.to_string(), String::new()),
    };
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
    let connection = headers
        .iter()
        .find(|(k, _)| k == "connection")
        .map(|(_, v)| v.to_ascii_lowercase());
    let keep_alive = match connection.as_deref() {
        Some("close") => false,
        Some("keep-alive") => true,
        _ => version == 1,
    };
    Ok(Some((
        head_len,
        Request {
            method,
            path,
            query,
            headers,
            body: Vec::new(),
            keep_alive,
        },
    )))
}

/// The reason phrase for a status.
pub fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

/// Serialises the status line and headers, including body length and connection.
fn encode_head(response: &Response, keep_alive: bool) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status,
        reason(response.status)
    )
    .into_bytes();
    for (k, v) in &response.headers {
        out.extend_from_slice(format!("{k}: {v}\r\n").as_bytes());
    }
    out.extend_from_slice(format!("Content-Length: {}\r\n", response.body.len()).as_bytes());
    out.extend_from_slice(
        if keep_alive {
            "Connection: keep-alive\r\n\r\n"
        } else {
            "Connection: close\r\n\r\n"
        }
        .as_bytes(),
    );
    out
}

/// Writes one response.
pub async fn write_response<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    response: &Response,
    keep_alive: bool,
) -> std::io::Result<()> {
    writer.write_all(&encode_head(response, keep_alive)).await?;
    match &response.body {
        Body::Bytes(bytes) => writer.write_all(bytes).await?,
        Body::File(file, len) => {
            let mut reader = tokio::fs::File::from_std(file.try_clone()?);
            reader.rewind().await?;
            let copied = tokio::io::copy(&mut reader.take(*len), writer).await?;
            if copied != *len {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "export body truncated",
                ));
            }
        }
    }
    writer.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn file_response_writes_bounded_chunks_with_exact_content_length() {
        struct Sink {
            written: u64,
            largest: usize,
        }
        impl tokio::io::AsyncWrite for Sink {
            fn poll_write(
                mut self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
                bytes: &[u8],
            ) -> std::task::Poll<std::io::Result<usize>> {
                self.written += bytes.len() as u64;
                self.largest = self.largest.max(bytes.len());
                std::task::Poll::Ready(Ok(bytes.len()))
            }
            fn poll_flush(
                self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::task::Poll::Ready(Ok(()))
            }
            fn poll_shutdown(
                self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::task::Poll::Ready(Ok(()))
            }
        }
        let len = 16 * 1024 * 1024;
        let file = tempfile::tempfile().unwrap();
        file.set_len(len).unwrap();
        let response = Response {
            status: 200,
            headers: vec![],
            body: Body::File(file, len),
        };
        let header = encode_head(&response, true);
        assert!(String::from_utf8_lossy(&header).contains("Content-Length: 16777216\r\n"));
        let mut sink = Sink {
            written: 0,
            largest: 0,
        };
        write_response(&mut sink, &response, true).await.unwrap();
        assert_eq!(sink.written, len + header.len() as u64);
        assert!(sink.largest <= 64 * 1024);
    }

    #[tokio::test]
    async fn a_request_is_parsed_with_its_body_and_the_cap_is_enforced() {
        let raw = b"POST /v1/system/ping?x=1 HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}GET / HTTP/1.1\r\n\r\n";
        let mut cursor = std::io::Cursor::new(raw.to_vec());
        let mut carry = Vec::new();
        let request = read_request(&mut cursor, 10, &mut carry).await.unwrap();
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/v1/system/ping");
        assert_eq!(request.query, "x=1");
        assert_eq!(request.header("host"), Some("127.0.0.1"));
        assert_eq!(request.body, b"{}");
        assert!(request.keep_alive);
        // The pipelined second request is carried into the next read.
        let second = read_request(&mut cursor, 10, &mut carry).await.unwrap();
        assert_eq!(second.path, "/");
        assert!(carry.is_empty());
        let mut cursor = std::io::Cursor::new(b"GET / HTTP/1.0\r\n\r\n".to_vec());
        assert!(
            !read_request(&mut cursor, 10, &mut carry)
                .await
                .unwrap()
                .keep_alive
        );
        let mut cursor =
            std::io::Cursor::new(b"POST / HTTP/1.1\r\nContent-Length: 11\r\n\r\n".to_vec());
        assert_eq!(
            read_request(&mut cursor, 10, &mut carry).await.unwrap_err(),
            ReadError::BodyTooLarge {
                declared: 11,
                cap: 10
            }
        );
        let mut cursor = std::io::Cursor::new(Vec::new());
        assert_eq!(
            read_request(&mut cursor, 10, &mut carry).await.unwrap_err(),
            ReadError::Closed
        );
        let mut cursor = std::io::Cursor::new(b"nonsense\r\n\r\n".to_vec());
        assert!(matches!(
            read_request(&mut cursor, 10, &mut carry).await.unwrap_err(),
            ReadError::Malformed(_)
        ));
        let mut encoded = Vec::new();
        write_response(
            &mut encoded,
            &Response {
                status: 404,
                headers: vec![("Content-Type".into(), "text/plain".into())],
                body: Body::Bytes(b"no".to_vec()),
            },
            false,
        )
        .await
        .unwrap();
        let text = String::from_utf8(encoded).unwrap();
        assert!(text.starts_with("HTTP/1.1 404 Not Found\r\n"));
        assert!(text.contains("Content-Length: 2\r\n"));
        assert!(text.ends_with("Connection: close\r\n\r\nno"));
    }
}
