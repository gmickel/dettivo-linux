//! Stdio framing. The first bytes a client sends decide the framing for
//! the whole session: a `{` or `[` means one JSON message per line, a
//! `Content-Length:` header means LSP-style framing, and the server
//! answers in the framing it detected. A message over the byte cap is
//! refused with the cap named and the session continues: the rest of the
//! line, or the announced byte count, is drained and the next message is
//! read as usual.

use std::io::{self, Read, Write};

use serde_json::Value;

/// The two framings a client may speak.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    /// One JSON object per `\n`-terminated line.
    Line,
    /// `Content-Length: N\r\n\r\n` followed by N bytes of JSON.
    ContentLength,
}

impl Framing {
    /// The name reported by `dettivo mcp check` and the harness.
    pub fn name(self) -> &'static str {
        match self {
            Self::Line => "line-delimited",
            Self::ContentLength => "content-length",
        }
    }
}

/// Why a message could not be read.
#[derive(Debug)]
pub enum ReadError {
    /// A message longer than the cap; the transport has skipped it.
    TooLarge {
        /// The cap in force.
        max_bytes: usize,
    },
    /// Bytes that are not a framed JSON object; the bytes were consumed.
    Invalid(String),
    /// The input failed.
    Io(io::Error),
}

impl From<io::Error> for ReadError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// What an oversized message leaves to drain before the next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Discard {
    None,
    UntilNewline,
    Bytes(usize),
}

/// A framing-detecting reader over any byte source.
pub struct Reader<R: Read> {
    input: R,
    buf: Vec<u8>,
    framing: Option<Framing>,
    max_bytes: usize,
    discard: Discard,
}

impl<R: Read> Reader<R> {
    /// A reader that refuses messages over `max_bytes`.
    pub fn new(input: R, max_bytes: usize) -> Self {
        Self {
            input,
            buf: Vec::new(),
            framing: None,
            max_bytes,
            discard: Discard::None,
        }
    }

    /// The framing detected so far.
    pub fn framing(&self) -> Option<Framing> {
        self.framing
    }

    /// The next message, `None` at end of input.
    pub fn read_message(&mut self) -> Result<Option<Value>, ReadError> {
        loop {
            if !self.drain()? {
                return Ok(None);
            }
            if let Some(message) = self.extract()? {
                return Ok(Some(message));
            }
            let mut chunk = [0u8; 65_536];
            let n = self.input.read(&mut chunk)?;
            if n == 0 {
                return Ok(None);
            }
            self.buf.extend_from_slice(&chunk[..n]);
        }
    }

    /// Finishes a pending discard; false at end of input.
    fn drain(&mut self) -> Result<bool, ReadError> {
        loop {
            match self.discard {
                Discard::None => return Ok(true),
                Discard::UntilNewline => {
                    if let Some(i) = self.buf.iter().position(|b| *b == b'\n') {
                        self.buf.drain(..=i);
                        self.discard = Discard::None;
                        return Ok(true);
                    }
                    self.buf.clear();
                }
                Discard::Bytes(n) => {
                    if self.buf.len() >= n {
                        self.buf.drain(..n);
                        self.discard = Discard::None;
                        return Ok(true);
                    }
                    self.discard = Discard::Bytes(n - self.buf.len());
                    self.buf.clear();
                }
            }
            let mut chunk = [0u8; 65_536];
            let n = self.input.read(&mut chunk)?;
            if n == 0 {
                return Ok(false);
            }
            self.buf.extend_from_slice(&chunk[..n]);
        }
    }

