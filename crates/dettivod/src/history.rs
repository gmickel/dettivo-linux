//! The history service: the store behind one lock, the artifact directory,
//! the retention settings in force, the set of items a job is working on,
//! the archive hook the session calls with every finished transcript, the
//! QA seed and the hourly sweep. Errors map to the contract here so every
//! handler answers the same way (`database_busy`, `NOT_FOUND`).

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Mutex, RwLock};

use dettivo_core::config::Loaded;
use dettivo_core::paths::Paths;
use dettivo_proto::error::{AppCode, ErrorData, ErrorDetails, JsonRpcError};
use dettivo_session::{Archive, Transcript};
use dettivo_storage::item::{DictationItem, ItemStatus, SourceKind};
use dettivo_storage::meetings::{MeetingArtifacts, MeetingRow, MeetingStatus};
use dettivo_storage::retention::{self, ArtifactPolicy, Artifacts, Retention, SweepReport};
use dettivo_storage::{Store, StoreError};
use serde_json::{Map, Value};

/// The service.
pub struct History {
    store: Mutex<Store>,
    artifacts: Artifacts,
    meetings: MeetingArtifacts,
    retention: RwLock<Retention>,
    busy: Mutex<HashSet<String>>,
    /// The exclusive lock on `<db>.lock`, held for the daemon's life: a
    /// second daemon on the same data fails here, before recovery touches
    /// a row or a take the first one is still working on.
    _owner: std::fs::File,
}

/// Takes the exclusive advisory lock on `<db>.lock` and records this
/// pid in it, or names the daemon that holds it: the message carries the
/// holder's pid and its socket, the same words a socket collision uses.
fn own_store(db: &Path, socket: &Path) -> Result<std::fs::File, StoreError> {
    use std::io::{Read, Seek, Write};
    use std::os::fd::AsRawFd;
    if let Some(parent) = db.parent() {
        std::fs::create_dir_all(parent).map_err(|e| StoreError::Io(e.to_string()))?;
    }
    let lock_path = db.with_extension("db.lock");
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| StoreError::Io(format!("{}: {e}", lock_path.display())))?;
    // SAFETY: a valid descriptor this function owns; flock has no other
    // preconditions.
    #[allow(unsafe_code)]
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        let mut holder = String::new();
        let _ = file.read_to_string(&mut holder);
        let pid = match holder.trim().parse::<u32>() {
            Ok(pid) => format!(" (pid {pid})"),
            Err(_) => String::new(),
        };
        return Err(StoreError::Io(format!(
            "dettivod is already running{pid} on {}; it owns {}",
            socket.display(),
            db.display()
        )));
    }
    let _ = file.set_len(0);
    let _ = file.rewind();
    let _ = writeln!(file, "{}", std::process::id());
    Ok(file)
}

/// The `[history]` keys as the store reads them.
pub fn retention_of(loaded: &Loaded) -> Retention {
    let h = &loaded.config.history;
    Retention {
        keep_audio: h.keep_audio,
        audio_retention_days: h.audio_retention_days,
        max_items: h.max_items,
        artifacts: ArtifactPolicy::parse(&h.artifacts).unwrap_or_else(|| {
            tracing::warn!("history.artifacts is not keep, audio_only or none; keeping artifacts");
            ArtifactPolicy::Keep
        }),
    }
}

/// A store error as the contract reports it.
pub fn store_error(e: StoreError) -> JsonRpcError {
    match e {
        StoreError::Busy => {
            let mut details = Map::new();
            details.insert("reason".into(), Value::String("database_busy".into()));
            JsonRpcError::with_data(
                "the history database is busy",
                ErrorData::with_retryable(AppCode::InternalError, true, ErrorDetails(details)),
            )
        }
        StoreError::NotFound(id) => JsonRpcError::new(
            AppCode::NotFound,
            format!("no dictation {id}"),
            ErrorDetails::empty(),
        ),
        StoreError::InvalidQuery(m) => {
            JsonRpcError::new(AppCode::InvalidParams, m, ErrorDetails::empty())
        }
        other => JsonRpcError::new(
            AppCode::InternalError,
            other.to_string(),
            ErrorDetails::empty(),
        ),
    }
}

