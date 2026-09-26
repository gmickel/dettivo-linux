//! The state machine: `recording → stopping → stopped → transcribing →
//! completed`, with `cancelled` and `failed` exits and `partial` for a
//! meeting a restart promoted. One meeting records at a time; the active
//! flag is taken under the lock before the microphone opens, so a second
//! `start` is refused even when it races the first. `stop` asks and
//! answers at once with the stopping state (a second call answers the
//! same), the worker closes the takes, reports `stopped`, frees the slot
//! and finalises; a finalisation runs beside the next meeting and is
//! tracked here by meeting id so `status` and `cancel` reach it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use dettivo_speech::SttEngine;
use dettivo_storage::meetings::{MeetingArtifacts, MeetingRow, MeetingStatus};

use crate::source::Sources;
use crate::transcript::LiveTails;
use crate::worker::Worker;
use crate::worker_end::{FinalizeContext, finalize_meeting};
use crate::{Archive, MeetingError, Policy, Publisher, State, StateChange};

/// The public view of the active meeting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// The job id (`job_meeting_<n>`).
    pub job_id: String,
    /// The meeting id.
    pub meeting_id: String,
    /// Where it stands.
    pub state: State,
    /// When it started.
    pub started: Instant,
    /// Milliseconds captured so far.
    pub duration_ms: u64,
    /// Microphone takes written so far.
    pub microphone_takes: u32,
    /// True while the system track records.
    pub system_audio: bool,
    /// Final live segments so far.
    pub live_segment_count: u64,
    /// The end of the last final live segment.
    pub live_last_end_ms: u64,
    /// The failure reason when `Failed`.
    pub error: Option<String>,
}

impl Snapshot {
    /// The change the publisher gets for entering `state` from `previous`.
    pub(crate) fn change(
        &self,
        state: State,
        previous: State,
        reason: Option<String>,
    ) -> StateChange {
        StateChange {
            meeting_id: self.meeting_id.clone(),
            job_id: self.job_id.clone(),
            state,
            previous,
            reason,
            duration_ms: self.duration_ms,
            microphone_takes: self.microphone_takes,
            system_audio: self.system_audio,
            live_segment_count: self.live_segment_count,
            live_last_end_ms: self.live_last_end_ms,
            is_finalizing: state == State::Transcribing,
            diarization_status: None,
            analysis_status: None,
        }
    }
}

pub(crate) struct Active {
    pub(crate) snapshot: Snapshot,
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) worker: Option<std::thread::JoinHandle<()>>,
}

/// The shared slot the worker updates.
pub(crate) type Shared = Arc<Mutex<Option<Active>>>;

/// A finalisation in flight.
#[derive(Debug, Clone)]
pub struct Finalizing {
    /// The job id (`job_meeting_<n>`, or `job_recover_<n>` after a recovery).
    pub job_id: String,
    /// Set to stop between chunks.
    pub cancel: Arc<AtomicBool>,
    /// Chunks finished over every take.
    pub chunks_completed: u32,
    /// Chunks in total.
    pub chunks_total: u32,
}

/// The finalisations in flight by meeting id.
pub(crate) type Finalizations = Arc<Mutex<HashMap<String, Finalizing>>>;

/// The meeting service.
pub struct Session {
    active: Shared,
    finalizing: Finalizations,
    counter: AtomicU64,
    recover_counter: AtomicU64,
    artifacts: MeetingArtifacts,
    publisher: Arc<dyn Publisher>,
    archive: Arc<dyn Archive>,
    pub(crate) tails: LiveTails,
}

impl Session {
    /// A service whose meetings live under `artifacts` (one directory per
    /// meeting id), reporting to `publisher` and keeping rows through
    /// `archive`.
    pub fn new(
        artifacts: MeetingArtifacts,
        publisher: Arc<dyn Publisher>,
        archive: Arc<dyn Archive>,
    ) -> Self {
        Self {
            active: Arc::new(Mutex::new(None)),
            finalizing: Arc::new(Mutex::new(HashMap::new())),
            counter: AtomicU64::new(0),
            recover_counter: AtomicU64::new(0),
            artifacts,
            publisher,
            archive,
            tails: LiveTails::default(),
        }
    }