    fn extract(&mut self) -> Result<Option<Value>, ReadError> {
        let skip = self
            .buf
            .iter()
            .take_while(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
            .count();
        self.buf.drain(..skip);
        match self.framing {
            Some(Framing::Line) if is_header(&self.buf) => {
                let Some((end, separator)) = header_end(&self.buf) else {
                    if self.buf.len() > self.max_bytes {
                        return Err(ReadError::Io(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "refused framing switch has oversized headers",
                        )));
                    }
                    return Ok(None);
                };
                let length = content_length(&String::from_utf8_lossy(&self.buf[..end]))
                    .map_err(|e| ReadError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;
                self.buf.drain(..end + separator);
                self.discard = Discard::Bytes(length);
                Err(ReadError::Invalid(
                    "MCP framing cannot change during a session".into(),
                ))
            }
            Some(Framing::ContentLength) if matches!(self.buf.first(), Some(b'{' | b'[')) => {
                match self.extract_line()? {
                    None => Ok(None),
                    Some(_) => Err(ReadError::Invalid(
                        "MCP framing cannot change during a session".into(),
                    )),
                }
            }
            Some(Framing::Line) => self.extract_line(),
            Some(Framing::ContentLength) => self.extract_content_length(),
            None => match self.buf.first() {
                None => Ok(None),
                Some(b'{' | b'[') => {
                    self.framing = Some(Framing::Line);
                    self.extract_line()
                }
                Some(_) => {
                    self.framing = Some(Framing::ContentLength);
                    self.extract_content_length()
                }
            },
        }
    }

    fn extract_line(&mut self) -> Result<Option<Value>, ReadError> {
        let Some(end) = self.buf.iter().position(|b| *b == b'\n') else {
            if self.buf.len() > self.max_bytes {
                self.buf.clear();
                self.discard = Discard::UntilNewline;
                return Err(ReadError::TooLarge {
                    max_bytes: self.max_bytes,
                });
            }
            return Ok(None);
        };
        let mut line: Vec<u8> = self.buf.drain(..=end).collect();
        line.pop();
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        if line.len() > self.max_bytes {
            return Err(ReadError::TooLarge {
                max_bytes: self.max_bytes,
            });
        }
        parse(&line).map(Some)
    }

    fn extract_content_length(&mut self) -> Result<Option<Value>, ReadError> {
        let Some((header_end, separator_len)) = header_end(&self.buf) else {
            if self.buf.len() > self.max_bytes {
                self.buf.clear();
                return Err(ReadError::Invalid(
                    "MCP headers exceed transport limit".into(),
                ));
            }
            return Ok(None);
        };
        let header = String::from_utf8_lossy(&self.buf[..header_end]).into_owned();
        let payload_start = header_end + separator_len;
        let length = match content_length(&header) {
            Ok(n) => n,
            Err(e) => {
                self.buf.drain(..payload_start);
                return Err(ReadError::Invalid(e));
            }
        };
        if length > self.max_bytes {
            self.buf.drain(..payload_start);
            self.discard = Discard::Bytes(length);
            return Err(ReadError::TooLarge {
                max_bytes: self.max_bytes,
            });
        }
        if self.buf.len() < payload_start + length {
            return Ok(None);
        }
        let payload: Vec<u8> = self.buf[payload_start..payload_start + length].to_vec();
        self.buf.drain(..payload_start + length);
        parse(&payload).map(Some)
    }
}

fn is_header(buf: &[u8]) -> bool {
    let Some(end) = buf.iter().position(|b| *b == b'\n') else {
        return false;
    };
    let Some(colon) = buf[..end].iter().position(|b| *b == b':') else {
        return false;
    };
    colon > 0
        && buf[..colon]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(b))
}

