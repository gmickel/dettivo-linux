//! The dictation service the `dictation.*` handlers drive: the session
//! machine from `dettivo-session`, the policy frozen from the config in
//! force (the speech keys, and the `[polish]` and `[llm]` sections the
//! mode step runs under, ADR 0023), the audio source (PipeWire, or the `DETTIVO_MOCK_MIC` fixture),
//! the engine from the supervisor, the always-ready microphone a take
//! starts from when it is open (ADR 0092), and the publisher that feeds
//! the event bus with `dictation.state` and `audio.level`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dettivo_audio::Target;
use dettivo_audio::mock::MockCapture;
use dettivo_audio::service::AudioService;
use dettivo_core::config::Loaded;
use dettivo_core::paths::Paths;
use dettivo_language::policy::RulesConfig;
use dettivo_proto::events::{
    AudioLevelPayload, DictationInsertion, DictationStatePayload, DictationTimings, LevelSource,
    Topic,
};
use dettivo_proto::methods::polish::PolishNotice;
use dettivo_session::machine::{Session, Snapshot};
use dettivo_session::ready::PRE_ROLL;
use dettivo_session::source::{Source, SourceFactory};
use dettivo_session::{
    Archive, Inserter, Level, POST_RELEASE_TAIL, Policy, Publisher, SessionError, SessionTarget,
    StateChange, Transcript,
};
use dettivo_speech::gate::VoiceDetector;
use serde_json::json;

use crate::daemon::Daemon;
use crate::events::EventBus;
use crate::feedback::Feedback;
use crate::ready::MicReady;

/// Bridges session events onto the bus and to the feedback service.
struct BusPublisher {
    bus: Arc<EventBus>,
    feedback: Arc<Feedback>,
}

impl Publisher for BusPublisher {
    fn state(&self, change: &StateChange) {
        self.feedback.on_state(change);
        tracing::info!(
            job = %change.job_id,
            state = change.state.as_str(),
            previous = change.previous.as_str(),
            reason = change.reason.as_deref().unwrap_or("-"),
            "dictation state"
        );
        let insertion = change.insertion.as_ref().map(|r| DictationInsertion {
            outcome: r.outcome,
            method: r.method,
            backend: r
                .backend
                .as_ref()
                .map(|b| b.name.clone())
                .unwrap_or_default(),
            target_app: r.target_app.clone(),
            reason: r.reason.clone(),
        });
        let timings = change.timings.map(|t| DictationTimings {
            capture_ms: t.capture_ms,
            transcribe_ms: t.transcribe_ms,
            insert_ms: t.insert_ms,
        });
        self.bus.publish(
            Topic::DictationState,
            json!(DictationStatePayload {
                job_id: change.job_id.clone(),
                state: change.state.as_str().to_string(),
                previous_state: change.previous.as_str().to_string(),
                reason: change.reason.clone(),
                insertion,
                first_words: change.first_words.clone(),
                timings,
                mode: change.mode.clone(),
                notice: change.notice.as_ref().map(|n| PolishNotice {
                    kind: n.kind.as_str().to_string(),
                    reason: n.reason.clone(),
                }),
                policy_hash: change.policy_hash.clone(),
            }),
        );
    }

    fn level(&self, level: Level) {
        self.bus.publish(
            Topic::AudioLevel,
            json!(AudioLevelPayload {
                rms: level.rms,
                peak: level.peak,
                source: LevelSource::Microphone,
            }),
        );
    }
}

