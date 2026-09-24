//! The state machine: `idle → recording → transcribing → inserting → idle`
//! with `cancelled` and `failed` exits. One session per daemon; the active
//! flag is reserved synchronously in `start` before anything else runs
//! (Appendix D), so a second `start` is `CONFLICT` even when it races the
//! first. A worker thread drains the audio source, writes the take,
//! transcribes through the frozen engine and hands the text over. Every
//! take has a completion of its own, created before its source opens:
//! a stop or cancel waits on that take's completion, so a stop during
//! the open, two stoppers of one take and a start right after the take
//! all see the result that belongs to them.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

use dettivo_language::provider::LocalEngine;
use dettivo_speech::SttEngine;

use crate::source::SourceFactory;
use crate::worker::Worker;
use crate::{
    Archive, Inserter, Policy, Publisher, SessionError, SessionTarget, State, StateChange,
    Transcript,
};

/// The public view of the active session.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// The job id.
    pub job_id: String,
    /// Where it stands.
    pub state: State,
    /// Progress in `[0, 1]`: recording seconds over the maximum, then the
    /// transcription steps.
    pub progress: f64,
    /// The frozen policy.
    pub policy: Policy,
    /// The failure reason when `Failed`.
    pub error: Option<String>,
    /// When it started.
    pub started: Instant,
    /// The window captured at start, handed to the inserter as the guard.
    pub target: Option<SessionTarget>,
}

pub(crate) struct Active {
    pub(crate) snapshot: Snapshot,
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) engine: Arc<dyn SttEngine>,
    pub(crate) worker: Option<std::thread::JoinHandle<()>>,
    pub(crate) completion: Arc<Completion>,
}

/// How a take ended.
#[derive(Debug, Clone)]
pub(crate) enum Outcome {
    Completed(Box<Transcript>),
    Cancelled,
    Failed(String),
}

/// One take's end: set once, by the worker or by the start that could
/// not run it, and waited on by every stopper of that take.
#[derive(Default)]
pub(crate) struct Completion {
    outcome: Mutex<Option<Outcome>>,
    done: Condvar,
}

impl Completion {
    /// Records the outcome, once; a second call keeps the first.
    pub(crate) fn set(&self, outcome: Outcome) {
        let mut g = self.outcome.lock().unwrap_or_else(|p| p.into_inner());
        if g.is_none() {
            *g = Some(outcome);
        }
        self.done.notify_all();
    }

    /// Waits until the take ended.
    fn wait(&self) -> Outcome {
        let mut g = self.outcome.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if let Some(o) = g.as_ref() {
                return o.clone();
            }
            g = self.done.wait(g).unwrap_or_else(|p| p.into_inner());
        }
    }
}

/// The session service.
pub struct Session {
    active: Arc<Mutex<Option<Active>>>,
    last: Arc<Mutex<Option<Transcript>>>,
    counter: AtomicU64,
    sessions_dir: PathBuf,
    publisher: Arc<dyn Publisher>,
    inserter: Arc<dyn Inserter>,
    archive: Option<Arc<dyn Archive>>,
    /// The daemon's local language model engine, frozen per session at
    /// its start (ADR 0026).
    local_llm: Mutex<Option<Arc<dyn LocalEngine>>>,
}

impl Session {
    /// A service that keeps takes under `sessions_dir/<job_id>/`.
    pub fn new(
        sessions_dir: PathBuf,
        publisher: Arc<dyn Publisher>,
        inserter: Arc<dyn Inserter>,
    ) -> Self {
        Self {
            active: Arc::new(Mutex::new(None)),
            last: Arc::new(Mutex::new(None)),
            counter: AtomicU64::new(0),
            sessions_dir,
            publisher,
            inserter,
            archive: None,
            local_llm: Mutex::new(None),
        }
    }

    /// Installs (or clears) the local language model engine the Enhanced
    /// pass reaches; a session starting afterwards freezes it.
    pub fn set_local_llm(&self, engine: Option<Arc<dyn LocalEngine>>) {
        *self.local_llm.lock().unwrap_or_else(|p| p.into_inner()) = engine;
    }

    /// Replaces the inserter (S-08 swaps in the backend chain).
    pub fn set_inserter(&mut self, inserter: Arc<dyn Inserter>) {
        self.inserter = inserter;
    }

    /// Installs the archive every finished transcript is handed to.
    pub fn set_archive(&mut self, archive: Arc<dyn Archive>) {
        self.archive = Some(archive);
    }

    /// The inserter in force.
    pub fn inserter(&self) -> Arc<dyn Inserter> {
        self.inserter.clone()
    }