impl History {
    /// Opens the database named by `[history] db_path` (default
    /// `<data_dir>/dettivo.db`) and marks any item a lost job left in
    /// `transcribing` as failed. The store is owned first: a daemon that
    /// cannot take the lock has a live sibling and exits before touching
    /// a row.
    pub fn open(paths: &Paths, loaded: &Loaded) -> Result<Self, StoreError> {
        let db = loaded.db_path(paths);
        let owner = own_store(&db, &loaded.socket(paths))?;
        let store = Store::open(&db)?;
        let stale = store.fail_stale_jobs("the daemon restarted before the job finished")?;
        if stale > 0 {
            tracing::warn!(stale, "history: unfinished jobs marked failed");
        }
        let stale =
            store.fail_stale_meeting_jobs("the daemon restarted before the job finished")?;
        if stale > 0 {
            tracing::warn!(stale, "history: unfinished meeting jobs marked failed");
        }
        // The analysis and diarization passes run in threads that die with
        // the process; their `running` rows settle here so both retries
        // come back and nothing waits on a worker that no longer exists.
        let stale =
            store.fail_stale_meeting_passes("the daemon restarted before the pass finished")?;
        if stale > 0 {
            tracing::warn!(stale, "history: unfinished meeting passes marked failed");
        }
        tracing::info!(
            path = %store.path().display(),
            version = store.schema_version().unwrap_or(0),
            "history store open"
        );
        Ok(Self {
            store: Mutex::new(store),
            artifacts: Artifacts::new(loaded.data_dir(paths).join("dictations")),
            meetings: MeetingArtifacts::new(loaded.data_dir(paths).join("meetings")),
            retention: RwLock::new(retention_of(loaded)),
            busy: Mutex::new(HashSet::new()),
            _owner: owner,
        })
    }

    /// Applies a reloaded configuration's `[history]` keys.
    pub fn apply(&self, loaded: &Loaded) {
        *self.retention.write().unwrap_or_else(|p| p.into_inner()) = retention_of(loaded);
    }

    /// Runs `f` against the store.
    pub fn with_store<T>(
        &self,
        f: impl FnOnce(&Store) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let store = self.store.lock().unwrap_or_else(|p| p.into_inner());
        f(&store)
    }

    /// The retention settings in force.
    pub fn retention(&self) -> Retention {
        self.retention
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// The item directories.
    pub fn artifacts(&self) -> &Artifacts {
        &self.artifacts
    }

    /// The meeting directories.
    pub fn meeting_artifacts(&self) -> &MeetingArtifacts {
        &self.meetings
    }

    /// Deletes a meeting's directory and then its row: a directory that
    /// cannot be removed keeps the row addressable, so a retry through
    /// the API finishes the delete instead of answering `NOT_FOUND`.
    pub fn delete_meeting(&self, id: &str) -> Result<MeetingRow, StoreError> {
        self.with_store(|store| store.get_meeting(id))?
            .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
        self.meetings.remove(id)?;
        self.with_store(|store| store.delete_meeting_row(id))
    }

    /// Inserts an imported meeting (status `transcribing`) and marks it busy.
    pub fn begin_meeting(&self, row: &MeetingRow) -> Result<(), StoreError> {
        self.with_store(|store| store.insert_meeting(row))?;
        self.mark_busy(&row.id);
        Ok(())
    }

    /// Finishes an imported meeting with the texts and segments the job
    /// produced (kept on failure too), or the error.
    pub fn finish_meeting(
        &self,
        row: &mut MeetingRow,
        item: &DictationItem,
        outcome: Result<(), (String, String)>,
        archive: Option<&dyn dettivo_meeting::Archive>,
    ) -> Result<(), StoreError> {
        row.raw_text = item.raw_text.clone();
        row.final_text = item.final_text.clone();
        row.segments = item.segments.clone();
        row.duration_ms = item.duration_ms;
        if !item.language.is_empty() {
            row.language = item.language.clone();
        }
        row.ended_at = Some(dettivo_storage::time::now_iso());
        match outcome {
            Ok(()) => {
                row.status = MeetingStatus::Completed;
                row.error_code = None;
                row.error_message = None;
                if let Some(a) = archive {
                    a.completed(row);
                }
            }
            Err((code, message)) => {
                row.status = if code == crate::jobs_run::CANCELLED {
                    MeetingStatus::Cancelled
                } else {
                    MeetingStatus::Failed
                };
                row.error_code = Some(code);
                row.error_message = Some(message);
            }
        }
        self.with_store(|store| store.update_meeting(row))?;
        Ok(())
    }

    /// Marks an item as in use by a job (the sweep leaves it alone).
    pub fn mark_busy(&self, id: &str) {
        self.busy
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id.to_string());
    }

