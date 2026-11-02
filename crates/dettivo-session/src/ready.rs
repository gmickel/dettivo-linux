//! The always-ready microphone (ADR 0092): one open source drained into
//! an [`AudioRing`] while dictation is idle, so a take can start
//! [`PRE_ROLL`] before its press. A take taps the feed rather than open a
//! second capture of the same source: it receives the ring's audio from
//! the pre-roll onward, then every live chunk until its stop, which ends
//! the tap after the last chunk it forwarded. Ring audio never leaves this
//! process and is never written; only a take's window reaches its take
//! file.

use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dettivo_audio::ring::{AudioRing, RING_CAPACITY};
use dettivo_audio::{EndReason, Event};

use crate::source::Source;

/// How far before the press a take starts when the ring is open.
pub const PRE_ROLL: Duration = Duration::from_millis(450);

#[derive(Default)]
struct Tap {
    /// Counts taps, so a stale take's stop never ends a later one.
    generation: u64,
    sender: Option<Sender<Event>>,
}

struct FeedState {
    ring: AudioRing,
    tap: Tap,
    /// Why the capture ended, once it has.
    ended: Option<EndReason>,
    /// The default source moved since the feed opened.
    moved: bool,
    /// When the newest chunk was captured.
    last_at: Option<Instant>,
}

/// A running always-ready feed.
pub struct ReadyFeed {
    state: Arc<Mutex<FeedState>>,
    stop: Arc<dyn Fn() + Send + Sync>,
    drain: Option<std::thread::JoinHandle<()>>,
}

fn lock(state: &Mutex<FeedState>) -> std::sync::MutexGuard<'_, FeedState> {
    state.lock().unwrap_or_else(|p| p.into_inner())
}

impl ReadyFeed {
    /// Drains `source` into the ring on a thread of its own.
    pub fn open(source: Source) -> std::io::Result<Self> {
        let state = Arc::new(Mutex::new(FeedState {
            ring: AudioRing::new(RING_CAPACITY),
            tap: Tap::default(),
            ended: None,
            moved: false,
            last_at: None,
        }));
        let stop = source.stopper();
        let shared = state.clone();
        let drain = std::thread::Builder::new()
            .name("dettivo-ready".into())
            .spawn(move || drain(source, &shared))?;
        Ok(Self {
            state,
            stop,
            drain: Some(drain),
        })
    }

    /// Starts a take at `since`: the ring's audio from then on, then the
    /// live chunks. `None` while another take taps the feed or once the
    /// capture ended.
    pub fn take(&self, since: Instant) -> Option<Source> {
        let mut s = lock(&self.state);
        if s.tap.sender.is_some() || s.ended.is_some() {
            return None;
        }
        let (tx, events) = mpsc::channel();
        let backlog = s.ring.since(since);
        if let (false, Some(at)) = (backlog.is_empty(), s.last_at) {
            let _ = tx.send(Event::Pcm { pcm: backlog, at });
        }
        s.tap.generation += 1;
        s.tap.sender = Some(tx);
        let generation = s.tap.generation;
        let state = Arc::downgrade(&self.state);
        Some(Source::Channel {
            events,
            on_stop: Arc::new(move || {
                let Some(state) = state.upgrade() else {
                    return;
                };
                let mut s = lock(&state);
                if s.tap.generation == generation
                    && let Some(tx) = s.tap.sender.take()
                {
                    let _ = tx.send(Event::Ended {
                        reason: EndReason::Stopped,
                    });
                }
            }),
        })
    }

    /// True while a take taps the feed.
    pub fn tapped(&self) -> bool {
        lock(&self.state).tap.sender.is_some()
    }

    /// Why the feed should be reopened: its capture ended, or the default
    /// source moved.
    pub fn stale(&self) -> Option<String> {
        let s = lock(&self.state);
        match (&s.ended, s.moved) {
            (Some(reason), _) => Some(format!("capture ended: {reason:?}")),
            (None, true) => Some("the default source moved".into()),
            (None, false) => None,
        }
    }

    /// Stops the capture and waits until it acknowledged the stop, so a
    /// capture opened after this returns never runs beside it.
    pub fn close(mut self) {
        (self.stop)();
        if let Some(drain) = self.drain.take() {
            let _ = drain.join();
        }
    }
}

impl Drop for ReadyFeed {
    fn drop(&mut self) {
        if self.drain.is_some() {
            (self.stop)();
        }
    }
}

/// Moves every event into the ring and to the tap, until the capture ends.
fn drain(source: Source, state: &Mutex<FeedState>) {
    loop {
        let event = source.events().recv();
        let mut s = lock(state);
        match event {
            Ok(Event::Pcm { pcm, at }) => {
                s.ring.push(at, &pcm);
                s.last_at = Some(at);
                if let Some(tx) = &s.tap.sender
                    && tx.send(Event::Pcm { pcm, at }).is_err()
                {
                    s.tap.sender = None;
                }
            }
            Ok(Event::DeviceChanged { from, to }) => {
                // Audio from the old source never prefixes a later take.
                s.moved = true;
                s.ring.clear();
                s.last_at = None;
                if let Some(tx) = &s.tap.sender {
                    let _ = tx.send(Event::DeviceChanged { from, to });
                }
            }
            Ok(level @ Event::Level { .. }) => {
                if let Some(tx) = &s.tap.sender {
                    let _ = tx.send(level);
                }
            }
            Ok(Event::Ended { reason }) => {
                if let Some(tx) = s.tap.sender.take() {
                    let _ = tx.send(Event::Ended {
                        reason: reason.clone(),
                    });
                }
                s.ended = Some(reason);
                return;
            }
            Err(_) => {
                let reason = EndReason::Error("the capture closed without an end".into());
                if let Some(tx) = s.tap.sender.take() {
                    let _ = tx.send(Event::Ended {
                        reason: reason.clone(),
                    });
                }
                s.ended = Some(reason);
                return;
            }
        }
    }
}

#[cfg(test)]
#[path = "ready_tests.rs"]
mod tests;
