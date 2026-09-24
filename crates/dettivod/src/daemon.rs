//! Shared daemon state: the resolved paths, the configuration as loaded
//! (swapped whole on reload), the start instant and the state file.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use dettivo_audio::service::AudioService;
use dettivo_core::config::{Loaded, LogLevel};
use dettivo_core::paths::Paths;
use dettivo_core::qa::QaEnv;
use dettivo_core::state::State;
use dettivo_insert::{Mock, Session, Settings};
use dettivo_proto::capabilities::IpcMode;

use crate::analysis::Analysis;
use crate::diarization::Diarization;
use crate::dictation::Dictation;
use crate::engines::Engines;
use crate::events::EventBus;
use crate::feedback::Feedback;
use crate::history::History;
use crate::hotkeys::Hotkeys;
use crate::inserter::ChainInserter;
use crate::jobs::Jobs;
use crate::meetings::Meetings;
use crate::meetings_archive::MeetingArchive;
use crate::models::Models;
use crate::self_target::SelfTargetAllowance;
use crate::transfers::Transfers;

/// Runs blocking desktop work (Wayland, X11, D-Bus, helper commands)
/// without stalling the runtime's workers.
pub fn blocking<T>(work: impl FnOnce() -> T) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(work)
        }
        _ => work(),
    }
}

/// Everything a handler needs, behind one `Arc`.
pub struct Daemon {
    /// The resolved base paths (environment and defaults).
    pub paths: Paths,
    /// The text insertion service (ADR 0007); the dictation session
    /// inserts through the same chain.
    pub insert: Arc<dettivo_insert::Service>,
    config: Arc<RwLock<Loaded>>,
    /// The authentication mode and the token it expects, swapped as one
    /// on every reload and read as one per request.
    auth: RwLock<(IpcMode, Option<String>)>,
    audio: Option<AudioService>,
    started: Instant,
    engines: Engines,
    models: Models,
    bus: Arc<EventBus>,
    dictation: Dictation,
    meetings: Meetings,
    analysis: Arc<Analysis>,
    meeting_archive: Arc<MeetingArchive>,
    history: Arc<History>,
    transfers: Transfers,
    jobs: Arc<Jobs>,
    diarization: Arc<Diarization>,
    hotkeys: Hotkeys,
    self_target: Arc<SelfTargetAllowance>,
    qa_mode: bool,
    rest: crate::rest::State,
    /// Held across every read-modify-write of `config.toml` and every
    /// reload, so two connections editing at once cannot lose each
    /// other's change and the watcher cannot install a stale file.
    config_edits: Mutex<()>,
    /// The one capture reservation: a dictation, meeting or hotkey start
    /// holds it from its exclusion checks until its session is installed,
    /// so two starts of different kinds cannot both see idle and proceed.
    capture_starts: Mutex<()>,
}

