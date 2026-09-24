//! The model service: the catalogue, the models directory, running
//! downloads (bounded by `[models] max_concurrent_downloads`), readiness
//! per model from one source (FR-E5), verification at start
//! (`verify_on_start`) and the `model.download` payloads the event stream
//! will carry.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, Weak};

use dettivo_core::config::Loaded;
use dettivo_core::paths::Paths;
use dettivo_proto::events::ModelDownloadPayload;
use dettivo_proto::methods::speech::{ModelReadiness, ModelStatus};
use dettivo_speech::catalogue::{Catalogue, ModelEntry};
use dettivo_speech::download::{self, Download, DownloadState, Progress};
use dettivo_speech::models::{ModelStore, Readiness};

/// The model service behind `speech.models.*`.
pub struct Models {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    /// The service itself, so a finished download can start a queued one.
    self_ref: Weak<Mutex<Inner>>,
    store: ModelStore,
    max_concurrent: usize,
    running: BTreeMap<String, Download>,
    queued: VecDeque<String>,
    /// The last progress per model, kept after a download ends.
    last: BTreeMap<String, Progress>,
    /// Payloads published since the last drain (the event stream's feed).
    published: Vec<ModelDownloadPayload>,
}

fn key(entry: &ModelEntry) -> String {
    format!("{}/{}", entry.provider, entry.id)
}

