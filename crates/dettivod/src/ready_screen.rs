//! The compositor's screen lock (ADR 0092). Hyprland tells a client when
//! any `ext-session-lock` locker locks and unlocks the session
//! (`hyprland_lock_notify_v1`), whatever the locker is. Omarchy's
//! Quickshell lock sets nothing in logind, so this is what closes the
//! always-ready microphone on an Omarchy lock. A compositor that does not
//! offer the protocol leaves the lock to logind (`ready_session.rs`).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use wayland_client::protocol::wl_registry;
use wayland_client::{ConnectError, Connection, Dispatch, QueueHandle};

use crate::daemon::Daemon;
use crate::ready::{LockSource, MicReady};

/// The client side of `protocols/hyprland-lock-notify-v1.xml`.
#[allow(missing_docs, non_upper_case_globals, non_camel_case_types, unused)]
#[allow(clippy::all)]
mod protocol {
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_client::backend as wayland_backend;
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/hyprland-lock-notify-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocols/hyprland-lock-notify-v1.xml");
}

use protocol::hyprland_lock_notification_v1::{self as notification, HyprlandLockNotificationV1};
use protocol::hyprland_lock_notifier_v1::HyprlandLockNotifierV1;

/// How long a lost compositor connection waits before the next try.
const RETRY: Duration = Duration::from_secs(5);

/// Why following the lock stopped.
#[derive(Debug)]
pub enum Stop {
    /// The compositor does not offer lock notifications.
    NotOffered,
    /// The connection failed or ended; names why.
    Lost(String),
}

struct Watch {
    report: Box<dyn Fn(bool)>,
    notification: Option<HyprlandLockNotificationV1>,
    /// The compositor reported a lock state.
    reported: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for Watch {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name, interface, ..
        } = event
            && interface == "hyprland_lock_notifier_v1"
            && state.notification.is_none()
        {
            let notifier: HyprlandLockNotifierV1 = registry.bind(name, 1, qh, ());
            state.notification = Some(notifier.get_lock_notification(qh, ()));
        }
    }
}

impl Dispatch<HyprlandLockNotifierV1, ()> for Watch {
    fn event(
        _: &mut Self,
        _: &HyprlandLockNotifierV1,
        _: <HyprlandLockNotifierV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<HyprlandLockNotificationV1, ()> for Watch {
    fn event(
        state: &mut Self,
        _: &HyprlandLockNotificationV1,
        event: notification::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.reported = true;
        match event {
            notification::Event::Locked => (state.report)(true),
            notification::Event::Unlocked => (state.report)(false),
        }
    }
}

/// Reports every lock and unlock the compositor on `conn` sends, until
/// the connection ends.
pub fn follow(conn: &Connection, report: impl Fn(bool) + 'static) -> Result<(), Stop> {
    let lost = |e: &dyn std::fmt::Display| Stop::Lost(e.to_string());
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    conn.display().get_registry(&qh, ());
    let mut watch = Watch {
        report: Box::new(report),
        notification: None,
        reported: false,
    };
    queue.roundtrip(&mut watch).map_err(|e| lost(&e))?;
    if watch.notification.is_none() {
        return Err(Stop::NotOffered);
    }
    // The compositor sends `locked` at once for a session that is locked
    // already; a roundtrip without it means unlocked.
    queue.roundtrip(&mut watch).map_err(|e| lost(&e))?;
    if !watch.reported {
        (watch.report)(false);
    }
    loop {
        queue.blocking_dispatch(&mut watch).map_err(|e| lost(&e))?;
    }
}

/// Follows the compositor's lock on a thread of its own: a lost
/// connection is tried again every [`RETRY`], a compositor without lock
/// notifications or a session without a Wayland display is left to
/// logind. The ring stays closed until the compositor first reports,
/// across lost connections and retries.
pub fn watch(daemon: Arc<Daemon>) {
    let ready: fn(&Daemon) -> &MicReady = |d| d.dictation().ready();
    ready(&daemon).lock_pending(LockSource::Compositor);
    let d = daemon.clone();
    let spawned = std::thread::Builder::new()
        .name("dettivo-screen-lock".into())
        .spawn(move || keep_following(daemon, ready, Connection::connect_to_env, RETRY));
    if let Err(e) = spawned {
        ready(&d).set_lock(LockSource::Compositor, None);
        tracing::error!(error = %e, "microphone ready: the screen lock watcher did not start");
    }
}

/// Follows the compositor `connect` reaches into the ring `ready` reaches
/// from `owner`, connecting again `retry` after a lost connection, until
/// there is no compositor or it offers no lock notifications. A
/// connection lost before the compositor's first report leaves the ring
/// waiting for it, closed, through the retry.
fn keep_following<O: Send + Sync + 'static>(
    owner: Arc<O>,
    ready: fn(&O) -> &MicReady,
    mut connect: impl FnMut() -> Result<Connection, ConnectError>,
    retry: Duration,
) {
    let mut last = String::new();
    let reported = Arc::new(AtomicBool::new(false));
    loop {
        let stop = match connect() {
            Ok(conn) => {
                let (o, r) = (owner.clone(), reported.clone());
                follow(&conn, move |locked| {
                    r.store(true, Ordering::Relaxed);
                    ready(&o).set_lock(LockSource::Compositor, Some(locked));
                })
            }
            Err(ConnectError::NoCompositor) => {
                ready(&owner).set_lock(LockSource::Compositor, None);
                tracing::info!(
                    "microphone ready: no Wayland display, the compositor's lock is not followed"
                );
                return;
            }
            Err(e) => Err(Stop::Lost(e.to_string())),
        };
        let lost = matches!(stop, Err(Stop::Lost(_)));
        if !lost || reported.load(Ordering::Relaxed) {
            ready(&owner).set_lock(LockSource::Compositor, None);
        }
        match stop {
            Err(Stop::Lost(why)) => {
                if why != last {
                    tracing::info!(
                        why,
                        "microphone ready: the compositor's lock is not followed; trying again"
                    );
                    last = why;
                }
            }
            Err(Stop::NotOffered) | Ok(()) => {
                tracing::info!("microphone ready: the compositor offers no lock notifications");
                return;
            }
        }
        std::thread::sleep(retry);
    }
}

#[cfg(test)]
#[path = "ready_screen_tests.rs"]
mod tests;
