//! The dictation service the `dictation.*` handlers drive: the session
//! machine from `dettivo-session`, the policy frozen from the config in
//! force (the speech keys, and the `[polish]` and `[llm]` sections the
//! mode step runs under, ADR 0023), the audio source (PipeWire, or the `DETTIVO_MOCK_MIC` fixture),
//! the engine from the supervisor, and the publisher that feeds the event
//! bus with `dictation.state` and `audio.level`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

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
use dettivo_session::source::{Source, SourceFactory};
use dettivo_session::{
    Archive, Inserter, Level, Policy, Publisher, SessionError, SessionTarget, StateChange,
    Transcript,
};
use serde_json::json;

use crate::events::EventBus;
use crate::feedback::Feedback;

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

/// Opens the session's audio: the fixture named by `DETTIVO_MOCK_MIC`
/// when set, else a PipeWire capture on the pinned device or the default.
struct DaemonSource {
    audio: Option<AudioService>,
    mock: Option<PathBuf>,
    input_device: String,
}

impl SourceFactory for DaemonSource {
    fn open(&self, level_interval_ms: u64) -> Result<Source, String> {
        if let Some(path) = &self.mock {
            return MockCapture::open(path, level_interval_ms)
                .map(Source::Mock)
                .map_err(|e| format!("mock microphone: {e}"));
        }
        let Some(audio) = &self.audio else {
            return Err("PipeWire is not available".into());
        };
        let target = if self.input_device.is_empty() {
            Target::Default
        } else {
            Target::Node(self.input_device.clone())
        };
        audio
            .open(target, level_interval_ms)
            .map(Source::Capture)
            .map_err(|e| e.to_string())
    }
}

/// The service.
pub struct Dictation {
    session: Session,
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
        Self { session }
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
            silence_peak_threshold: d.silence_peak_threshold as f32,
            keep_audio: loaded.config.history.keep_audio,
            level_interval_ms: loaded.config.audio.level_interval_ms.max(1),
            config_hash: config_hash(loaded, paths),
            rules: RulesConfig::from_config(&loaded.config.polish),
            llm: loaded.config.llm.clone(),
        }
    }

    /// Starts a session with the policy, engine, source and the window
    /// captured at start.
    pub fn start(
        &self,
        policy: Policy,
        engine: Arc<dyn dettivo_speech::SttEngine>,
        audio: Option<AudioService>,
        input_device: &str,
        target: Option<SessionTarget>,
    ) -> Result<Snapshot, SessionError> {
        let mock = std::env::var_os("DETTIVO_MOCK_MIC")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty());
        if let Some(m) = &mock {
            tracing::info!(fixture = %m.display(), "dictation uses the mock microphone");
        }
        let source = Arc::new(DaemonSource {
            audio,
            mock,
            input_device: input_device.to_string(),
        });
        self.session.start(policy, engine, source, target)
    }

    /// Installs the local language model the Enhanced pass reaches; a
    /// session starting afterwards freezes it (ADR 0026).
    pub fn set_local_llm(&self, engine: Arc<dyn dettivo_language::provider::LocalEngine>) {
        self.session.set_local_llm(Some(engine));
    }

    /// Stops and finishes the session.
    pub fn stop(&self) -> Result<Transcript, SessionError> {
        self.session.stop()
    }

    /// Stops the named take without affecting a replacement session.
    pub fn stop_expected(&self, expected_job_id: &str) -> Result<Transcript, SessionError> {
        self.session.stop_expected(Some(expected_job_id))
    }

    /// Cancels the session.
    pub fn cancel(&self) -> Result<Snapshot, SessionError> {
        self.session.cancel()
    }

    /// Cancels the expected job without affecting a replacement session.
    pub fn cancel_expected(&self, expected_job_id: Option<&str>) -> Result<Snapshot, SessionError> {
        self.session.cancel_expected(expected_job_id)
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
