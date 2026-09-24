//! Frames: `u32` big-endian header length, the JSON header, then each
//! attachment's bytes in order. The header names the attachments and their
//! sizes so a reader never guesses. A malformed header is rejected with the
//! field named; an attachment that does not match its declared size is a
//! framing error.

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The largest header a reader or writer accepts (1 MiB).
pub const MAX_HEADER_BYTES: u32 = 1 << 20;
/// Longest PCM track transported for one diarization pass (four hours).
pub const MAX_PCM_SECONDS: u64 = 14_400;
/// Maximum number of 16 kHz mono samples in that track.
pub const MAX_PCM_SAMPLES: u64 = MAX_PCM_SECONDS * 16_000;
/// Maximum total attachment bytes per frame, including a four-hour PCM track.
pub const MAX_ATTACHMENT_BYTES: u64 = MAX_PCM_SAMPLES * 2;

/// Checks a track's sample count before reading or encoding its PCM.
pub fn validate_pcm_samples(samples: u64) -> io::Result<()> {
    if samples > MAX_PCM_SAMPLES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("PCM track exceeds {MAX_PCM_SECONDS} seconds ({MAX_PCM_SAMPLES} samples)"),
        ));
    }
    Ok(())
}

fn validate_attachments(frame: &Frame) -> Result<(), String> {
    let mut total = 0u64;
    for attachment in &frame.attachments {
        total = total
            .checked_add(attachment.bytes)
            .filter(|total| *total <= MAX_ATTACHMENT_BYTES)
            .ok_or_else(|| format!("attachments: total bytes exceeds {MAX_ATTACHMENT_BYTES}"))?;
    }
    Ok(())
}

/// What a frame is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A request from the daemon to the engine.
    Request,
    /// The engine's answer to one request.
    Response,
    /// An unsolicited event from the engine (progress, partial, loaded, error).
    Event,
}

/// One declared attachment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attachment {
    /// `pcm16k` (16 kHz mono signed 16-bit little-endian) or a future kind.
    pub kind: String,
    /// Byte length.
    pub bytes: u64,
}

/// The JSON header of a frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    /// Protocol version, always [`crate::VERSION`].
    pub v: u32,
    /// Request id; responses and request-scoped events echo it.
    pub id: u64,
    /// Request, response or event.
    pub kind: Kind,
    /// Message name (`load`, `recognize`, `progress` ...).
    pub name: String,
    /// The typed payload as JSON.
    pub payload: Value,
    /// Declared attachments, in wire order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
}

impl Frame {
    /// A request frame.
    pub fn request(id: u64, name: &str, payload: Value) -> Self {
        Self {
            v: crate::VERSION,
            id,
            kind: Kind::Request,
            name: name.to_string(),
            payload,
            attachments: Vec::new(),
        }
    }

    /// A response frame for `id`.
    pub fn response(id: u64, name: &str, payload: Value) -> Self {
        Self {
            v: crate::VERSION,
            id,
            kind: Kind::Response,
            name: name.to_string(),
            payload,
            attachments: Vec::new(),
        }
    }

    /// An event frame (`id` 0 when not tied to a request).
    pub fn event(id: u64, name: &str, payload: Value) -> Self {
        Self {
            v: crate::VERSION,
            id,
            kind: Kind::Event,
            name: name.to_string(),
            payload,
            attachments: Vec::new(),
        }
    }

    /// Declares one PCM attachment.
    pub fn with_pcm(mut self, bytes: u64) -> Self {
        self.attachments.push(Attachment {
            kind: "pcm16k".into(),
            bytes,
        });
        self
    }
}

/// Why a frame could not be read.
#[derive(Debug)]
pub enum FrameError {
    /// The stream ended cleanly before a frame started.
    Eof,
    /// The transport failed.
    Io(io::Error),
    /// The header is not this protocol; the message names the field.
    Malformed(String),
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Eof => write!(f, "end of stream"),
            Self::Io(e) => write!(f, "transport: {e}"),
            Self::Malformed(m) => write!(f, "malformed frame: {m}"),
        }
    }
}

impl std::error::Error for FrameError {}

impl From<io::Error> for FrameError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Writes one frame and its attachment bytes (in declared order).
pub fn write_frame<W: Write>(w: &mut W, frame: &Frame, attachments: &[&[u8]]) -> io::Result<()> {
    validate_attachments(frame).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    if attachments.len() != frame.attachments.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "attachment count does not match the header",
        ));
    }
    for (declared, bytes) in frame.attachments.iter().zip(attachments) {
        if declared.bytes != bytes.len() as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "attachment {} declares {} bytes, got {}",
                    declared.kind,
                    declared.bytes,
                    bytes.len()
                ),
            ));
        }
    }
    let header = serde_json::to_vec(frame).map_err(io::Error::other)?;
    if header.len() > MAX_HEADER_BYTES as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "header exceeds maximum length",
        ));
    }
    w.write_all(&(header.len() as u32).to_be_bytes())?;
    w.write_all(&header)?;
    for bytes in attachments {
        w.write_all(bytes)?;
    }
    w.flush()
}