/// Why a model operation was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// No catalogue entry.
    Unknown(String),
    /// Listed but not offered yet.
    Unavailable(String),
    /// The selected model; deletion needs `force`.
    Selected(String),
    /// A download of the model is still running after a cancel.
    Busy(String),
    /// An I/O failure.
    Io(String),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown(m) => write!(f, "unknown model {m}"),
            Self::Unavailable(m) => write!(f, "{m} is not available for download yet"),
            Self::Selected(m) => write!(f, "{m} is the selected model; pass force to delete it"),
            Self::Busy(m) => write!(f, "{m} is still downloading; cancel it and retry"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl Models {
    /// Builds the service from the configuration in force.
    pub fn new(paths: &Paths, loaded: &Loaded) -> Self {
        let store = ModelStore::new(loaded.models_dir(paths), catalogue_for(loaded));
        Self {
            inner: Arc::new_cyclic(|w| {
                Mutex::new(Inner {
                    self_ref: w.clone(),
                    store,
                    max_concurrent: loaded.config.models.max_concurrent_downloads.max(1) as usize,
                    running: BTreeMap::new(),
                    queued: VecDeque::new(),
                    last: BTreeMap::new(),
                    published: Vec::new(),
                })
            }),
        }
    }

    /// Applies a reloaded configuration: a new models directory or
    /// catalogue replaces the store (running downloads finish where they
    /// started), and the concurrency bound takes effect at the next start.
    pub fn apply(&self, paths: &Paths, loaded: &Loaded) {
        let mut g = self.lock();
        let root = loaded.models_dir(paths);
        if g.store.root() != root.as_path()
            || g.store.catalogue().version != catalogue_for(loaded).version
        {
            g.store = ModelStore::new(root, catalogue_for(loaded));
        }
        g.max_concurrent = loaded.config.models.max_concurrent_downloads.max(1) as usize;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The store (catalogue plus directory) as it is now.
    pub fn store(&self) -> ModelStore {
        self.lock().store.clone()
    }

    /// Re-hashes models on disk without a verified manifest, off the
    /// caller's thread, then runs `then` (the start-up preload) on that
    /// thread; quarantined ones are logged.
    pub fn verify_on_start(&self, then: impl FnOnce() + Send + 'static) {
        let store = self.store();
        std::thread::Builder::new()
            .name("model-verify".into())
            .spawn(move || {
                let quarantined = store.verify_all();
                if !quarantined.is_empty() {
                    tracing::warn!(models = ?quarantined, "models quarantined at start");
                } else {
                    tracing::info!("models on disk verified");
                }
                then();
            })
            .map(|_| ())
            .unwrap_or_else(|e| tracing::warn!(error = %e, "verification thread not started"));
    }

    /// The gate every catalogue-backed load passes (`crate::verify`),
    /// over the store as it is when the load happens.
    pub fn verifier(&self) -> crate::verify::Verifier {
        let inner = self.inner.clone();
        crate::verify::verifier(move || {
            inner
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .store
                .clone()
        })
    }

    /// One row per catalogue model, catalogue order; `selected` marks the
    /// dictation selection.
    pub fn status(
        &self,
        provider: Option<&str>,
        model: Option<&str>,
        selected: (&str, &str),
    ) -> Vec<ModelStatus> {
        let mut g = self.lock();
        g.reap_finished();
        let entries: Vec<ModelEntry> = g
            .store
            .catalogue()
            .models
            .iter()
            .filter(|m| provider.is_none_or(|p| m.provider == p))
            .filter(|m| model.is_none_or(|id| m.id == id))
            .cloned()
            .collect();
        entries.iter().map(|e| g.row(e, selected)).collect()
    }

    /// One model's row.
    pub fn status_of(&self, entry: &ModelEntry, selected: (&str, &str)) -> ModelStatus {
        let mut g = self.lock();
        g.reap_finished();
        g.row(entry, selected)
    }

    /// The catalogue entry, or why not.
    pub fn entry(&self, provider: &str, id: &str) -> Result<ModelEntry, ModelError> {
        self.lock()
            .store
            .catalogue()
            .find(provider, id)
            .cloned()
            .ok_or_else(|| ModelError::Unknown(format!("{provider}/{id}")))
    }

    /// Starts (or joins) a download. Idempotent while it runs; a full
    /// slot queues the model until one frees.
    pub fn download(&self, entry: &ModelEntry) -> Result<(), ModelError> {
        if !entry.available {
            return Err(ModelError::Unavailable(key(entry)));
        }
        let mut g = self.lock();
        g.reap_finished();
        let k = key(entry);
        if g.running.contains_key(&k) || g.queued.contains(&k) {
            return Ok(());
        }
        if g.store.readiness(entry) == Readiness::Ready {
            return Ok(());
        }
        if g.running.len() >= g.max_concurrent {
            g.queued.push_back(k);
            return Ok(());
        }
        g.spawn(entry.clone(), self.inner.clone());
        Ok(())
    }

    /// Stops a running download (its partial file stays) or drops it from
    /// the queue. Returns whether anything was running or queued.
    pub fn cancel(&self, entry: &ModelEntry) -> bool {
        let mut g = self.lock();
        let k = key(entry);
        if let Some(d) = g.running.get(&k) {
            d.cancel();
            return true;
        }
        if let Some(i) = g.queued.iter().position(|q| *q == k) {
            g.queued.remove(i);
            return true;
        }
        false
    }

    /// Deletes the model files; the selected model needs `force`.
    pub fn delete(
        &self,
        entry: &ModelEntry,
        selected: (&str, &str),
        force: bool,
    ) -> Result<bool, ModelError> {
        if !force && (entry.provider.as_str(), entry.id.as_str()) == selected {
            return Err(ModelError::Selected(key(entry)));
        }
        self.cancel(entry);
        let store = self.store();
        // The transfer stops at its next chunk; a read that is stalled on the
        // network can take longer, and then nothing is removed under it.
        if !self.wait_stopped(entry, std::time::Duration::from_secs(3)) {
            return Err(ModelError::Busy(key(entry)));
        }
        store
            .delete(entry)
            .map_err(|e| ModelError::Io(e.to_string()))
    }

    /// Waits until no download of `entry` runs; false when it still does.
    pub fn wait_stopped(&self, entry: &ModelEntry, timeout: std::time::Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            {
                let mut g = self.lock();
                g.reap_finished();
                if !g.running.contains_key(&key(entry)) {
                    return true;
                }
            }
            if std::time::Instant::now() > deadline {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// Takes the payloads published since the last call.
    pub fn drain_published(&self) -> Vec<ModelDownloadPayload> {
        std::mem::take(&mut self.lock().published)
    }

    /// True while any download runs.
    pub fn downloading(&self) -> bool {
        let mut g = self.lock();
        g.reap_finished();
        !g.running.is_empty()
    }

    /// Waits for every running download to end (tests and shutdown).
    pub fn wait_idle(&self, timeout: std::time::Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            if !self.downloading() && self.lock().queued.is_empty() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        false
    }

    /// Cancels every download and waits briefly for the threads.
    pub fn shutdown(&self) {
        {
            let g = self.lock();
            for d in g.running.values() {
                d.cancel();
            }
        }
        self.lock().queued.clear();
        self.wait_idle(std::time::Duration::from_secs(2));
    }
}

impl Inner {
    fn spawn(&mut self, entry: ModelEntry, inner: Arc<Mutex<Inner>>) {
        let k = key(&entry);
        let (provider, model) = (entry.provider.clone(), entry.id.clone());
        let d = download::start(self.store.clone(), entry, move |p: &Progress| {
            let mut g = inner.lock().unwrap_or_else(|e| e.into_inner());
            g.last.insert(format!("{provider}/{model}"), p.clone());
            g.published.push(ModelDownloadPayload {
                provider: provider.clone(),
                model: model.clone(),
                state: match p.state {
                    DownloadState::Running => "running",
                    DownloadState::Done => "done",
                    DownloadState::Cancelled => "cancelled",
                    DownloadState::Failed => "failed",
                    DownloadState::Quarantined => "quarantined",
                }
                .into(),
                bytes_done: p.bytes_done,
                bytes_total: p.bytes_total,
                error: p.error.clone(),
            });
        });
        tracing::info!(model = %k, "model download started");
        self.running.insert(k, d);
    }

    /// Drops finished downloads and starts queued ones.
    fn reap_finished(&mut self) {
        let finished: Vec<String> = self
            .running
            .iter()
            .filter(|(_, d)| !d.running())
            .map(|(k, _)| k.clone())
            .collect();
        for k in finished {
            if let Some(d) = self.running.remove(&k) {
                let p = d.join();
                self.last.insert(k, p);
            }
        }
        while self.running.len() < self.max_concurrent {
            let Some(k) = self.queued.pop_front() else {
                break;
            };
            let entry = k
                .split_once('/')
                .and_then(|(p, id)| self.store.catalogue().find(p, id).cloned());
            if let (Some(entry), Some(inner)) = (entry, self.self_ref.upgrade()) {
                self.spawn(entry, inner);
            }
        }
    }

    fn row(&self, entry: &ModelEntry, selected: (&str, &str)) -> ModelStatus {
        let k = key(entry);
        let disk = self.store.readiness(entry);
        let size = entry.download_bytes();
        let (readiness, bytes_done, bytes_total, error) = match self.running.get(&k) {
            Some(d) => {
                let p = d.progress();
                (
                    ModelReadiness::Downloading,
                    p.bytes_done,
                    p.bytes_total,
                    p.error,
                )
            }
            None => {
                let last = self.last.get(&k);
                let error = last.and_then(|p| p.error.clone());
                match disk {
                    Readiness::Ready => (ModelReadiness::Ready, size, size, None),
                    Readiness::Unverified => (ModelReadiness::Unverified, size, size, None),
                    Readiness::Downloading {
                        bytes_done,
                        bytes_total,
                    } => (ModelReadiness::Downloading, bytes_done, bytes_total, error),
                    Readiness::Partial { bytes_done } => {
                        (ModelReadiness::Partial, bytes_done, size, error)
                    }
                    Readiness::Missing => (ModelReadiness::Missing, 0, size, error),
                    Readiness::Quarantined => (ModelReadiness::Quarantined, 0, size, error),
                }
            }
        };
        let path = (matches!(
            readiness,
            ModelReadiness::Ready | ModelReadiness::Unverified
        ))
        .then(|| self.store.load_path(entry).to_string_lossy().into_owned());
        ModelStatus {
            provider: entry.provider.clone(),
            id: entry.id.clone(),
            display_name: entry.display_name.clone(),
            size_bytes: entry.download_bytes(),
            readiness,
            bytes_done,
            bytes_total,
            path,
            license: entry.license.clone(),
            is_default: entry.default,
            is_selected: (entry.provider.as_str(), entry.id.as_str()) == selected,
            available: entry.available,
            error,
            kind: match entry.kind {
                dettivo_speech::catalogue::ModelKind::Stt => "stt",
                dettivo_speech::catalogue::ModelKind::Vad => "vad",
                dettivo_speech::catalogue::ModelKind::Speaker => "speaker",
                dettivo_speech::catalogue::ModelKind::Llm => "llm",
                dettivo_speech::catalogue::ModelKind::Diarization => "diarization",
            }
            .to_string(),
            languages: entry.languages.clone(),
            recommended_for: entry.recommended_for.clone(),
            source: None,
            format: None,
            base_model: None,
            manifest_error: None,
        }
    }
}

/// The catalogue the configuration names, or the built-in one.
pub(crate) fn catalogue_for(loaded: &Loaded) -> Catalogue {
    let file = &loaded.config.models.catalogue_file;
    if file.is_empty() {
        return Catalogue::builtin();
    }
    match std::fs::read_to_string(file)
        .map_err(|e| e.to_string())
        .and_then(|t| Catalogue::parse(&t))
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(path = %file, error = %e, "catalogue file unusable; using the built-in catalogue");
            Catalogue::builtin()
        }
    }
}