impl Daemon {
    /// Loads the configuration file once; the watcher reloads it later.
    /// The shared token (peer_token mode) is resolved here and on every
    /// reload, never per connection, so a reconnecting client cannot make
    /// the daemon shell out to the Secret Service repeatedly.
    pub fn new(paths: Paths, qa: &QaEnv) -> Result<Self, String> {
        let loaded = Loaded::load(&paths.config_file, |k| std::env::var_os(k));
        if let Some(err) = &loaded.error {
            // A file that exists and does not parse stops the start: the
            // defaults would drop `peer_token` and the retention the file
            // asked for. The refusal names the key and the line, never
            // the value.
            return Err(format!(
                "{} is invalid at key {} line {}; fix it (`dettivo config edit`) and start again",
                paths.config_file.display(),
                err.key.as_deref().unwrap_or("-"),
                err.line.unwrap_or(0)
            ));
        }
        let history =
            Arc::new(History::open(&paths, &loaded).map_err(|e| format!("history store: {e}"))?);
        let auth = (
            loaded.config.ipc.auth_mode,
            Self::resolve_token_for(&paths, &loaded),
        );
        let models = Models::new(&paths, &loaded);
        let engines = Engines::new(&paths, &loaded, qa.force_cpu, models.verifier());
        let bus = Arc::new(EventBus::new());
        let hook_bus = bus.clone();
        engines.set_hook(Arc::new(
            move |t: dettivo_speech::supervisor::EngineTransition| {
                hook_bus.publish(
                    dettivo_proto::events::Topic::EngineState,
                    serde_json::json!(dettivo_proto::events::EngineStatePayload {
                        binary: t.binary,
                        state: t.state.to_string(),
                        model: t.model,
                        backend: t.backend.map(|b| format!("{b:?}").to_lowercase()),
                        reason: t.reason,
                    }),
                );
            },
        ));
        let qa_dir = paths
            .state_file
            .parent()
            .map(|d| d.join("qa"))
            .unwrap_or_else(|| PathBuf::from("qa"));
        let mock = qa.mock_insert.then(|| Mock {
            inserted_file: qa_dir.join("inserted.txt"),
        });
        let mock_focus = qa.mock_a11y;
        let insert = Arc::new(dettivo_insert::Service::new(move || {
            Session::from_env(mock.clone(), mock_focus)
        }));
        let config = Arc::new(RwLock::new(loaded));
        // The mock microphone is the mock audio path: cues are logged, not
        // played, so a test can prove which sounds a session asked for.
        let sounds_log = (qa.mock_mode || qa.mock_mic.is_some()).then(|| qa_dir.join("sounds.txt"));
        let feedback = Arc::new(Feedback::new(
            config.clone(),
            paths.runtime_dir.join("sounds"),
            sounds_log,
        ));
        let self_target = Arc::new(SelfTargetAllowance::default());
        let inserter = Arc::new(ChainInserter::new(
            insert.clone(),
            config.clone(),
            self_target.clone(),
        ));
        let dictation = Dictation::new(&paths, bus.clone(), inserter, history.clone(), feedback);
        dictation.set_local_llm(engines.local_llm());
        let analysis = Arc::new(Analysis::new(config.clone(), bus.clone(), history.clone()));
        analysis.set_local_llm(engines.analysis_llm());
        let meeting_archive = Arc::new(MeetingArchive::new(
            history.clone(),
            analysis.clone(),
            config.clone(),
        ));
        let meetings = Meetings::new(
            &paths,
            bus.clone(),
            history.meeting_artifacts().clone(),
            meeting_archive.clone(),
        );
        let transfers = Transfers::new(&paths.cache_dir, crate::platform::transfer_caps());
        transfers.set_max_upload(
            config
                .read()
                .unwrap_or_else(|p| p.into_inner())
                .config
                .transfer
                .max_upload_bytes,
        );
        Ok(Self {
            paths,
            insert,
            config,
            auth: RwLock::new(auth),
            audio: None,
            started: Instant::now(),
            engines,
            models,
            bus,
            dictation,
            meetings,
            analysis,
            meeting_archive,
            history,
            transfers,
            jobs: Arc::new(Jobs::default()),
            diarization: Arc::new(Diarization::default()),
            hotkeys: Hotkeys::new(),
            self_target,
            qa_mode: qa.enabled,
            rest: crate::rest::State::default(),
            config_edits: Mutex::new(()),
            capture_starts: Mutex::new(()),
        })
    }

    /// Connects the audio service (PipeWire); without PipeWire the daemon
    /// runs on, and `audio.devices` says so.
    pub fn with_audio(mut self) -> Self {
        match AudioService::start() {
            Ok(service) => {
                tracing::info!("audio: PipeWire connected");
                self.audio = Some(service);
            }
            Err(e) => tracing::warn!(error = %e, "audio: PipeWire unavailable, capture disabled"),
        }
        self
    }

    /// The audio service, when PipeWire answered at start.
    pub fn audio(&self) -> Option<&AudioService> {
        self.audio.as_ref()
    }

    /// The `[insert]` settings in force.
    pub fn insert_settings(&self) -> Settings {
        insert_settings(&self.config())
    }