    /// Releases an item a job finished with.
    pub fn clear_busy(&self, id: &str) {
        self.busy
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id);
    }

    /// The items jobs are working on.
    pub fn busy(&self) -> HashSet<String> {
        self.busy.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Deletes an item per the artifact policy.
    pub fn delete(&self, id: &str) -> Result<DictationItem, StoreError> {
        let policy = self.retention().artifacts;
        self.with_store(|store| retention::delete_item(store, &self.artifacts, policy, id))
    }

    /// The hourly sweep.
    pub fn sweep(&self) -> Result<SweepReport, StoreError> {
        let retention = self.retention();
        let busy = self.busy();
        self.with_store(|store| retention::sweep(store, &self.artifacts, &retention, None, &busy))
    }

    /// Seeds the QA rows (`DETTIVO_E2E_SEED=1`); returns how many were new.
    /// With `keep_audio` on and the age limit off (`audio_retention_days
    /// = 0`; the rows are dated in February, so any limit would sweep the
    /// take at once), the seed's re-runnable item gets one second of
    /// retained silence beside the database and its `audio_path`.
    pub fn seed(&self) -> Result<usize, StoreError> {
        let added = self.with_store(dettivo_storage::seed::apply)?;
        let retention = self.retention();
        if retention.keep_audio
            && retention.artifacts != ArtifactPolicy::None
            && retention.audio_retention_days == 0
        {
            let id = dettivo_storage::seed::AUDIO_ITEM_ID;
            let target = self
                .artifacts
                .dir(id)
                .join(dettivo_storage::retention::AUDIO_FILE);
            if !target.is_file() {
                dettivo_storage::seed::write_silence(&target)?;
            }
            let path = target.to_string_lossy().into_owned();
            self.with_store(|store| match store.get(id)? {
                Some(item) if item.audio_path.as_deref() != Some(path.as_str()) => {
                    store.attach_audio(id, &path)
                }
                _ => Ok(()),
            })?;
        }
        Ok(added)
    }

    /// Inserts a job's item (status `transcribing`) and marks it busy.
    pub fn begin_item(&self, item: &DictationItem) -> Result<(), StoreError> {
        self.with_store(|store| store.insert(item))?;
        self.mark_busy(&item.id);
        Ok(())
    }

    /// Finishes a job's item: the texts on success, the error otherwise.
    pub fn finish_item(
        &self,
        item: &mut DictationItem,
        outcome: Result<(), (String, String)>,
    ) -> Result<(), StoreError> {
        match outcome {
            Ok(()) => {
                item.status = ItemStatus::Completed;
                item.error_code = None;
                item.error_message = None;
            }
            Err((code, message)) => {
                item.status = ItemStatus::Failed;
                item.error_code = Some(code);
                item.error_message = Some(message);
            }
        }
        self.with_store(|store| store.update(item))?;
        let retention = self.retention();
        if let Err(e) = self.artifacts.write_metadata(item, &retention) {
            tracing::debug!(error = %e, "history: metadata not written");
        }
        Ok(())
    }

    /// `transcripts.stats`.
    pub fn stats(&self) -> Result<dettivo_proto::methods::transcripts::StatsResult, StoreError> {
        self.with_store(|store| {
            let audio_items = store.with_audio_before("9999-12-31T23:59:59Z")?.len() as u64;
            Ok(dettivo_proto::methods::transcripts::StatsResult {
                path: store.path().to_string_lossy().into_owned(),
                size_bytes: store.size_bytes(),
                item_count: store.count()?,
                audio_items,
                schema_version: store.schema_version()?,
                last_migration: store.last_migration()?,
            })
        })
    }
}

