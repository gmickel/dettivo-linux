//! Model downloads (FR-E4): one thread per download, a `.part` file that
//! resumes with an HTTP `Range` request, progress reported at a bounded
//! rate, cancellation, and verification against the catalogue checksum
//! when the last byte lands; a mismatch quarantines the file (FR-P3). A
//! model set (ADR 0035) downloads its files one after the other under one
//! progress, a file that is already on disk and verified is skipped, and
//! a file that is an archive is unpacked to its member and the member
//! verified too.

use std::io::{Read, Seek, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::catalogue::{ModelEntry, ModelFile};
use crate::models::{ModelStore, sha256_of};

/// Where a download stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadState {
    /// Bytes are arriving.
    Running,
    /// The file landed and verified.
    Done,
    /// Stopped on request; the partial file stays for a resume.
    Cancelled,
    /// The transfer failed; the partial file stays for a resume.
    Failed,
    /// The file arrived with the wrong checksum and was moved aside.
    Quarantined,
}

/// Progress of one download.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    /// The state.
    pub state: DownloadState,
    /// Bytes on disk.
    pub bytes_done: u64,
    /// Bytes expected (the catalogue size until the server says otherwise).
    pub bytes_total: u64,
    /// The failure reason, without the URL's query or any body text.
    pub error: Option<String>,
    /// The transfer resumed from a partial file.
    pub resumed: bool,
}

/// The sidecar beside a partial file: what it was fetching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PartMeta {
    url: String,
    bytes_total: u64,
}

/// A running or finished download.
pub struct Download {
    progress: Arc<Mutex<Progress>>,
    cancel: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Download {
    /// The latest progress.
    pub fn progress(&self) -> Progress {
        self.progress
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Asks the transfer to stop at the next chunk.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// True while the thread runs.
    pub fn running(&self) -> bool {
        self.handle.as_ref().is_some_and(|h| !h.is_finished())
    }

    /// Waits for the thread and returns the final progress.
    pub fn join(mut self) -> Progress {
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        self.progress()
    }
}

/// Starts a download of `entry` into `store`; `on_progress` is called at
/// most four times a second plus once at the end.
pub fn start(
    store: ModelStore,
    entry: ModelEntry,
    on_progress: impl Fn(&Progress) + Send + 'static,
) -> Download {
    let progress = Arc::new(Mutex::new(Progress {
        state: DownloadState::Running,
        bytes_done: 0,
        bytes_total: entry.download_bytes(),
        error: None,
        resumed: false,
    }));
    let cancel = Arc::new(AtomicBool::new(false));
    let (p, c) = (progress.clone(), cancel.clone());
    let handle = std::thread::Builder::new()
        .name(format!("download-{}", entry.id))
        .spawn(move || {
            let result = run(&store, &entry, &p, &c, &on_progress);
            // The final snapshot is taken under the lock and published
            // after it is released: the callback takes the caller's own
            // lock, and a status request holds that lock while it asks
            // for this progress.
            let last = set(&p, |g| match result {
                Ok(state) => g.state = state,
                Err(e) => {
                    g.state = DownloadState::Failed;
                    g.error = Some(e);
                }
            });
            on_progress(&last);
        })
        .expect("download thread");
    Download {
        progress,
        cancel,
        handle: Some(handle),
    }
}

fn set(p: &Mutex<Progress>, f: impl FnOnce(&mut Progress)) -> Progress {
    let mut g = p.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut g);
    g.clone()
}

fn run(
    store: &ModelStore,
    entry: &ModelEntry,
    progress: &Mutex<Progress>,
    cancel: &AtomicBool,
    on_progress: &dyn Fn(&Progress),
) -> Result<DownloadState, String> {
    std::fs::create_dir_all(store.dir(entry)).map_err(|e| e.to_string())?;
    let total = entry.download_bytes();
    let mut done_before = 0u64;
    for file in entry.files() {
        let landed = store.file_path(entry, &file);
        if landed.is_file() && sha256_of(&landed).map_err(|e| e.to_string())? == file.disk_sha256()
        {
            done_before += file.size_bytes;
            continue;
        }
        let state = fetch_file(
            store,
            entry,
            &file,
            done_before,
            total,
            progress,
            cancel,
            on_progress,
        )?;
        if state != DownloadState::Done {
            return Ok(state);
        }
        done_before += file.size_bytes;
    }
    store
        .write_manifest(entry, true)
        .map_err(|e| e.to_string())?;
    tracing::info!(model = %entry.id, bytes = done_before, "model downloaded and verified");
    Ok(DownloadState::Done)
}

/// Fetches one file of the set into its `.part`, verifies it, unpacks an
/// archive's member, and moves the file into place; `done_before` is the
/// bytes the earlier files count towards `total`.
#[allow(clippy::too_many_arguments)]
fn fetch_file(
    store: &ModelStore,
    entry: &ModelEntry,
    file: &ModelFile,
    done_before: u64,
    total: u64,
    progress: &Mutex<Progress>,
    cancel: &AtomicBool,
    on_progress: &dyn Fn(&Progress),
) -> Result<DownloadState, String> {
    let part = store.file_part_path(entry, file);
    let meta_path = store.file_part_meta_path(entry, file);
    let existing = resumable_bytes(&part, &meta_path, &file.url);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let mut request = client.get(&file.url);
    if existing > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={existing}-"));
    }
    let mut response = request.send().map_err(|e| transport(&e))?;
    let status = response.status();
    let (mut out, mut done) = if status == reqwest::StatusCode::PARTIAL_CONTENT && existing > 0 {
        tracing::info!(model = %entry.id, file = %file.file_name, from = existing, "download resumed");
        set(progress, |p| p.resumed = true);
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&part)
            .map_err(|e| e.to_string())?;
        f.seek(std::io::SeekFrom::End(0))
            .map_err(|e| e.to_string())?;
        (f, existing)
    } else if status.is_success() {
        if existing > 0 {
            tracing::info!(model = %entry.id, "server ignored the range; downloading from the start");
        }
        let f = std::fs::File::create(&part).map_err(|e| e.to_string())?;
        (f, 0)
    } else {
        return Err(format!("server answered {status}"));
    };
    let file_total = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .map(|len| len + done)
        .unwrap_or(file.size_bytes);
    write_meta(&meta_path, &file.url, file_total);
    // The set's total follows the server when it corrects one file's size.
    let total = total - file.size_bytes + file_total;
    set(progress, |p| {
        p.bytes_done = done_before + done;
        p.bytes_total = total;
    });
    let mut buf = vec![0u8; 1 << 16];
    let mut last_report = Instant::now() - Duration::from_secs(1);
    loop {
        if cancel.load(Ordering::Relaxed) {
            out.flush().map_err(|e| e.to_string())?;
            return Ok(DownloadState::Cancelled);
        }
        let n = response.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if last_report.elapsed() >= Duration::from_millis(250) {
            last_report = Instant::now();
            let snapshot = set(progress, |p| p.bytes_done = done_before + done);
            on_progress(&snapshot);
        }
    }
    out.flush().map_err(|e| e.to_string())?;
    drop(out);
    set(progress, |p| p.bytes_done = done_before + done);
    if done < file_total {
        return Err(format!(
            "transfer of {} ended after {done} of {file_total} bytes",
            file.file_name
        ));
    }
    let have = sha256_of(&part).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&meta_path);
    if have != file.sha256 {
        tracing::warn!(model = %entry.id, file = %file.file_name, "downloaded file failed verification; quarantined");
        store.quarantine(entry, &part).map_err(|e| e.to_string())?;
        set(progress, |p| p.error = Some("checksum mismatch".into()));
        return Ok(DownloadState::Quarantined);
    }
    let landed = store.file_path(entry, file);
    match &file.unpack {
        None => std::fs::rename(&part, &landed).map_err(|e| e.to_string())?,
        Some(unpack) => {
            let extracted = unpack_member(&part, &unpack.member, &store.dir(entry))?;
            let _ = std::fs::remove_file(&part);
            let have = sha256_of(&extracted).map_err(|e| e.to_string())?;
            if have != unpack.sha256 {
                tracing::warn!(model = %entry.id, file = %file.file_name, "unpacked member failed verification; quarantined");
                store
                    .quarantine(entry, &extracted)
                    .map_err(|e| e.to_string())?;
                set(progress, |p| p.error = Some("checksum mismatch".into()));
                return Ok(DownloadState::Quarantined);
            }
            std::fs::rename(&extracted, &landed).map_err(|e| e.to_string())?;
        }
    }
    Ok(DownloadState::Done)
}