    /// The meeting directories.
    pub fn artifacts(&self) -> &MeetingArtifacts {
        &self.artifacts
    }

    /// Starts a meeting on `row` (its id names the directory): the slot is
    /// taken under the lock, the microphone opens (required), the system
    /// track opens (a missing one is journaled and the meeting is room
    /// audio), the row is stored as `recording` and the worker runs with
    /// `engine` for the live path and the finalisation.
    pub fn start(
        &self,
        policy: Policy,
        sources: Arc<dyn Sources>,
        engine: Arc<dyn SttEngine>,
        mut row: MeetingRow,
    ) -> Result<Snapshot, MeetingError> {
        if policy.live {
            policy.tuning.validate().map_err(MeetingError::Settings)?;
        }
        let mut guard = self.active.lock().unwrap_or_else(|p| p.into_inner());
        if guard.is_some() {
            return Err(MeetingError::Active);
        }
        let n = self.counter.fetch_add(1, Ordering::Relaxed) + 1;
        let job_id = format!("job_meeting_{n}");
        let started = Instant::now();
        // Each source is stamped the moment it opened: that instant is the
        // origin of its first samples on the meeting clock, which the take
        // manifest and the live windower both count from.
        let mic = sources
            .open_microphone(policy.level_interval_ms, false)
            .map_err(MeetingError::Audio)?;
        let mic = (mic, Instant::now());
        let system = match sources.open_system(policy.level_interval_ms) {
            Ok(s) => Ok((s, Instant::now())),
            Err(e) => {
                tracing::warn!(error = %e, "meeting: system track unavailable; room audio only");
                Err(e)
            }
        };
        let dir = self.artifacts.dir(&row.id);
        row.status = MeetingStatus::Recording;
        row.audio_dir = Some(dir.to_string_lossy().into_owned());
        row.system_audio = system.is_ok();
        if let Err(e) = self.archive.started(&row) {
            mic.0.stop();
            if let Ok(s) = &system {
                s.0.stop();
            }
            return Err(MeetingError::Store(e));
        }
        let snapshot = Snapshot {
            job_id: job_id.clone(),
            meeting_id: row.id.clone(),
            state: State::Recording,
            started,
            duration_ms: 0,
            microphone_takes: 0,
            system_audio: system.is_ok(),
            live_segment_count: 0,
            live_last_end_ms: 0,
            error: None,
        };
        let stop = Arc::new(AtomicBool::new(false));
        let cancel = Arc::new(AtomicBool::new(false));
        *guard = Some(Active {
            snapshot: snapshot.clone(),
            stop: stop.clone(),
            cancel: cancel.clone(),
            worker: None,
        });
        // The tail is readable from the first instant the meeting is.
        if policy.live {
            self.tails.tail(&row.id);
        }
        drop(guard);
        self.publisher
            .state(&snapshot.change(State::Recording, State::Idle, None));
        tracing::info!(job = %job_id, system_audio = system.is_ok(), live = policy.live, "meeting started");
        let id = row.id.clone();
        let worker = Worker {
            job_id: job_id.clone(),
            row,
            policy,
            sources,
            engine,
            stop,
            cancel,
            dir,
            clock: started,
            publisher: self.publisher.clone(),
            archive: self.archive.clone(),
            finalizing: self.finalizing.clone(),
            live: None,
            tails: self.tails.clone(),
        };
        let shared = self.active.clone();
        let handle = std::thread::Builder::new()
            .name(format!("meeting-{n}"))
            .spawn(move || worker.run(mic, system.ok(), shared))
            .map_err(|e| {
                *self.active.lock().unwrap_or_else(|p| p.into_inner()) = None;
                self.tails.remove(&id);
                MeetingError::Audio(format!("meeting thread: {e}"))
            })?;
        if let Some(a) = self
            .active
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_mut()
        {
            a.worker = Some(handle);
        }
        Ok(snapshot)
    }

