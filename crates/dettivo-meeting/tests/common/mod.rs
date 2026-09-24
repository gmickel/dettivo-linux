//! What the session tests share: a publisher that logs, an archive that
//! keeps every row, channel sources the test feeds, a fake engine that
//! answers every window with a fresh numbered word, and the policy.

#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dettivo_audio::Event;
use dettivo_engine_proto::{Backend, RecognizeResult, Segment};
use dettivo_meeting::machine::Session;
use dettivo_meeting::source::{Source, Sources};
use dettivo_meeting::{Archive, FinalizeProgress, Level, Policy, Publisher, State, StateChange};
use dettivo_speech::{Capabilities, EngineError, PreloadSource, RecognizeRequest, SttEngine};
use dettivo_storage::meetings::{MeetingArtifacts, MeetingRow};
use dettivo_storage::retention::ArtifactPolicy;
use dettivo_transcribe::live::{LiveSegment, LiveSettings};

#[derive(Default)]
pub struct Log {
    pub states: Mutex<Vec<StateChange>>,
    pub levels: Mutex<Vec<Level>>,
    pub segments: Mutex<Vec<(String, LiveSegment)>>,
    pub progress: Mutex<Vec<(String, FinalizeProgress)>>,
}

impl Publisher for Log {
    fn state(&self, change: &StateChange) {
        self.states.lock().unwrap().push(change.clone());
    }
    fn level(&self, level: Level) {
        self.levels.lock().unwrap().push(level);
    }
    fn segment(&self, meeting_id: &str, segment: &LiveSegment) {
        self.segments
            .lock()
            .unwrap()
            .push((meeting_id.to_string(), segment.clone()));
    }
    fn progress(&self, job_id: &str, _meeting_id: &str, progress: FinalizeProgress) {
        self.progress
            .lock()
            .unwrap()
            .push((job_id.to_string(), progress));
    }
}

/// Every row the session stored; `refuse_completed` makes the store
/// refuse a completed row, the way a full disk or a locked database
/// would at the end of a finalisation.
#[derive(Default)]
pub struct Rows {
    pub rows: Mutex<Vec<MeetingRow>>,
    pub refuse_completed: std::sync::atomic::AtomicBool,
}

impl Archive for Rows {
    fn started(&self, row: &MeetingRow) -> Result<(), String> {
        self.rows.lock().unwrap().push(row.clone());
        Ok(())
    }
    fn updated(&self, row: &MeetingRow) -> Result<(), String> {
        if row.status == dettivo_storage::meetings::MeetingStatus::Completed
            && self.refuse_completed.load(Ordering::SeqCst)
        {
            return Err("database is locked".into());
        }
        self.rows.lock().unwrap().push(row.clone());
        Ok(())
    }
}

impl Rows {
    pub fn last(&self) -> MeetingRow {
        self.rows.lock().unwrap().last().unwrap().clone()
    }
}

/// Channel sources: every open hands out a fresh channel and keeps its
/// sender, so the test feeds each track and each reopened microphone.
pub struct Feeds {
    mics: Mutex<Vec<Sender<Event>>>,
    systems: Mutex<Vec<Sender<Event>>>,
    reopen_fails: bool,
    no_system: bool,
    system_delay: Duration,
}

impl Feeds {
    pub fn new(reopen_fails: bool, no_system: bool) -> Self {
        Self {
            mics: Mutex::new(Vec::new()),
            systems: Mutex::new(Vec::new()),
            reopen_fails,
            no_system,
            system_delay: Duration::ZERO,
        }
    }

    /// The system track takes `delay` to open, the way a PipeWire monitor
    /// stream negotiates after the microphone is already capturing.
    pub fn with_system_delay(mut self, delay: Duration) -> Self {
        self.system_delay = delay;
        self
    }
    pub fn mic(&self, n: usize) -> Sender<Event> {
        self.mics.lock().unwrap()[n].clone()
    }
    pub fn system(&self) -> Sender<Event> {
        self.systems.lock().unwrap()[0].clone()
    }
}