/// Extracts `member` from the `tar.bz2` at `archive` into a scratch
/// directory under `dir` and returns the extracted file's path.
fn unpack_member(archive: &Path, member: &str, dir: &Path) -> Result<std::path::PathBuf, String> {
    let scratch = dir.join(".unpack");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    let status = std::process::Command::new("tar")
        .arg("-xjf")
        .arg(archive)
        .arg("-C")
        .arg(&scratch)
        .arg(member)
        .status()
        .map_err(|e| format!("tar: {e}"))?;
    if !status.success() {
        return Err(format!("tar could not unpack {member} ({status})"));
    }
    let extracted = scratch.join(member);
    if !extracted.is_file() {
        return Err(format!("{member} is not in the archive"));
    }
    let moved = dir.join(format!(
        ".{}.unpacked",
        Path::new(member)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));
    std::fs::rename(&extracted, &moved).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&scratch);
    Ok(moved)
}

/// Bytes a partial file already holds for this URL, 0 when it belongs to
/// another source or there is none.
fn resumable_bytes(part: &Path, meta: &Path, url: &str) -> u64 {
    let Ok(size) = std::fs::metadata(part).map(|m| m.len()) else {
        return 0;
    };
    let same_source = std::fs::read_to_string(meta)
        .ok()
        .and_then(|t| serde_json::from_str::<PartMeta>(&t).ok())
        .is_some_and(|m| m.url == url);
    if same_source { size } else { 0 }
}

fn write_meta(path: &Path, url: &str, bytes_total: u64) {
    let meta = PartMeta {
        url: url.to_string(),
        bytes_total,
    };
    let _ = std::fs::write(path, serde_json::to_string(&meta).unwrap_or_default());
}

/// A transport error without the URL's query string.
fn transport(e: &reqwest::Error) -> String {
    let mut text = e.to_string();
    if let Some(i) = text.find('?') {
        text.truncate(i);
    }
    text
}