/// The offset of the first header separator and its length.
fn header_end(buf: &[u8]) -> Option<(usize, usize)> {
    let crlf = find(buf, b"\r\n\r\n").map(|i| (i, 4));
    let lf = find(buf, b"\n\n").map(|i| (i, 2));
    match (crlf, lf) {
        (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
        (a, b) => a.or(b),
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn content_length(header: &str) -> Result<usize, String> {
    for line in header.replace("\r\n", "\n").split('\n') {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            return rest
                .trim()
                .parse::<usize>()
                .map_err(|_| "Invalid Content-Length".to_string());
        }
    }
    Err("Missing Content-Length header".into())
}

fn parse(bytes: &[u8]) -> Result<Value, ReadError> {
    match serde_json::from_slice::<Value>(bytes) {
        Ok(v) if v.is_object() => Ok(v),
        _ => Err(ReadError::Invalid("Invalid JSON payload".into())),
    }
}

/// One message in `framing`, ready to write.
pub fn encode(framing: Framing, message: &Value) -> Vec<u8> {
    let payload = serde_json::to_vec(message).unwrap_or_default();
    match framing {
        Framing::Line => {
            let mut out = payload;
            out.push(b'\n');
            out
        }
        Framing::ContentLength => {
            let mut out = format!("Content-Length: {}\r\n\r\n", payload.len()).into_bytes();
            out.extend_from_slice(&payload);
            out
        }
    }
}

/// Writes `message` in `framing`, line-delimited while none is known yet.
pub fn write_message(
    out: &mut impl Write,
    framing: Option<Framing>,
    message: &Value,
) -> io::Result<()> {
    out.write_all(&encode(framing.unwrap_or(Framing::Line), message))?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn reader(bytes: &[u8]) -> Reader<std::io::Cursor<Vec<u8>>> {
        Reader::new(std::io::Cursor::new(bytes.to_vec()), 64)
    }

    #[test]
    fn line_and_content_length_are_detected_from_the_first_bytes() {
        let mut r = reader(b"{\"a\":1}\r\n\n{\"b\":2}\n");
        assert_eq!(r.read_message().unwrap(), Some(json!({"a": 1})));
        assert_eq!(r.framing(), Some(Framing::Line));
        assert_eq!(r.read_message().unwrap(), Some(json!({"b": 2})));
        assert_eq!(r.read_message().unwrap(), None);

        let mut r = reader(b"Content-Length: 7\r\n\r\n{\"a\":1}content-length: 7\n\n{\"b\":2}");
        assert_eq!(r.read_message().unwrap(), Some(json!({"a": 1})));
        assert_eq!(r.framing(), Some(Framing::ContentLength));
        assert_eq!(r.read_message().unwrap(), Some(json!({"b": 2})));
        assert_eq!(r.read_message().unwrap(), None);
    }

    #[test]
    fn a_message_over_the_cap_is_skipped_and_the_session_continues() {
        let big = format!("{{\"pad\":\"{}\"}}\n{{\"ok\":1}}\n", "x".repeat(100));
        let mut r = reader(big.as_bytes());
        assert!(matches!(
            r.read_message(),
            Err(ReadError::TooLarge { max_bytes: 64 })
        ));
        assert_eq!(r.read_message().unwrap(), Some(json!({"ok": 1})));

        let payload = format!("{{\"pad\":\"{}\"}}", "y".repeat(100));
        let framed = format!(
            "Content-Length: {}\r\n\r\n{payload}Content-Length: 8\r\n\r\n{{\"ok\":2}}",
            payload.len()
        );
        let mut r = reader(framed.as_bytes());
        assert!(matches!(r.read_message(), Err(ReadError::TooLarge { .. })));
        assert_eq!(r.read_message().unwrap(), Some(json!({"ok": 2})));
    }

    #[test]
    fn the_first_framing_sticks_and_a_switch_is_refused() {
        let mut r = reader(b"{\"a\":1}\nContent-Length: 7\r\n\r\n{\"b\":2}\n");
        assert_eq!(r.read_message().unwrap(), Some(json!({"a": 1})));
        assert!(matches!(r.read_message(), Err(ReadError::Invalid(_))));
        assert_eq!(r.framing(), Some(Framing::Line));
    }

    #[test]
    fn encode_mirrors_each_framing() {
        assert_eq!(encode(Framing::Line, &json!({"a": 1})), b"{\"a\":1}\n");
        assert_eq!(
            encode(Framing::ContentLength, &json!({"a": 1})),
            b"Content-Length: 7\r\n\r\n{\"a\":1}"
        );
    }
}
