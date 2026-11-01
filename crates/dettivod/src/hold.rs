//! The daemon's hold key (ADR 0083): the state machine from
//! `dettivo_hotkeys::hold`, fed by `dictation.press` and
//! `dictation.release` (the compositor bindings) and by the portal and
//! evdev backends, acted on against the session. It owns the take it
//! started; a take that ended any other way (cancel, toggle, the maximum
//! duration) returns the key to idle before the next event is read, so
//! the machine never acts on a take it no longer has.

use std::sync::Mutex;
use std::time::Instant;

use dettivo_hotkeys::hold::{HoldAction, HoldState, on_press, on_release};
use dettivo_proto::error::JsonRpcError;
use dettivo_proto::events::{DictationStatePayload, Topic};
use dettivo_proto::methods::dictation::HoldResult;
use dettivo_session::State;
use serde_json::json;

use crate::actions;
use crate::daemon::Daemon;

/// The reason a `dictation.state` carries when the key locks the take.
pub const LOCKED: &str = "locked";

/// A press or a release of the hold chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    /// The chord went down.
    Press,
    /// The chord came up.
    Release,
}

#[derive(Default)]
struct Inner {
    state: HoldState,
    /// The take this key started and still owns.
    job: Option<String>,
}

/// What the hold key does to the session: the daemon, or a fake in the
/// tests.
pub trait Takes {
    /// The take recording now.
    fn recording(&self) -> Option<String>;
    /// Starts a take; its job id.
    fn start(&self) -> Result<String, JsonRpcError>;
    /// Drops the take a tap started.
    fn discard(&self, job: &str);
    /// Tells the pill `job` is locked, only while it still records; false
    /// when it ended meanwhile.
    fn lock(&self, job: &str) -> bool;
}

impl Takes for Daemon {
    fn recording(&self) -> Option<String> {
        self.dictation()
            .snapshot()
            .filter(|s| s.state == State::Recording)
            .map(|s| s.job_id)
    }
    fn start(&self) -> Result<String, JsonRpcError> {
        actions::start(self, None, None, None).map(|s| s.job_id)
    }
    fn discard(&self, job: &str) {
        if let Err(e) = self.dictation().cancel_expected(Some(job)) {
            tracing::debug!(error = %e, "hold key: the tapped take was already gone");
        }
    }
    fn lock(&self, job: &str) -> bool {
        self.dictation()
            .while_recording(job, || publish_locked(self, job))
            .is_some()
    }
}

/// What an edge did, and the take the caller now settles when it
/// finished one: the settlement runs outside the key's lock, so the next
/// edge is not held behind the transcription.
pub struct Outcome {
    /// The wire answer.
    pub result: HoldResult,
    /// The take to stop.
    pub stop: Option<String>,
}

/// The hold key.
#[derive(Default)]
pub struct HoldKey {
    inner: Mutex<Inner>,
    /// The take a double tap locked, for `dictation.status`; apart from
    /// `inner` so a status never waits behind a start.
    locked: Mutex<Option<String>>,
}

impl HoldKey {
    /// Feeds one edge that arrived `at` and acts on it. Edges are
    /// serialized: the lock is held through a start and a discard, so a
    /// release that arrives meanwhile sees the take. Every edge counts
    /// from its arrival, so one that waited behind a slow start or
    /// discard (a cold engine, `pause_media`) is timed as the person
    /// pressed it.
    pub fn on(&self, takes: &impl Takes, edge: Edge, at: Instant) -> Result<Outcome, JsonRpcError> {
        let mut g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        // The take is still the key's only while it records: a take that
        // was stopped, cancelled or replaced elsewhere frees the key.
        if g.state.records() && (g.job.is_none() || g.job != takes.recording()) {
            tracing::debug!(
                hold = g.state.as_str(),
                "hold key: its take ended elsewhere"
            );
            *g = Inner::default();
        }
        let (mut next, mut action) = match edge {
            Edge::Press => on_press(g.state, at),
            Edge::Release => on_release(g.state, at),
        };
        let mut job_id = g.job.clone();
        let mut stop = None;
        match action {
            HoldAction::Start => match takes.start() {
                Ok(job) => {
                    job_id = Some(job);
                    g.job = job_id.clone();
                }
                Err(e) => {
                    *g = Inner::default();
                    return Err(e);
                }
            },
            HoldAction::Stop => stop = g.job.take(),
            HoldAction::DiscardTap => {
                if let Some(job) = g.job.take() {
                    takes.discard(&job);
                }
            }
            HoldAction::Lock => {
                // Noted first, so a status read beside the notice agrees.
                let job = g.job.clone().unwrap_or_default();
                *self.locked.lock().unwrap_or_else(|p| p.into_inner()) = Some(job.clone());
                if !takes.lock(&job) {
                    // The take ended between the release and the notice.
                    (next, action, job_id) = (HoldState::Idle, HoldAction::None, None);
                    g.job = None;
                }
            }
            HoldAction::None => {
                // At debug, so a live run can tell a release the
                // compositor never delivered from one the key ignored.
                tracing::debug!(edge = ?edge, hold = g.state.as_str(), "hold key: edge did nothing");
                if edge == Edge::Press {
                    tracing::info!(action = "push_to_talk", "hotkey press ignored: key repeat");
                }
                job_id = None;
            }
        }
        g.state = next;
        if next != HoldState::Locked {
            self.locked.lock().unwrap_or_else(|p| p.into_inner()).take();
        }
        Ok(Outcome {
            result: HoldResult {
                action: action.as_str().to_string(),
                hold: next.as_str().to_string(),
                job_id,
            },
            stop,
        })
    }

    /// Whether a double tap locked the take `job`.
    pub fn locked(&self, job: &str) -> bool {
        self.locked
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_deref()
            == Some(job)
    }
}

/// Tells the pill the take stays recording after the release: a
/// `dictation.state` from `recording` to `recording`, reason `locked`.
fn publish_locked(daemon: &Daemon, job: &str) {
    tracing::info!(job, "dictation locked: the next press finishes the take");
    daemon.bus().publish(
        Topic::DictationState,
        json!(DictationStatePayload {
            job_id: job.to_string(),
            state: "recording".into(),
            previous_state: "recording".into(),
            reason: Some(LOCKED.into()),
            insertion: None,
            first_words: None,
            timings: None,
            mode: None,
            notice: None,
            policy_hash: None,
        }),
    );
}

#[cfg(test)]
#[path = "hold_tests.rs"]
mod tests;