/// The item a finished session transcript becomes.
pub fn item_from(transcript: &Transcript) -> DictationItem {
    let mut item = DictationItem::new(SourceKind::Dictation);
    item.id = transcript.id.clone();
    if let Some(insertion) = &transcript.insertion {
        item.app_id = insertion.target_app.bundle_id.clone();
        item.app_name = insertion.target_app.name.clone();
        item.insertion = serde_json::to_value(insertion).ok();
    }
    item.mode = transcript.policy.mode.clone();
    item.stt_provider = transcript.policy.provider.clone();
    item.stt_model = transcript.policy.model.clone();
    item.language = transcript.language.clone();
    item.raw_text = transcript.raw_text.clone();
    item.final_text = transcript.text.clone();
    item.duration_ms = transcript.duration_ms;
    item.policy_hash = transcript.policy_hash.clone();
    item.timings = transcript.timings.map(|t| dettivo_storage::item::Timings {
        capture_ms: t.capture_ms,
        transcribe_ms: t.transcribe_ms,
        insert_ms: t.insert_ms,
    });
    item.notice = transcript
        .notice
        .as_ref()
        .and_then(|n| serde_json::to_value(n).ok());
    item.with_derived_title()
}

impl dettivo_meeting::Archive for History {
    fn started(&self, row: &MeetingRow) -> Result<(), String> {
        self.with_store(|store| store.insert_meeting(row))
            .map_err(|e| e.to_string())?;
        tracing::info!(id = %row.id, "history: meeting stored");
        Ok(())
    }

    fn updated(&self, row: &MeetingRow) -> Result<(), String> {
        self.with_store(|store| store.update_meeting(row))
            .map_err(|e| e.to_string())
    }
}

impl Archive for History {
    fn archive(&self, transcript: &Transcript, take_dir: &Path) -> Result<(), String> {
        let retention = self.retention();
        let mut item = item_from(transcript);
        let new_artifacts = !self.artifacts.dir(&item.id).exists();
        // A take that cannot be retained (disk full, a moved directory) is
        // logged and the item is stored without audio: the transcript is
        // what must never be lost.
        match self.artifacts.retain_take(&item.id, take_dir, &retention) {
            Ok(path) => item.audio_path = path.map(|p| p.to_string_lossy().into_owned()),
            Err(e) => tracing::warn!(error = %e, "history: take not retained"),
        }
        if let Err(error) = self.with_store(|store| store.insert(&item)) {
            if new_artifacts {
                if let Err(cleanup) = self.artifacts.remove(&item, ArtifactPolicy::Keep) {
                    tracing::warn!(error = %cleanup, "history: failed archive cleanup");
                }
            }
            return Err(error.to_string());
        }
        if let Err(e) = self.artifacts.write_metadata(&item, &retention) {
            tracing::debug!(error = %e, "history: metadata not written");
        }
        tracing::info!(id = %item.id, audio = item.audio_path.is_some(), "history: dictation stored");
        Ok(())
    }
}

#[cfg(test)]
#[path = "history_archive_tests.rs"]
mod archive_tests;