    /// The hotkey service.
    pub fn hotkeys(&self) -> &Hotkeys {
        &self.hotkeys
    }

    /// The self-target allowance of the first-run Try it step (ADR 0024).
    pub fn self_target(&self) -> &SelfTargetAllowance {
        &self.self_target
    }

    /// True when the daemon started with QA mode on.
    /// The lock a configuration edit holds from reading the file to the
    /// reload after its write.
    pub fn config_edit_lock(&self) -> MutexGuard<'_, ()> {
        self.config_edits
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The capture reservation a session start holds through its
    /// exclusion checks and its start; dropped on every path, so a start
    /// that fails frees the next one.
    pub fn capture_start_lock(&self) -> MutexGuard<'_, ()> {
        self.capture_starts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn qa_mode(&self) -> bool {
        self.qa_mode
    }

    /// The REST shim's listener state (ADR 0028).
    pub fn rest(&self) -> &crate::rest::State {
        &self.rest
    }

    fn resolve_token_for(paths: &Paths, loaded: &Loaded) -> Option<String> {
        match loaded.config.ipc.auth_mode {
            IpcMode::Peer => None,
            IpcMode::PeerToken => {
                crate::auth::resolve_token(&loaded.token_file(paths)).map(|(t, _)| t)
            }
        }
    }

