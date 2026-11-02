//! How a take ends: its completion, set once and waited on by every
//! stopper, and the terminal state the worker publishes.

use std::sync::{Arc, Condvar, Mutex};

use dettivo_speech::gate::SilenceReason;

use super::{Active, Session, while_recording};
use crate::{Publisher, State, StateChange, Transcript};

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
    pub(super) fn wait(&self) -> Outcome {
        let mut g = self.outcome.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if let Some(o) = g.as_ref() {
                return o.clone();
            }
            g = self.done.wait(g).unwrap_or_else(|p| p.into_inner());
        }
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
    pub(super) fn shared(&self, completion: Arc<Completion>) -> SharedState {
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
    /// The press was shorter than the minimum hold: a tap, not a take.
    Tap,
    /// The speech gate found the take silent; it never reached the engine.
    Silence(SilenceReason),
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
        Exit::Tap => (State::Cancelled, Some(crate::TAP.to_string())),
        Exit::Silence(stage) => (
            State::Cancelled,
            Some(format!("silence: {}", stage.as_str())),
        ),
        Exit::Failed(r) => (State::Failed, Some(r.clone())),
    };
    let outcome = match (&exit, transcript) {
        (Exit::Completed(_), Some(t)) => {
            *shared.last.lock().unwrap_or_else(|p| p.into_inner()) = Some(t.clone());
            Outcome::Completed(Box::new(t))
        }
        (Exit::Completed(_), None) => Outcome::Failed("no transcript".into()),
        (Exit::Cancelled | Exit::Tap | Exit::Silence(_), _) => Outcome::Cancelled,
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
    while_recording::mark_ending(&shared.active);
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
