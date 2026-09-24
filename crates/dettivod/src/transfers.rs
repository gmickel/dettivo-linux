//! `transfer.*`: chunked upload and download over the line protocol. An
//! upload is spooled to `$XDG_CACHE_HOME/dettivo/transfers/<id>` chunk by
//! chunk in order, committed against a SHA-256, and consumed by
//! `transcripts.import`; a download is a file `transcripts.export` binds to
//! a transfer the client began, pulled chunk by chunk. Idle transfers
//! expire after `transfers.timeout_seconds`.

use std::collections::HashMap;
use std::io::{Seek, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use base64::Engine as _;
use dettivo_proto::capabilities::TransferCaps;
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::transfer::{
    BeginResult, CancelResult, ChunkResult, CommitResult, Direction, PullResult,
};
use sha2::{Digest, Sha256};

/// The content types an upload may declare (FR-S6 plus the Linux
/// additions `audio/flac`, `audio/ogg` and the archive).
pub const CONTENT_TYPES: &[&str] = &[
    "audio/wav",
    "audio/x-wav",
    "audio/mpeg",
    "audio/mp4",
    "audio/m4a",
    "audio/aac",
    "audio/x-caf",
    "audio/caf",
    "audio/aiff",
    "audio/flac",
    "audio/ogg",
    "application/zip",
];

/// The largest upload accepted unless `[transfer] max_upload_bytes` says
/// otherwise.
pub const MAX_UPLOAD_BYTES: u64 = 1 << 30;

/// A finished upload, ready to import.
#[derive(Debug, Clone)]
pub struct Upload {
    /// The transfer id.
    pub id: String,
    /// The spooled file.
    pub path: PathBuf,
    /// The declared content type.
    pub content_type: String,
}

#[derive(Debug)]
struct Transfer {
    direction: Direction,
    content_type: String,
    path: PathBuf,
    file: std::fs::File,
    /// The next chunk expected (upload) or the number of chunks (download).
    next_seq: u64,
    /// Bytes accepted so far (upload); the limit applies to these, not to
    /// the client's size hint.
    received_bytes: u64,
    committed: bool,
    bound: bool,
    expires: Instant,
}

/// The service.
pub struct Transfers {
    inner: Mutex<HashMap<String, Transfer>>,
    next: AtomicU64,
    staging: PathBuf,
    exports: PathBuf,
    caps: TransferCaps,
    max_upload: AtomicU64,
}

fn invalid(message: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(AppCode::InvalidParams, message, ErrorDetails::empty())
}

fn not_found() -> JsonRpcError {
    JsonRpcError::new(
        AppCode::NotFound,
        "Transfer not found",
        ErrorDetails::empty(),
    )
}

fn io(message: String) -> JsonRpcError {
    JsonRpcError::new(AppCode::InternalError, message, ErrorDetails::empty())
}

fn private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(dir)?;
    file.set_permissions(std::fs::Permissions::from_mode(0o700))
}

/// ISO 8601 UTC of `now + timeout`.
fn expires_at(timeout: Duration) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        + timeout.as_secs() as i64;
    dettivo_storage::time::iso_from_unix(secs)
}