/// The fixture `DETTIVO_MOCK_MIC` names, when set.
fn mock_microphone() -> Option<PathBuf> {
    std::env::var_os("DETTIVO_MOCK_MIC")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

/// Opens the microphone: the fixture when one is named, else a PipeWire
/// capture on the pinned device or the default.
fn open_capture(
    audio: Option<&AudioService>,
    mock: Option<&Path>,
    input_device: &str,
    level_interval_ms: u64,
) -> Result<Source, String> {
    if let Some(path) = mock {
        return MockCapture::open(path, level_interval_ms)
            .map(Source::Mock)
            .map_err(|e| format!("mock microphone: {e}"));
    }
    let Some(audio) = audio else {
        return Err("PipeWire is not available".into());
    };
    let target = if input_device.is_empty() {
        Target::Default
    } else {
        Target::Node(input_device.to_string())
    };
    audio
        .open(target, level_interval_ms)
        .map(Source::Capture)
        .map_err(|e| e.to_string())
}

/// The session's audio: the take the always-ready microphone handed over,
/// else a capture of its own.
struct DaemonSource {
    ready: Mutex<Option<Source>>,
    audio: Option<AudioService>,
    mock: Option<PathBuf>,
    input_device: String,
}

impl SourceFactory for DaemonSource {
    fn open(&self, level_interval_ms: u64) -> Result<Source, String> {
        if let Some(take) = self.ready.lock().unwrap_or_else(|p| p.into_inner()).take() {
            return Ok(take);
        }
        open_capture(
            self.audio.as_ref(),
            self.mock.as_deref(),
            &self.input_device,
            level_interval_ms,
        )
    }
}

/// The service.
pub struct Dictation {
    session: Session,
    ready: MicReady,
}

impl Dictation {
    /// Takes live under `<state>/sessions/<job>/`; every finished
    /// transcript goes to `inserter` (with the target captured at start),
    /// then to `archive`; `feedback` plays the cues.
    pub fn new(
        paths: &Paths,
        bus: Arc<EventBus>,
        inserter: Arc<dyn Inserter>,
        archive: Arc<dyn Archive>,
        feedback: Arc<Feedback>,
    ) -> Self {
        let sessions_dir = paths
            .state_file
            .parent()
            .map(|p| p.join("sessions"))
            .unwrap_or_else(|| PathBuf::from("sessions"));
        let mut session = Session::new(
            sessions_dir,
            Arc::new(BusPublisher { bus, feedback }),
            inserter,
        );
        session.set_archive(archive);
        Self {
            session,
            ready: MicReady::default(),
        }
    }

    /// The inserter in force (the chain; its mock backend in QA mode).
    pub fn inserter(&self) -> Arc<dyn Inserter> {
        self.session.inserter()
    }

    /// The policy a session starting now would freeze.
    pub fn policy(
        loaded: &Loaded,
        paths: &Paths,
        model_path: PathBuf,
        language: Option<&str>,
        mode: Option<&str>,
    ) -> Policy {
        let d = &loaded.config.dictation;
        let speech = &loaded.config.speech;
        Policy {
            provider: speech.provider.clone(),
            model: speech.model.clone(),
            model_path,
            language: language
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| d.language.clone()),
            mode: mode.map(str::to_string).unwrap_or_else(|| d.mode.clone()),
            vocabulary: d.vocabulary.clone(),
            replacements: d
                .replacements
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            spoken_punctuation: d.spoken_punctuation,
            protect_tokens: d.protect_tokens,
            max_duration: Duration::from_secs(d.max_duration_seconds.max(1)),
            min_hold: dettivo_hotkeys::hold::MIN_HOLD,
            release_tail: POST_RELEASE_TAIL,
            silence_dbfs: d.silence_dbfs as f32,
            keep_audio: loaded.config.history.keep_audio,
            level_interval_ms: loaded.config.audio.level_interval_ms.max(1),
            config_hash: config_hash(loaded, paths),
            rules: RulesConfig::from_config(&loaded.config.polish),
            llm: loaded.config.llm.clone(),
        }
    }

    /// Starts a session for the press that arrived at `pressed_at` with
    /// the policy, engine, the speech gate's voice pass, the daemon's
    /// microphone and the window captured at start. An open always-ready microphone hands
    /// over its audio from [`PRE_ROLL`] before the press.
    pub fn start(
        &self,
        policy: Policy,
        engine: Arc<dyn dettivo_speech::SttEngine>,
        vad: Option<Arc<dyn VoiceDetector>>,
        daemon: &Daemon,
        target: Option<SessionTarget>,
        pressed_at: Instant,
    ) -> Result<Snapshot, SessionError> {
        self.session.set_vad(vad);
        let mock = mock_microphone();
        let ready = self
            .ready
            .take(pressed_at.checked_sub(PRE_ROLL).unwrap_or(pressed_at));
        match (&ready, &mock) {
            (Some(_), _) => tracing::info!("dictation starts from the always-ready microphone"),
            (None, Some(m)) => {
                tracing::info!(fixture = %m.display(), "dictation uses the mock microphone")
            }
            (None, None) => {}
        }
        let source = Arc::new(DaemonSource {
            ready: Mutex::new(ready),
            audio: daemon.audio().cloned(),
            mock,
            input_device: daemon.config().config.audio.input_device,
        });
        let started = self
            .session
            .start(policy, engine, source.clone(), target, pressed_at);
        // A start refused before it opened its source frees the ring now,
        // so the next start finds it idle.
        if let Some(unused) = source
            .ready
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            unused.stop();
        }
        started
    }

    /// The always-ready microphone.
    pub fn ready(&self) -> &MicReady {
        &self.ready
    }

    /// Opens the microphone the always-ready ring drains: `device`, or the
    /// default source when it is empty.
    pub fn open_microphone(&self, daemon: &Daemon, device: &str) -> Result<Source, String> {
        open_capture(
            daemon.audio(),
            mock_microphone().as_deref(),
            device,
            daemon.config().config.audio.level_interval_ms.max(1),
        )
    }

    /// Installs the local language model the Enhanced pass reaches; a
    /// session starting afterwards freezes it (ADR 0026).
    pub fn set_local_llm(&self, engine: Arc<dyn dettivo_language::provider::LocalEngine>) {
        self.session.set_local_llm(Some(engine));
    }

    /// Stops and finishes the session for a release that arrived at
    /// `released_at`.
    pub fn stop(&self, released_at: Instant) -> Result<Transcript, SessionError> {
        self.session.stop_expected(None, released_at)
    }

    /// Stops the named take without affecting a replacement session.
    pub fn stop_expected(
        &self,
        expected_job_id: &str,
        released_at: Instant,
    ) -> Result<Transcript, SessionError> {
        self.session
            .stop_expected(Some(expected_job_id), released_at)
    }

    /// Drops the named take as a tap.
    pub fn discard_tap(&self, expected_job_id: &str) -> Result<Snapshot, SessionError> {
        self.session.discard_tap(expected_job_id)
    }

    /// Cancels the session.
    pub fn cancel(&self) -> Result<Snapshot, SessionError> {
        self.session.cancel()
    }

    /// Cancels the expected job without affecting a replacement session.
    pub fn cancel_expected(&self, expected_job_id: Option<&str>) -> Result<Snapshot, SessionError> {
        self.session.cancel_expected(expected_job_id)
    }

    /// Runs `f` while the take `job_id` records and has not begun to end.
    pub fn while_recording<R>(&self, job_id: &str, f: impl FnOnce() -> R) -> Option<R> {
        self.session.while_recording(job_id, f)
    }

    /// The active session.
    pub fn snapshot(&self) -> Option<Snapshot> {
        self.session.snapshot()
    }

    /// Inserts the last transcript again.
    pub fn reinsert_last(&self) -> Result<Transcript, SessionError> {
        self.session.reinsert_last()
    }
}

/// A stable hash of the keys a session freezes, for log correlation.
fn config_hash(loaded: &Loaded, paths: &Paths) -> String {
    let text = format!(
        "{:?}|{:?}|{:?}|{}",
        loaded.config.speech,
        loaded.config.dictation,
        loaded.config.audio,
        loaded.models_dir(paths).display()
    );
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}