    /// Asks the meeting to stop and answers with the stopping state at
    /// once; the worker closes the takes, reports `stopped` and
    /// finalises. A second call while stopping answers the same snapshot.
    pub fn stop(&self) -> Result<Snapshot, MeetingError> {
        let mut g = self.active.lock().unwrap_or_else(|p| p.into_inner());
        let Some(a) = g.as_mut() else {
            return Err(MeetingError::NoSession);
        };
        if a.snapshot.state == State::Recording {
            a.snapshot.state = State::Stopping;
            a.snapshot.duration_ms = a.snapshot.started.elapsed().as_millis() as u64;
            self.publisher
                .state(&a.snapshot.change(State::Stopping, State::Recording, None));
            tracing::info!(job = %a.snapshot.job_id, "meeting stopping");
        }
        a.stop.store(true, Ordering::Relaxed);
        Ok(a.snapshot.clone())
    }

    /// Cancels the recording meeting: the takes are removed and the row is
    /// marked cancelled. Waits for the worker.
    pub fn cancel(&self) -> Result<Snapshot, MeetingError> {
        let (cancel, stop, worker, snapshot) = {
            let mut g = self.active.lock().unwrap_or_else(|p| p.into_inner());
            let Some(a) = g.as_mut() else {
                return Err(MeetingError::NoSession);
            };
            (
                a.cancel.clone(),
                a.stop.clone(),
                a.worker.take(),
                a.snapshot.clone(),
            )
        };
        cancel.store(true, Ordering::Relaxed);
        stop.store(true, Ordering::Relaxed);
        if let Some(w) = worker {
            let _ = w.join();
        }
        Ok(Snapshot {
            state: State::Cancelled,
            ..snapshot
        })
    }

    /// The active meeting, if any.
    pub fn snapshot(&self) -> Option<Snapshot> {
        self.active
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|a| a.snapshot.clone())
    }

    /// The finalisation of `meeting_id`, while one runs.
    pub fn finalizing(&self, meeting_id: &str) -> Option<Finalizing> {
        self.finalizing
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(meeting_id)
            .cloned()
    }

    /// Stops the finalisation of `meeting_id` between chunks; true when
    /// one was running. The row ends `stopped` with its audio.
    pub fn cancel_finalize(&self, meeting_id: &str) -> bool {
        match self.finalizing(meeting_id) {
            Some(f) => {
                f.cancel.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }

    /// Finalises a recovered meeting (`meetings.recover`): its takes go
    /// through the same finalisation on a thread of their own; the row is
    /// stored `transcribing` before this returns with the job id.
    pub fn finalize_recovered(
        &self,
        policy: Policy,
        engine: Arc<dyn SttEngine>,
        mut row: MeetingRow,
    ) -> Result<String, MeetingError> {
        let n = self.recover_counter.fetch_add(1, Ordering::Relaxed) + 1;
        let job_id = format!("job_recover_{n}");
        let dir = self.artifacts.dir(&row.id);
        row.status = MeetingStatus::Transcribing;
        row.is_partial = false;
        self.archive.updated(&row).map_err(MeetingError::Store)?;
        let ctx = FinalizeContext {
            job_id: job_id.clone(),
            row,
            dir,
            policy,
            engine,
            publisher: self.publisher.clone(),
            archive: self.archive.clone(),
            finalizing: self.finalizing.clone(),
            previous: State::Partial,
            capture_duration_ms: 0,
            tails: self.tails.clone(),
        };
        std::thread::Builder::new()
            .name(format!("meeting-recover-{n}"))
            .spawn(move || {
                finalize_meeting(ctx);
            })
            .map_err(|e| MeetingError::Audio(format!("finalise thread: {e}")))?;
        Ok(job_id)
    }

    /// Waits until the worker has finished (a stop that was asked for)
    /// and no finalisation runs, up to `timeout`; true when idle by then.
    pub fn wait_idle(&self, timeout: std::time::Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let idle = || {
            self.snapshot().is_none()
                && self
                    .finalizing
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .is_empty()
        };
        while Instant::now() < deadline {
            if idle() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        idle()
    }
}