fn channel(list: &Mutex<Vec<Sender<Event>>>) -> Source {
    let (tx, rx) = mpsc::channel();
    list.lock().unwrap().push(tx);
    Source::Channel {
        events: rx,
        on_stop: Box::new(|| {}),
    }
}

impl Sources for Feeds {
    fn open_microphone(&self, _level: u64, reopen: bool) -> Result<Source, String> {
        if reopen && self.reopen_fails {
            return Err("no source left".into());
        }
        Ok(channel(&self.mics))
    }
    fn open_system(&self, _level: u64) -> Result<Source, String> {
        if self.no_system {
            return Err("no default sink".into());
        }
        std::thread::sleep(self.system_delay);
        Ok(channel(&self.systems))
    }
}

/// An engine that answers every request with one segment `word<n>`
/// starting half a second in, after an optional delay; `timestamps`
/// off makes it answer text without segments like a provider that has
/// none.
pub struct FakeEngine {
    pub calls: AtomicU64,
    pub delay: Duration,
    pub timestamps: bool,
    pub lengths: Mutex<Vec<u64>>,
}

impl FakeEngine {
    pub fn new(delay: Duration) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicU64::new(0),
            delay,
            timestamps: true,
            lengths: Mutex::new(Vec::new()),
        })
    }
    pub fn without_timestamps() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicU64::new(0),
            delay: Duration::ZERO,
            timestamps: false,
            lengths: Mutex::new(Vec::new()),
        })
    }
}

impl SttEngine for FakeEngine {
    fn provider(&self) -> &str {
        "fake"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_timestamps: self.timestamps,
            supports_streaming: false,
            supported_languages: Vec::new(),
            max_duration_seconds: 3600,
            supports_custom_vocabulary: false,
            supports_model_deletion: false,
        }
    }
    fn preload(&self, _source: PreloadSource) -> Result<(), EngineError> {
        Ok(())
    }
    fn recognize(
        &self,
        request: RecognizeRequest,
        _timeout: Duration,
    ) -> Result<RecognizeResult, EngineError> {
        std::thread::sleep(self.delay);
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        let len_ms = request.pcm.len() as u64 * 1000 / 16_000;
        self.lengths.lock().unwrap().push(len_ms);
        let text = format!("word{n}");
        Ok(RecognizeResult {
            text: text.clone(),
            language: "en".into(),
            segments: if self.timestamps {
                vec![Segment {
                    start_ms: 500.min(len_ms.saturating_sub(1)),
                    end_ms: len_ms,
                    text,
                    words: Vec::new(),
                }]
            } else {
                Vec::new()
            },
            duration_ms: len_ms,
            backend: Backend::Cpu,
        })
    }
    fn cancel(&self) {}
    fn backend(&self) -> Option<Backend> {
        Some(Backend::Cpu)
    }
}

pub fn policy() -> Policy {
    Policy {
        keep_audio: true,
        artifacts: ArtifactPolicy::Keep,
        checkpoint_interval: Duration::from_millis(300),
        level_interval_ms: 50,
        live: true,
        tuning: LiveSettings::default(),
        transcribe: dettivo_transcribe::Settings::default(),
        prompt: None,
    }
}

pub fn setup(feeds: Feeds) -> (Session, Arc<Log>, Arc<Rows>, Arc<Feeds>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let log = Arc::new(Log::default());
    let rows = Arc::new(Rows::default());
    let session = Session::new(
        MeetingArtifacts::new(dir.path().join("meetings")),
        log.clone(),
        rows.clone(),
    );
    (session, log, rows, Arc::new(feeds), dir)
}

pub fn states(log: &Log) -> Vec<(State, State, Option<String>)> {
    log.states
        .lock()
        .unwrap()
        .iter()
        .map(|c| (c.state, c.previous, c.reason.clone()))
        .collect()
}

/// A square wave at `amplitude` for `ms` milliseconds (speech to the gate).
pub fn tone(ms: u64, amplitude: i16) -> Vec<i16> {
    (0..ms * 16)
        .map(|i| if i % 2 == 0 { amplitude } else { -amplitude })
        .collect()
}