    /// The authentication mode and the token connections compare against
    /// (`None` in peer mode or when no source has one), as one snapshot:
    /// a request never sees the new mode with the old token.
    pub fn auth_snapshot(&self) -> (IpcMode, Option<String>) {
        self.auth
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// The configuration cell itself, for a service that must read the
    /// values in force later (a meeting reopening its microphone on the
    /// device pinned now).
    pub fn config_handle(&self) -> Arc<RwLock<Loaded>> {
        self.config.clone()
    }

    /// A snapshot of the configuration in force.
    pub fn config(&self) -> Loaded {
        self.config
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Re-reads the file and swaps the snapshot under the edit lock, so a
    /// watcher reload never installs an older file over an edit that is
    /// being written. Returns the new snapshot; the watcher hands it to
    /// the hotkey service, which restarts its backend when `[hotkeys]`
    /// changed.
    pub fn reload(&self) -> Loaded {
        let _guard = self.config_edit_lock();
        self.reload_locked()
    }

    /// `reload` for a caller that already holds the edit lock (the
    /// writer that just replaced the file).
    pub(crate) fn reload_locked(&self) -> Loaded {
        let previous = self.config();
        let loaded =
            Loaded::load(&self.paths.config_file, |k| std::env::var_os(k)).retaining(&previous);
        let auth = (
            loaded.config.ipc.auth_mode,
            Self::resolve_token_for(&self.paths, &loaded),
        );
        *self
            .config
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = loaded.clone();
        *self
            .auth
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = auth;
        if previous.config.daemon.log_level != loaded.config.daemon.log_level
            && crate::logging::set_level(loaded.config.daemon.log_level)
        {
            tracing::info!("log level follows the configuration");
        }
        self.models.apply(&self.paths, &loaded);
        self.engines.apply(&self.paths, &loaded);
        self.dictation.set_local_llm(self.engines.local_llm());
        self.analysis.set_local_llm(self.engines.analysis_llm());
        self.history.apply(&loaded);
        self.transfers
            .set_max_upload(loaded.config.transfer.max_upload_bytes);
        self.publish_config_changed(&previous, &loaded);
        loaded
    }

    /// `config.changed` (ADR 0033): the keys whose value or source moved
    /// with this reload; nothing is published when nothing moved.
    fn publish_config_changed(&self, previous: &Loaded, loaded: &Loaded) {
        let before = previous.entries(None, &self.paths).unwrap_or_default();
        let after = loaded.entries(None, &self.paths).unwrap_or_default();
        let keys: Vec<String> = after
            .iter()
            .filter(|entry| {
                before
                    .iter()
                    .find(|b| b.key == entry.key)
                    .is_none_or(|b| b.value != entry.value || b.source != entry.source)
            })
            .map(|entry| entry.key.clone())
            .collect();
        if keys.is_empty() && previous.error == loaded.error {
            return;
        }
        let payload = dettivo_proto::events::ConfigChangedPayload {
            path: self.paths.config_file.to_string_lossy().into_owned(),
            ok: loaded.error.is_none(),
            keys,
        };
        self.bus.publish(
            dettivo_proto::events::Topic::ConfigChanged,
            serde_json::to_value(payload).unwrap_or(serde_json::Value::Null),
        );
    }

    /// The history store.
    pub fn history(&self) -> &Arc<History> {
        &self.history
    }

    /// The transfer service.
    pub fn transfers(&self) -> &Transfers {
        &self.transfers
    }

    /// The background jobs (re-runs, imports).
    pub fn jobs(&self) -> &Arc<Jobs> {
        &self.jobs
    }

    /// The post-meeting speaker pass (ADR 0035).
    pub fn diarization(&self) -> &Arc<Diarization> {
        &self.diarization
    }

    /// The model service.
    pub fn models(&self) -> &Models {
        &self.models
    }

    /// The engine supervisor.
    pub fn engines(&self) -> &Engines {
        &self.engines
    }

    /// The event bus.
    pub fn bus(&self) -> &Arc<EventBus> {
        &self.bus
    }

    /// The dictation service.
    pub fn dictation(&self) -> &Dictation {
        &self.dictation
    }

    /// The meeting service.
    pub fn meetings(&self) -> &Meetings {
        &self.meetings
    }

    /// The meeting analysis service.
    pub fn analysis(&self) -> &Arc<Analysis> {
        &self.analysis
    }

    /// The archive the meeting session and the meeting import write through.
    pub fn meeting_archive(&self) -> &Arc<MeetingArchive> {
        &self.meeting_archive
    }

    /// The configured log level (defaults apply when the file is broken).
    pub fn log_level(&self) -> LogLevel {
        self.config().config.daemon.log_level
    }

    /// The authentication mode in force.
    pub fn auth_mode(&self) -> IpcMode {
        self.auth_snapshot().0
    }

    /// The socket path in force: `--socket`/environment, file, default.
    pub fn socket_path(&self) -> PathBuf {
        self.config().socket(&self.paths)
    }

    /// The pid file beside the socket, which names the running instance
    /// to a second start.
    pub fn pid_file(&self) -> PathBuf {
        Paths::socket_dir(&self.socket_path()).join("dettivod.pid")
    }

    /// Seconds since the process started.
    pub fn uptime_seconds(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    /// True when the configuration file on disk parsed (or is absent);
    /// false while an edit that broke it is waiting to be fixed and the
    /// last valid values stay in force.
    pub fn config_ok(&self) -> bool {
        self.config().error.is_none()
    }

    /// Logs the configuration state after logging is up: the path.
    pub fn report_config_state(&self) {
        let loaded = self.config();
        tracing::info!(path = %loaded.path.display(), "configuration loaded");
    }

    /// Records the start in the state file (never in `config.toml`).
    pub fn record_start(&self) {
        let path = &self.paths.state_file;
        let mut state = match State::load(path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "state file unreadable, starting fresh");
                State::default()
            }
        };
        state.daemon.last_started_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        state.daemon.start_count += 1;
        if let Err(e) = state.save(path) {
            tracing::warn!(path = %path.display(), error = %e, "state file not written");
        }
    }
}

/// The `[insert]` settings of a configuration snapshot.
pub fn insert_settings(loaded: &Loaded) -> Settings {
    let insert = loaded.config.insert.clone();
    Settings {
        backend: insert.backend,
        paste_keys: insert.paste_keys,
        terminal_app_ids: insert.terminal_app_ids,
        self_app_ids: insert.self_app_ids,
        inter_key_delay_ms: insert.inter_key_delay_ms,
        restore_clipboard: insert.restore_clipboard,
        clipboard_restore_delay_ms: insert.clipboard_restore_delay_ms,
        undo_window_ms: insert.undo_window_ms,
    }
}
