//! The daemon's hold key (ADR 0081): the state machine from
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
}

impl HoldKey {
    /// Feeds one edge and acts on it. The lock is held through a start
    /// and a discard, so a release that arrives meanwhile sees the take.
    pub fn on(&self, daemon: &Daemon, edge: Edge) -> Result<Outcome, JsonRpcError> {
        let mut g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        // The take is still the key's only while it records: a take that
        // was stopped, cancelled or replaced elsewhere frees the key.
        let recording = daemon
            .dictation()
            .snapshot()
            .filter(|s| s.state == State::Recording)
            .map(|s| s.job_id);
        if g.state.records() && (g.job.is_none() || g.job != recording) {
            tracing::debug!(
                hold = g.state.as_str(),
                "hold key: its take ended elsewhere"
            );
            *g = Inner::default();
        }
        let now = Instant::now();
        let (mut next, action) = match edge {
            Edge::Press => on_press(g.state, now),
            Edge::Release => on_release(g.state, now),
        };
        let mut job_id = g.job.clone();
        let mut stop = None;
        match action {
            HoldAction::Start => match actions::start(daemon, None, None, None) {
                Ok(snapshot) => {
                    job_id = Some(snapshot.job_id.clone());
                    g.job = job_id.clone();
                    // The hold counts from when recording began: a start
                    // that loaded the engine must not turn the key's
                    // repeat, queued behind it, into a lost release.
                    if let HoldState::Held { second_tap, .. } = next {
                        next = HoldState::Held {
                            since: Instant::now(),
                            second_tap,
                        };
                    }
                }
                Err(e) => {
                    *g = Inner::default();
                    return Err(e);
                }
            },
            HoldAction::Stop => stop = g.job.take(),
            HoldAction::DiscardTap => {
                let gone = g
                    .job
                    .take()
                    .map(|job| daemon.dictation().cancel_expected(Some(&job)));
                if let Some(Err(e)) = gone {
                    tracing::debug!(error = %e, "hold key: the tapped take was already gone");
                }
            }
            HoldAction::Lock => {
                if let Some(job) = &g.job {
                    publish_locked(daemon, job);
                }
            }
            HoldAction::None => {
                if edge == Edge::Press {
                    tracing::info!(action = "push_to_talk", "hotkey press ignored: key repeat");
                }
                job_id = None;
            }
        }
        g.state = next;
        Ok(Outcome {
            result: HoldResult {
                action: action.as_str().to_string(),
                hold: next.as_str().to_string(),
                job_id,
            },
            stop,
        })
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
