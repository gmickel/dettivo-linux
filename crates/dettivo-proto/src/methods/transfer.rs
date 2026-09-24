//! `transfer.*`: chunked binary upload/download framing
//! (`docs/api/dettivo-ipc-v1.md` section 8.9).

use serde::{Deserialize, Serialize};

/// Transfer direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Client to server.
    Upload,
    /// Server to client.
    Download,
}

/// `transfer.begin` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginParams {
    /// Transfer direction.
    pub direction: Direction,
    /// MIME content type of the payload.
    pub content_type: String,
    /// Estimated total size in bytes, when known.
    pub size_hint: u64,
}

/// `transfer.begin` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginResult {
    /// Opaque transfer id, e.g. `"xfer_1"` (not a UUID by contract
    /// example).
    pub transfer_id: String,
    /// Maximum bytes per `transfer.chunk`.
    pub chunk_max_bytes: u64,
    /// ISO 8601 expiry timestamp.
    pub expires_at: String,
}

/// `transfer.chunk` params: an upload chunk (client to server).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChunkParams {
    /// The transfer this chunk belongs to.
    pub transfer_id: String,
    /// Zero-based chunk sequence number.
    pub seq: u64,
    /// Base64-encoded chunk bytes.
    pub data_b64: String,
}

/// `transfer.chunk` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChunkResult {
    /// Whether the chunk was accepted.
    pub accepted: bool,
    /// The next expected sequence number.
    pub next_seq: u64,
}

/// `transfer.pull` params: a download chunk request (client from server).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PullParams {
    /// The transfer to pull from.
    pub transfer_id: String,
    /// Requested chunk sequence number.
    pub seq: u64,
}

/// `transfer.pull` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PullResult {
    /// The chunk's sequence number.
    pub seq: u64,
    /// Base64-encoded chunk bytes.
    pub data_b64: String,
    /// Whether this is the final chunk.
    pub eof: bool,
}

/// `transfer.commit` params. Upload: the client calls this after the last
/// `transfer.chunk`. Download: the server-side transfer is ready at
/// begin/export binding; the client MAY call commit as a final ack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitParams {
    /// The transfer to commit.
    pub transfer_id: String,
    /// Total chunk count sent/expected.
    pub total_chunks: u64,
    /// Hex-encoded SHA-256 of the full payload.
    pub sha256: String,
}

/// `transfer.commit` result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitResult {
    /// Always `true` on success.
    pub committed: bool,
}

/// `transfer.cancel` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelParams {
    /// The transfer to cancel.
    pub transfer_id: String,
    /// Cancellation reason, e.g. `"client_abort"`.
    pub reason: String,
}

/// `transfer.cancel` result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelResult {
    /// Always `true` on success.
    pub cancelled: bool,
}