impl Transfers {
    /// A service spooling under `cache_dir`.
    pub fn new(cache_dir: &Path, caps: TransferCaps) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            staging: cache_dir.join("transfers"),
            exports: cache_dir.join("exports"),
            caps,
            max_upload: AtomicU64::new(MAX_UPLOAD_BYTES),
        }
    }

    /// Applies `[transfer] max_upload_bytes` (start and reload).
    pub fn set_max_upload(&self, bytes: u64) {
        self.max_upload.store(bytes.max(1), Ordering::Relaxed);
    }

    fn max_upload(&self) -> u64 {
        self.max_upload.load(Ordering::Relaxed)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(self.caps.timeout_seconds.max(1))
    }

    /// `transfer.begin`.
    pub fn begin(
        &self,
        direction: Direction,
        content_type: &str,
        size_hint: u64,
    ) -> Result<BeginResult, JsonRpcError> {
        if direction == Direction::Upload {
            if !CONTENT_TYPES.contains(&content_type) {
                return Err(invalid(format!(
                    "{content_type} is not an import content type; supported: {}",
                    CONTENT_TYPES.join(", ")
                )));
            }
            let limit = self.max_upload();
            if size_hint > limit {
                return Err(invalid(format!(
                    "size_hint {size_hint} exceeds the upload limit of {limit} bytes (transfer.max_upload_bytes)"
                )));
            }
        }
        let dir = match direction {
            Direction::Upload => &self.staging,
            Direction::Download => &self.exports,
        };
        private_dir(dir).map_err(|e| io(format!("cannot create {}: {e}", dir.display())))?;
        let (id, path, file) = loop {
            let n = self.next.fetch_add(1, Ordering::Relaxed);
            let id = match direction {
                Direction::Upload => format!("xfer_in_{n}"),
                Direction::Download => format!("xfer_out_{n}"),
            };
            let path = dir.join(&id);
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
            {
                Ok(file) => {
                    file.set_permissions(std::fs::Permissions::from_mode(0o600))
                        .map_err(|e| io(format!("cannot protect {}: {e}", path.display())))?;
                    break (id, path, file);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(io(format!("cannot create {}: {e}", path.display()))),
            }
        };
        self.inner.lock().unwrap_or_else(|p| p.into_inner()).insert(
            id.clone(),
            Transfer {
                direction,
                content_type: content_type.to_string(),
                path,
                file,
                next_seq: 1,
                received_bytes: 0,
                committed: false,
                bound: false,
                expires: Instant::now() + self.timeout(),
            },
        );
        tracing::debug!(transfer = %id, direction = ?direction, "transfer begun");
        Ok(BeginResult {
            transfer_id: id,
            chunk_max_bytes: self.caps.chunk_max_bytes,
            expires_at: expires_at(self.timeout()),
        })
    }

    /// `transfer.chunk`: chunks arrive in order from 1; one further ahead
    /// than `max_inflight` is refused as local backpressure.
    pub fn chunk(&self, id: &str, seq: u64, data_b64: &str) -> Result<ChunkResult, JsonRpcError> {
        let mut g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let t = g.get_mut(id).ok_or_else(not_found)?;
        if t.direction != Direction::Upload || t.committed {
            return Err(invalid(format!("{id} does not accept chunks")));
        }
        if seq > t.next_seq + u64::from(self.caps.max_inflight) {
            return Err(JsonRpcError::new(
                AppCode::RateLimitedLocal,
                "Too many chunks in flight",
                ErrorDetails::empty(),
            ));
        }
        if seq != t.next_seq {
            return Err(invalid(format!(
                "chunk {seq} out of order; {} expected",
                t.next_seq
            )));
        }
        let data = base64::engine::general_purpose::STANDARD
            .decode(data_b64)
            .map_err(|e| invalid(format!("data_b64: {e}")))?;
        if data.len() as u64 > self.caps.chunk_max_bytes {
            return Err(invalid(format!(
                "chunk is {} bytes; chunk_max_bytes is {}",
                data.len(),
                self.caps.chunk_max_bytes
            )));
        }
        let total = t.received_bytes.saturating_add(data.len() as u64);
        let limit = self.max_upload();
        if total > limit {
            return Err(invalid(format!(
                "upload would reach {total} bytes; the limit is {limit} (transfer.max_upload_bytes)"
            )));
        }
        t.file
            .write_all(&data)
            .map_err(|e| io(format!("cannot write {}: {e}", t.path.display())))?;
        t.received_bytes = total;
        t.next_seq += 1;
        t.expires = Instant::now() + self.timeout();
        Ok(ChunkResult {
            accepted: true,
            next_seq: t.next_seq,
        })
    }

    /// `transfer.commit`: an upload's chunk count and SHA-256 must match
    /// what arrived; a download's commit is the client's final ack.
    pub fn commit(
        &self,
        id: &str,
        total_chunks: u64,
        sha256: &str,
    ) -> Result<CommitResult, JsonRpcError> {
        let mut g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let t = g.get_mut(id).ok_or_else(not_found)?;
        if t.direction == Direction::Download {
            t.committed = true;
            return Ok(CommitResult { committed: true });
        }
        let received = t.next_seq - 1;
        if total_chunks != received {
            return Err(invalid(format!(
                "total_chunks {total_chunks} does not match the {received} chunks received"
            )));
        }
        let mut file = std::fs::File::open(&t.path)
            .map_err(|e| io(format!("cannot read {}: {e}", t.path.display())))?;
        let mut hasher = Sha256::new();
        let bytes = std::io::copy(&mut file, &mut hasher)
            .map_err(|e| io(format!("cannot read {}: {e}", t.path.display())))?;
        let digest = format!("{:x}", hasher.finalize());
        if !digest.eq_ignore_ascii_case(sha256) {
            return Err(invalid(format!(
                "sha256 {sha256} does not match the uploaded bytes ({digest})"
            )));
        }
        t.committed = true;
        t.expires = Instant::now() + self.timeout();
        tracing::debug!(transfer = %id, bytes, "upload committed");
        Ok(CommitResult { committed: true })
    }

    /// `transfer.cancel`: drops the staging or export file.
    pub fn cancel(&self, id: &str, _reason: &str) -> Result<CancelResult, JsonRpcError> {
        let t = self
            .inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id)
            .ok_or_else(not_found)?;
        let _ = std::fs::remove_file(&t.path);
        tracing::debug!(transfer = %id, "transfer cancelled");
        Ok(CancelResult { cancelled: true })
    }

    /// `transfer.pull`: chunk `seq` (from 1) of a bound download.
    pub fn pull(&self, id: &str, seq: u64) -> Result<PullResult, JsonRpcError> {
        let mut g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let t = g.get_mut(id).ok_or_else(not_found)?;
        if t.direction != Direction::Download || !t.bound {
            return Err(JsonRpcError::new(
                AppCode::Conflict,
                format!("{id} has no export bound to it yet"),
                ErrorDetails::empty(),
            ));
        }
        if seq == 0 || seq > t.next_seq {
            return Err(invalid(format!(
                "seq {seq} is past the last chunk {}",
                t.next_seq
            )));
        }
        // Only the requested range leaves the disk: a large export pulled in
        // many chunks costs one read per chunk, not one read of the whole
        // file per chunk.
        use std::io::{Read, Seek, SeekFrom};
        let size = self.caps.chunk_max_bytes;
        let mut file = std::fs::File::open(&t.path)
            .map_err(|e| io(format!("cannot read {}: {e}", t.path.display())))?;
        file.seek(SeekFrom::Start((seq - 1) * size))
            .map_err(|e| io(format!("cannot read {}: {e}", t.path.display())))?;
        let mut slice = Vec::with_capacity(size.min(1 << 20) as usize);
        file.take(size)
            .read_to_end(&mut slice)
            .map_err(|e| io(format!("cannot read {}: {e}", t.path.display())))?;
        t.expires = Instant::now() + self.timeout();
        Ok(PullResult {
            seq,
            data_b64: base64::engine::general_purpose::STANDARD.encode(&slice),
            eof: seq == t.next_seq,
        })
    }

    /// Binds rendered bytes to a download transfer the client began.
    pub fn bind_export(&self, id: &str, bytes: &[u8]) -> Result<(), JsonRpcError> {
        let mut g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let t = g.get_mut(id).ok_or_else(not_found)?;
        if t.direction != Direction::Download {
            return Err(invalid(format!(
                "{id} is an upload; begin a download transfer for an export"
            )));
        }
        t.file
            .set_len(0)
            .and_then(|()| t.file.rewind())
            .and_then(|()| t.file.write_all(bytes))
            .map_err(|e| io(format!("cannot write {}: {e}", t.path.display())))?;
        let size = self.caps.chunk_max_bytes.max(1);
        t.next_seq = (bytes.len() as u64).div_ceil(size).max(1);
        t.bound = true;
        t.expires = Instant::now() + self.timeout();
        Ok(())
    }

    /// The committed upload behind `id`, for an import.
    pub fn upload(&self, id: &str) -> Result<Upload, JsonRpcError> {
        let g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let t = g.get(id).ok_or_else(not_found)?;
        if t.direction != Direction::Upload {
            return Err(invalid(format!("{id} is a download")));
        }
        if !t.committed {
            return Err(JsonRpcError::new(
                AppCode::Conflict,
                format!("{id} is not committed yet"),
                ErrorDetails::conflict_kind("transferNotCommitted"),
            ));
        }
        Ok(Upload {
            id: id.to_string(),
            path: t.path.clone(),
            content_type: t.content_type.clone(),
        })
    }

    /// Forgets a consumed upload and hands its file to the caller, who
    /// removes it when done (an import job decodes it in its own thread).
    pub fn take_upload(&self, id: &str) -> PathBuf {
        let mut g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        match g.remove(id) {
            Some(t) => t.path,
            None => self.staging.join(id),
        }
    }

    /// Forgets a consumed upload and removes its file.
    pub fn finish_upload(&self, id: &str) {
        if let Some(t) = self
            .inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id)
        {
            let _ = std::fs::remove_file(&t.path);
        }
    }

    /// Drops transfers idle past the timeout; returns how many.
    pub fn expire(&self) -> usize {
        let now = Instant::now();
        let mut g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let gone: Vec<String> = g
            .iter()
            .filter(|(_, t)| t.expires <= now)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &gone {
            if let Some(t) = g.remove(id) {
                let _ = std::fs::remove_file(&t.path);
            }
        }
        gone.len()
    }

    /// How many transfers are open.
    #[cfg(test)]
    pub fn count(&self) -> usize {
        self.inner.lock().unwrap_or_else(|p| p.into_inner()).len()
    }
}

#[cfg(test)]
#[path = "transfers_tests.rs"]
mod tests;