/// Reads one frame and its attachment bytes.
pub fn read_frame<R: Read>(r: &mut R) -> Result<(Frame, Vec<Vec<u8>>), FrameError> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Err(FrameError::Eof),
        Err(e) => return Err(FrameError::Io(e)),
    }
    let len = u32::from_be_bytes(len);
    if len == 0 || len > MAX_HEADER_BYTES {
        return Err(FrameError::Malformed(format!("header length {len}")));
    }
    let mut header = vec![0u8; len as usize];
    r.read_exact(&mut header)?;
    let frame: Frame =
        serde_path_to_error::deserialize(&mut serde_json::Deserializer::from_slice(&header))
            // The path names the field; the serde message is left out because
            // it can quote the offending value, and a header may carry a
            // prompt or a transcript that must never reach a log.
            .map_err(|e| {
                let kind = match e.inner().classify() {
                    serde_json::error::Category::Syntax => "invalid JSON",
                    serde_json::error::Category::Data => "wrong type or missing field",
                    serde_json::error::Category::Eof => "truncated JSON",
                    serde_json::error::Category::Io => "read error",
                };
                FrameError::Malformed(format!("{}: {kind}", e.path()))
            })?;
    if frame.v != crate::VERSION {
        return Err(FrameError::Malformed(format!(
            "v: {} is not {}",
            frame.v,
            crate::VERSION
        )));
    }
    validate_attachments(&frame).map_err(FrameError::Malformed)?;
    let mut attachments = Vec::with_capacity(frame.attachments.len());
    for a in &frame.attachments {
        let mut bytes = vec![0u8; a.bytes as usize];
        r.read_exact(&mut bytes)?;
        attachments.push(bytes);
    }
    Ok((frame, attachments))
}

/// Packs 16 kHz mono samples as little-endian bytes.
pub fn pcm_to_bytes(samples: &[i16]) -> Vec<u8> {
    samples.iter().flat_map(|s| s.to_le_bytes()).collect()
}

/// Unpacks little-endian bytes as samples (a trailing odd byte is dropped).
pub fn bytes_to_pcm(bytes: &[u8]) -> Vec<i16> {
    bytes
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn four_hour_pcm_fits_the_transport_and_oversized_headers_never_write() {
        let limits: Value =
            serde_json::from_str(include_str!("../tests/fixtures/pcm-limits.json")).unwrap();
        assert_eq!(
            json!({
                "max_seconds": MAX_PCM_SECONDS,
                "max_samples": MAX_PCM_SAMPLES,
                "max_attachment_bytes": MAX_ATTACHMENT_BYTES,
            }),
            limits
        );
        assert!(validate_pcm_samples(MAX_PCM_SAMPLES).is_ok());
        assert!(validate_pcm_samples(MAX_PCM_SAMPLES + 1).is_err());
        assert!(
            validate_attachments(
                &Frame::request(1, "diarize", json!({})).with_pcm(MAX_ATTACHMENT_BYTES)
            )
            .is_ok()
        );
        for frame in [
            Frame::request(1, "diarize", json!({})).with_pcm(MAX_ATTACHMENT_BYTES + 2),
            Frame::request(1, "diarize", json!({}))
                .with_pcm(MAX_ATTACHMENT_BYTES)
                .with_pcm(2),
        ] {
            let header = serde_json::to_vec(&frame).unwrap();
            let mut wire = (header.len() as u32).to_be_bytes().to_vec();
            wire.extend(header);
            let error = read_frame(&mut wire.as_slice()).unwrap_err();
            assert!(error.to_string().contains("exceeds"), "{error}");
            let mut output = Vec::new();
            let error = write_frame(&mut output, &frame, &[]).unwrap_err();
            assert!(error.to_string().contains("exceeds"), "{error}");
            assert!(output.is_empty());
        }
        let frame = Frame::request(
            1,
            "load",
            json!({"model": "x".repeat(MAX_HEADER_BYTES as usize)}),
        );
        let mut wire = Vec::new();
        let error = write_frame(&mut wire, &frame, &[]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(wire.is_empty(), "reject before writing the frame prefix");
    }

    #[test]
    fn frames_round_trip_with_attachments() {
        let pcm = pcm_to_bytes(&[1, -2, 3]);
        let frame =
            Frame::request(7, "recognize", json!({"language": "en"})).with_pcm(pcm.len() as u64);
        let mut buf = Vec::new();
        write_frame(&mut buf, &frame, &[&pcm]).unwrap();
        let (back, attachments) = read_frame(&mut buf.as_slice()).unwrap();
        assert_eq!(back, frame);
        assert_eq!(bytes_to_pcm(&attachments[0]), vec![1, -2, 3]);
        assert!(matches!(
            read_frame(&mut buf[buf.len()..].as_ref()),
            Err(FrameError::Eof)
        ));
    }

    #[test]
    fn malformed_headers_name_the_field() {
        let mut buf = Vec::new();
        let header = br#"{"v":1,"id":"x","kind":"request","name":"load","payload":{}}"#;
        buf.extend_from_slice(&(header.len() as u32).to_be_bytes());
        buf.extend_from_slice(header);
        let err = read_frame(&mut buf.as_slice()).unwrap_err();
        assert!(err.to_string().contains("id"), "{err}");

        let mut buf = Vec::new();
        let header = br#"{"v":2,"id":1,"kind":"request","name":"load","payload":{}}"#;
        buf.extend_from_slice(&(header.len() as u32).to_be_bytes());
        buf.extend_from_slice(header);
        let err = read_frame(&mut buf.as_slice()).unwrap_err();
        assert!(err.to_string().contains("v: 2"), "{err}");

        let mut buf = Vec::new();
        let header = br#"{"v":1,"id":1,"kind":"request","name":"load","payload":{},"extra":1}"#;
        buf.extend_from_slice(&(header.len() as u32).to_be_bytes());
        buf.extend_from_slice(header);
        let err = read_frame(&mut buf.as_slice()).unwrap_err();
        assert!(err.to_string().contains("extra"), "{err}");

        let frame = Frame::request(1, "recognize", json!({})).with_pcm(4);
        let mut buf = Vec::new();
        assert!(write_frame(&mut buf, &frame, &[&[0u8; 3]]).is_err());
    }
}