    /// Starts a session: the active flag is taken under the lock before the
    /// source opens, so a concurrent start sees `Active`. The engine and
    /// the source are the caller's (frozen for this session).
    pub fn start(
        &self,
        policy: Policy,
        engine: Arc<dyn SttEngine>,
        source: Arc<dyn SourceFactory>,
        target: Option<SessionTarget>,
    ) -> Result<Snapshot, SessionError> {
        if dettivo_language::pipeline::Mode::parse(&policy.mode).is_none() {
            return Err(SessionError::ModeUnknown(policy.mode));
        }
        if !policy.model_path.is_file() {
            return Err(SessionError::ModelMissing {
                model: format!("{}/{}", policy.provider, policy.model),
                recommended_action: format!(
                    "run `dettivo speech download --model {}`",
                    policy.model
                ),
            });
        }
        let mut guard = self.active.lock().unwrap_or_else(|p| p.into_inner());
        if guard.is_some() {
            return Err(SessionError::Active);
        }
        let n = self.counter.fetch_add(1, Ordering::Relaxed) + 1;
        let job_id = format!("job_dict_{n}");
        let snapshot = Snapshot {
            job_id: job_id.clone(),
            state: State::Recording,
            progress: 0.0,
            policy: policy.clone(),
            error: None,
            started: Instant::now(),
            target: target.clone(),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let cancel = Arc::new(AtomicBool::new(false));
        let completion = Arc::new(Completion::default());
        // The slot is taken now, with this take's completion; the worker
        // fills in the rest.
        *guard = Some(Active {
            snapshot: snapshot.clone(),
            stop: stop.clone(),
            cancel: cancel.clone(),
            engine: engine.clone(),
            worker: None,
            completion: completion.clone(),
        });
        drop(guard);
        let shared = self.shared(completion);

        let opened = source.open(policy.level_interval_ms);
        let src = match opened {
            Ok(s) => s,
            Err(e) => {
                finish_shared(
                    &shared,
                    &*self.publisher,
                    &job_id,
                    State::Recording,
                    Exit::Failed(format!("capture: {e}")),
                    None,
                );
                return Err(SessionError::Audio(e));
            }
        };
        self.publisher.state(&StateChange::plain(
            &job_id,
            State::Recording,
            State::Idle,
            None,
        ));
        tracing::info!(job = %job_id, provider = %policy.provider, model = %policy.model, "dictation started");
        let ctx = Worker {
            job_id: job_id.clone(),
            policy,
            engine,
            stop,
            cancel,
            dir: self.sessions_dir.join(&job_id),
            publisher: self.publisher.clone(),
            inserter: self.inserter.clone(),
            archive: self.archive.clone(),
            target,
            local_llm: self
                .local_llm
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone(),
        };
        let worker_shared = shared.clone();
        let handle = match std::thread::Builder::new()
            .name(format!("dictation-{n}"))
            .spawn(move || ctx.run(src, worker_shared))
        {
            Ok(h) => h,
            Err(e) => {
                // The slot is freed and the exit reported, like a capture
                // that could not open; otherwise every later start would
                // see an active session that never ran.
                finish_shared(
                    &shared,
                    &*self.publisher,
                    &job_id,
                    State::Recording,
                    Exit::Failed(format!("session thread: {e}")),
                    None,
                );
                return Err(SessionError::Audio(format!("session thread: {e}")));
            }
        };
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

    /// Asks the session to stop recording and finish; waits for this
    /// take's result, whether the stop lands while the source is still
    /// opening or beside another stopper.
    pub fn stop(&self) -> Result<Transcript, SessionError> {
        self.stop_expected(None)
    }

    /// Stops only the named active take when supplied, never a replacement.
    pub fn stop_expected(&self, expected_job_id: Option<&str>) -> Result<Transcript, SessionError> {
        let (stop, worker, completion) = {
            let mut g = self.active.lock().unwrap_or_else(|p| p.into_inner());
            let Some(a) = g.as_mut() else {
                return Err(SessionError::NoSession);
            };
            if expected_job_id.is_some_and(|id| id != a.snapshot.job_id) {
                return Err(SessionError::NoSession);
            }
            (a.stop.clone(), a.worker.take(), a.completion.clone())
        };
        stop.store(true, Ordering::Relaxed);
        Self::await_take(worker, &completion);
        match completion.wait() {
            Outcome::Completed(t) => Ok(*t),
            Outcome::Cancelled => Err(SessionError::Cancelled),
            Outcome::Failed(r) => Err(SessionError::Failed(r)),
        }
    }

    /// Cancels the session in any state; nothing is kept. Waits for the
    /// take to end.
    pub fn cancel(&self) -> Result<Snapshot, SessionError> {
        self.cancel_expected(None)
    }

    /// Cancels only the named job when supplied. A replaced job is absent.
    pub fn cancel_expected(&self, expected_job_id: Option<&str>) -> Result<Snapshot, SessionError> {
        let (worker, snapshot, completion) = {
            let mut g = self.active.lock().unwrap_or_else(|p| p.into_inner());
            let Some(a) = g.as_mut() else {
                return Err(SessionError::NoSession);
            };
            if expected_job_id.is_some_and(|id| id != a.snapshot.job_id) {
                return Err(SessionError::NoSession);
            }
            a.cancel.store(true, Ordering::Relaxed);
            a.stop.store(true, Ordering::Relaxed);
            a.engine.cancel();
            (a.worker.take(), a.snapshot.clone(), a.completion.clone())
        };
        Self::await_take(worker, &completion);
        let outcome = completion.wait();
        // The worker had already handed the text over when the cancel
        // arrived: the session completed, so there was none to cancel.
        if matches!(outcome, Outcome::Completed(_)) {
            return Err(SessionError::NoSession);
        }
        Ok(Snapshot {
            state: State::Cancelled,
            ..snapshot
        })
    }

    /// Joins the worker when this caller holds its handle; a worker that
    /// ended without completing its take (a panic) completes it failed,
    /// so no stopper waits forever.
    fn await_take(worker: Option<std::thread::JoinHandle<()>>, completion: &Completion) {
        if let Some(w) = worker {
            let _ = w.join();
            completion.set(Outcome::Failed(
                "the session worker ended without a result".into(),
            ));
        }
    }

    /// The active session, if any.
    pub fn snapshot(&self) -> Option<Snapshot> {
        self.active
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|a| a.snapshot.clone())
    }

    /// The last finished transcript.
    pub fn last(&self) -> Option<Transcript> {
        self.last.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Hands the last transcript to the inserter again.
    pub fn reinsert_last(&self) -> Result<Transcript, SessionError> {
        let mut t = self.last().ok_or(SessionError::NoTranscript)?;
        let result = self.inserter.insert(&t.text, None);
        t.insertion = Some(result);
        *self.last.lock().unwrap_or_else(|p| p.into_inner()) = Some(t.clone());
        Ok(t)
    }
}

/// The two slots the worker updates (both live in the `Session`) and the
/// completion of the take it runs.
#[derive(Clone)]
pub(crate) struct SharedState {
    pub(crate) active: Arc<Mutex<Option<Active>>>,
    pub(crate) last: Arc<Mutex<Option<Transcript>>>,
    pub(crate) completion: Arc<Completion>,
}

impl Session {
    fn shared(&self, completion: Arc<Completion>) -> SharedState {
        SharedState {
            active: self.active.clone(),
            last: self.last.clone(),
            completion,
        }
    }
}

pub(crate) enum Exit {
    /// Finished; a warning (the archive failed) rides on the final state.
    Completed(Option<String>),
    Cancelled,
    Failed(String),
}

pub(crate) fn finish_shared(
    shared: &SharedState,
    publisher: &dyn Publisher,
    job_id: &str,
    previous: State,
    exit: Exit,
    transcript: Option<Transcript>,
) {
    let (state, reason) = match &exit {
        Exit::Completed(warning) => (State::Idle, warning.clone()),
        Exit::Cancelled => (State::Cancelled, Some("cancelled".to_string())),
        Exit::Failed(r) => (State::Failed, Some(r.clone())),
    };
    let outcome = match (&exit, transcript) {
        (Exit::Completed(_), Some(t)) => {
            *shared.last.lock().unwrap_or_else(|p| p.into_inner()) = Some(t.clone());
            Outcome::Completed(Box::new(t))
        }
        (Exit::Completed(_), None) => Outcome::Failed("no transcript".into()),
        (Exit::Cancelled, _) => Outcome::Cancelled,
        (Exit::Failed(r), _) => Outcome::Failed(r.clone()),
    };
    let completion = match &outcome {
        Outcome::Completed(t) => (
            t.insertion.clone(),
            Some(crate::first_words(&t.text)),
            t.timings,
        ),
        _ => (None, None, None),
    };
    // The Polish fields ride on the same completion transition, so a
    // client that sees `idle` knows which mode ran and why it fell back.
    let polish = match &outcome {
        Outcome::Completed(t) => (
            Some(t.policy.mode.clone()),
            t.notice.clone(),
            t.policy_hash.clone(),
        ),
        _ => (None, None, None),
    };
    let mut change = StateChange::plain(job_id, state, previous, reason.clone());
    (change.insertion, change.first_words, change.timings) = completion;
    (change.mode, change.notice, change.policy_hash) = polish;
    publisher.state(&change);
    if state != State::Idle {
        publisher.state(&StateChange::plain(job_id, State::Idle, state, reason));
    }
    // The slot is retired first, then the take completes: a stopper that
    // wakes finds the slot free for the next start.
    *shared.active.lock().unwrap_or_else(|p| p.into_inner()) = None;
    shared.completion.set(outcome);
}
