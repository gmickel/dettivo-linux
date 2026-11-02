//! The always-ready microphone (ADR 0092): with `[audio]
//! keep_microphone_ready` on, dictation keeps one capture open into the
//! session's ring while it is idle, the graphical session is active and
//! unlocked, and no meeting captures. Every change happens under the
//! capture reservation, so the ring never runs beside another capture of
//! the microphone: a meeting start closes it before the meeting opens its
//! streams, and it reopens once the meeting's capture ended. A ring that
//! cannot open is reported by `audio.devices` and `dettivo doctor`; a
//! dictation then opens the microphone on press, as with the setting off.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use dettivo_proto::methods::audio::MicrophoneReady;
use dettivo_session::ready::ReadyFeed;
use dettivo_session::source::Source;

use crate::daemon::Daemon;

/// How often the daemon reconciles the ring with what it wants.
pub const TICK: Duration = Duration::from_millis(250);

/// How long a ring that failed to open waits before the next try.
const RETRY: Duration = Duration::from_secs(2);

/// Where the ring stands, for `audio.devices`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Status {
    /// `[audio] keep_microphone_ready` is off.
    #[default]
    Off,
    /// The capture is open.
    Open,
    /// The ring waits: a meeting, a lock, a dictation on its own capture.
    Closed(String),
    /// The capture could not open; the next try follows after [`RETRY`].
    Failed(String),
}

impl Status {
    /// The wire form.
    pub fn wire(&self) -> MicrophoneReady {
        let (state, reason) = match self {
            Self::Off => ("off", None),
            Self::Open => ("open", None),
            Self::Closed(why) => ("closed", Some(why.clone())),
            Self::Failed(why) => ("failed", Some(why.clone())),
        };
        MicrophoneReady {
            state: state.into(),
            reason,
        }
    }
}

#[derive(Default)]
struct Inner {
    feed: Option<ReadyFeed>,
    /// The device the feed captures; empty for the default source.
    device: String,
    status: Status,
    retry_at: Option<Instant>,
}

/// The ring's owner.
#[derive(Default)]
pub struct MicReady {
    inner: Mutex<Inner>,
    /// logind reports the graphical session locked or inactive.
    blocked: AtomicBool,
}

impl MicReady {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// A take from the ring from `since` onward, when it is open and idle.
    /// A ring whose capture ended is closed here, so the caller's own
    /// capture never runs beside it.
    pub fn take(&self, since: Instant) -> Option<Source> {
        let mut g = self.lock();
        let take = g.feed.as_ref()?.take(since);
        if take.is_none()
            && let Some(why) = g.feed.as_ref().and_then(ReadyFeed::stale)
        {
            tracing::info!(why, "microphone ready: closed for a reopen");
            if let Some(feed) = g.feed.take() {
                feed.close();
            }
            g.status = Status::Closed(why);
        }
        take
    }

    /// Closes the ring and waits until its capture acknowledged the stop.
    pub fn close(&self, why: &str) {
        let mut g = self.lock();
        if let Some(feed) = g.feed.take() {
            feed.close();
            tracing::info!(why, "microphone ready: closed");
        }
        if g.status != Status::Off {
            g.status = Status::Closed(why.into());
        }
    }

    /// Records whether logind reports the session locked or inactive.
    pub fn set_blocked(&self, blocked: bool) {
        if self.blocked.swap(blocked, Ordering::Relaxed) != blocked {
            tracing::info!(blocked, "microphone ready: session lock changed");
        }
    }

    /// Where the ring stands.
    pub fn status(&self) -> Status {
        self.lock().status.clone()
    }

