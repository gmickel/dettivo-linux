//! Shared rig for the state machine tests: a channel-fed source, a
//! scripted engine and a recording publisher and inserter.
#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dettivo_audio::Event;
use dettivo_proto::runtime::{InsertionMethod, InsertionOutcome, InsertionResult, TargetApp};
use dettivo_session::machine::Session;
use dettivo_session::source::{Source, SourceFactory};
use dettivo_session::{
    Inserter, Level, Policy, Publisher, SessionTarget, SessionTimings, State, StateChange,
    no_context_pack,
};
use dettivo_speech::{Capabilities, EngineError, PreloadSource, RecognizeRequest, SttEngine};

#[derive(Default)]
pub struct Recorder {
    pub states: Mutex<Vec<StateChange>>,
    pub levels: AtomicUsize,
    pub inserted: Mutex<Vec<String>>,
    pub targets: Mutex<Vec<Option<SessionTarget>>>,
}

impl Publisher for Recorder {
    fn state(&self, change: &StateChange) {
        self.states.lock().unwrap().push(change.clone());
    }
    fn level(&self, _: Level) {
        self.levels.fetch_add(1, Ordering::Relaxed);
    }
}

impl Inserter for Recorder {
    fn insert(&self, text: &str, target: Option<&SessionTarget>) -> InsertionResult {
        self.inserted.lock().unwrap().push(text.to_string());
        self.targets.lock().unwrap().push(target.cloned());
        InsertionResult {
            outcome: InsertionOutcome::Inserted,
            method: InsertionMethod::Paste,
            target_app: TargetApp {
                bundle_id: "test".into(),
                name: "test".into(),
            },
            context_pack: no_context_pack(),
            reason: None,
            backend: None,
        }
    }
}

impl Recorder {
    pub fn sequence(&self) -> Vec<State> {
        self.states
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.state)
            .collect()
    }

    /// The timings carried by the completion change (into `idle` from
    /// `inserting`), when one was published.
    pub fn completion_timings(&self) -> Option<SessionTimings> {
        self.states
            .lock()
            .unwrap()
            .iter()
            .find(|c| c.state == State::Idle && c.previous == State::Inserting)
            .and_then(|c| c.timings)
    }
}

/// An engine that answers with a fixed text, can be told to fail or crash,
/// and honours cancel while it "works".
pub struct FakeEngine {
    pub text: String,
    pub fail: Option<EngineError>,
    pub slow: Duration,
    pub cancelled: AtomicBool,
    pub calls: AtomicUsize,
}

impl FakeEngine {
    pub fn new(text: &str) -> Arc<Self> {
        Arc::new(Self {
            text: text.into(),
            fail: None,
            slow: Duration::ZERO,
            cancelled: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
        })
    }
}

impl SttEngine for FakeEngine {
    fn provider(&self) -> &str {
        "fake"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_timestamps: true,
            supports_streaming: false,
            supported_languages: vec!["en".into()],
            max_duration_seconds: 300,
            supports_custom_vocabulary: true,
            supports_model_deletion: true,
        }
    }
    fn preload(&self, _: PreloadSource) -> Result<(), EngineError> {
        Ok(())
    }
    fn recognize(
        &self,
        request: RecognizeRequest,
        _: Duration,
    ) -> Result<dettivo_engine_proto::RecognizeResult, EngineError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let deadline = std::time::Instant::now() + self.slow;
        while std::time::Instant::now() < deadline {
            if self.cancelled.load(Ordering::Relaxed) {
                return Err(EngineError::Cancelled);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if let Some(e) = &self.fail {
            return Err(e.clone());
        }
        Ok(dettivo_engine_proto::RecognizeResult {
            text: self.text.clone(),
            language: request.language,
            segments: Vec::new(),
            duration_ms: request.pcm.len() as u64 * 1000 / 16_000,
            backend: dettivo_engine_proto::Backend::Cpu,
        })
    }
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    fn backend(&self) -> Option<dettivo_engine_proto::Backend> {
        Some(dettivo_engine_proto::Backend::Cpu)
    }
}

/// A source the test feeds through a channel.
pub struct Feed {
    pub tx: Mutex<Option<Sender<Event>>>,
    pub handle: Mutex<Option<Sender<Event>>>,
    pub stopped: Arc<AtomicBool>,
    pub fail_open: bool,
}

impl Feed {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            tx: Mutex::new(None),
            handle: Mutex::new(None),
            stopped: Arc::new(AtomicBool::new(false)),
            fail_open: false,
        })
    }
    pub fn send(&self, e: Event) {
        if let Some(tx) = self.handle.lock().unwrap().as_ref() {
            let _ = tx.send(e);
        }
    }
    pub fn speak(&self, ms: u64) {
        let chunk = vec![3000i16; 320];
        for _ in 0..(ms / 20) {
            self.send(Event::Pcm(chunk.clone()));
        }
        self.send(Event::Level {
            rms: 0.1,
            peak: 0.2,
        });
    }
}

impl SourceFactory for Feed {
    fn open(&self, _: u64) -> Result<Source, String> {
        if self.fail_open {
            return Err("no microphone".into());
        }
        let (tx, rx) = mpsc::channel();
        *self.tx.lock().unwrap() = Some(tx.clone());
        *self.handle.lock().unwrap() = Some(tx.clone());
        self.stopped.store(false, Ordering::Relaxed);
        let stopped = self.stopped.clone();
        Ok(Source::Channel {
            events: rx,
            on_stop: Box::new(move || {
                if !stopped.swap(true, Ordering::Relaxed) {
                    let _ = tx.send(Event::Ended {
                        reason: dettivo_audio::EndReason::Stopped,
                    });
                }
            }),
        })
    }
}

pub fn policy(dir: &std::path::Path, keep: bool) -> Policy {
    let model = dir.join("model.bin");
    std::fs::write(&model, b"fake").unwrap();
    Policy {
        provider: "fake".into(),
        model: "tiny".into(),
        model_path: model,
        language: "en".into(),
        mode: "raw".into(),
        vocabulary: vec!["Dettivo".into()],
        replacements: vec![("teh".into(), "the".into())],
        spoken_punctuation: true,
        protect_tokens: true,
        max_duration: Duration::from_secs(30),
        silence_peak_threshold: 0.01,
        keep_audio: keep,
        level_interval_ms: 50,
        config_hash: "h".into(),
        rules: dettivo_language::policy::RulesConfig::from_config(
            &dettivo_core::config::polish_schema::Polish::default(),
        ),
        llm: dettivo_core::config::polish_schema::Llm::default(),
    }
}

pub fn rig(
    text: &str,
) -> (
    tempfile::TempDir,
    Arc<Recorder>,
    Session,
    Arc<FakeEngine>,
    Arc<Feed>,
) {
    let dir = tempfile::tempdir().unwrap();
    let rec = Arc::new(Recorder::default());
    let session = Session::new(dir.path().join("sessions"), rec.clone(), rec.clone());
    (dir, rec, session, FakeEngine::new(text), Feed::new())
}

pub fn wait_idle(session: &Session) {
    for _ in 0..200 {
        if session.snapshot().is_none() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("session did not end");
}