    /// Brings the ring to what the daemon wants. A ring a take still uses
    /// stays as it is. One whose capture ended, whose default source moved
    /// or that runs on another device than `want.device` reopens; one the
    /// daemon no longer wants closes.
    pub fn reconcile(&self, want: Want, open: impl FnOnce(&str) -> Result<Source, String>) {
        let mut g = self.lock();
        if let Some(feed) = &g.feed {
            if feed.tapped() {
                return;
            }
            let stale = feed
                .stale()
                .or_else(|| (g.device != want.device).then(|| "the device changed".to_string()));
            if stale.is_none() && want.open.is_ok() {
                return;
            }
            if let Some(feed) = g.feed.take() {
                feed.close();
            }
            tracing::info!(
                why = stale.as_deref().unwrap_or("no longer wanted"),
                "microphone ready: closed"
            );
            g.retry_at = None;
        }
        match want.open {
            Err(None) => {
                g.status = Status::Off;
                g.retry_at = None;
            }
            Err(Some(why)) => g.status = Status::Closed(why),
            Ok(()) if !want.may_open => {
                g.status = Status::Closed("a dictation records on its own capture".into());
            }
            Ok(()) => {
                if g.retry_at.is_some_and(|at| Instant::now() < at) {
                    return;
                }
                let opened = open(&want.device)
                    .and_then(|src| ReadyFeed::open(src).map_err(|e| e.to_string()));
                match opened {
                    Ok(feed) => {
                        tracing::info!(device = %want.device, "microphone ready: open");
                        g.feed = Some(feed);
                        g.device = want.device;
                        g.status = Status::Open;
                        g.retry_at = None;
                    }
                    Err(e) => {
                        if g.status != Status::Failed(e.clone()) {
                            tracing::warn!(error = %e, "microphone ready: the capture did not open");
                        }
                        g.status = Status::Failed(e);
                        g.retry_at = Some(Instant::now() + RETRY);
                    }
                }
            }
        }
    }
}

/// What the daemon wants of the ring at one tick.
pub struct Want {
    /// `Ok` when it should be open, else why it waits (`None` when the
    /// setting is off).
    pub open: Result<(), Option<String>>,
    /// False while a dictation records on a capture of its own: the ring
    /// may not open beside it, and an open ring is left as it is.
    pub may_open: bool,
    /// The device to capture: the pinned `[audio] input_device` while it
    /// is present, else empty for the default source.
    pub device: String,
}

/// Why the ring should be closed, if it should: `None` inside the error
/// means the setting is off.
fn closed_because(setting: bool, blocked: bool, meeting: bool) -> Result<(), Option<String>> {
    let why = match () {
        _ if !setting => return Err(None),
        _ if blocked => "the session is locked",
        _ if meeting => "a meeting is capturing",
        _ => return Ok(()),
    };
    Err(Some(why.into()))
}

/// One reconciliation under the capture reservation. A pinned device that
/// is gone leaves the ring on the default source until it returns (R6).
pub fn tick(daemon: &Daemon) {
    let _reservation = daemon.capture_start_lock();
    let ready = daemon.dictation().ready();
    let audio = daemon.config().config.audio;
    let pinned = !audio.input_device.is_empty()
        && daemon
            .audio()
            .is_some_and(|a| a.node_named(&audio.input_device).is_some());
    let want = Want {
        open: closed_because(
            audio.keep_microphone_ready,
            ready.blocked.load(Ordering::Relaxed),
            daemon.meetings().snapshot().is_some(),
        ),
        may_open: daemon.dictation().snapshot().is_none(),
        device: if pinned {
            audio.input_device
        } else {
            String::new()
        },
    };
    ready.reconcile(want, |device| {
        daemon.dictation().open_microphone(daemon, device)
    });
}

/// Reconciles every [`TICK`] off the runtime's worker threads.
pub async fn supervise(daemon: Arc<Daemon>) {
    let mut ticks = tokio::time::interval(TICK);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticks.tick().await;
        let d = daemon.clone();
        if let Err(e) = tokio::task::spawn_blocking(move || tick(&d)).await {
            tracing::error!(error = %e, "microphone ready: reconcile failed this tick");
        }
    }
}

#[cfg(test)]
#[path = "ready_tests.rs"]
mod tests;
