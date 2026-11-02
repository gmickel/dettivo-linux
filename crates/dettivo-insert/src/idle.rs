//! Whether the user touched the keyboard or the pointer between two
//! moments, from the compositor's `ext_idle_notifier_v1` (version 2, the
//! input-idle notification that idle inhibitors do not hold back). The
//! notification fires `resumed` at the first input after [`IDLE_AFTER`]
//! without any; the monitor keeps when those arrived. A compositor
//! without version 2 gives no answer, and a join never happens on a
//! guess (ADR 0091).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1, ext_idle_notifier_v1,
};

/// Input after this long without any counts as new activity.
pub const IDLE_AFTER: Duration = Duration::from_millis(100);

/// How long activity is remembered.
const KEEP: Duration = Duration::from_secs(120);

/// Input activity between two moments.
pub trait InputActivity: Send + Sync {
    /// True when input arrived after `from` and before `until`; `None`
    /// when no source reports input.
    fn active_between(&self, from: Instant, until: Instant) -> Option<bool>;
}

#[derive(Default)]
struct Seen {
    /// The compositor answered with a notification.
    watching: bool,
    resumed: VecDeque<Instant>,
}

/// The compositor's idle notifications, watched on a thread of its own.
#[derive(Clone, Default)]
pub struct IdleMonitor {
    seen: Arc<Mutex<Seen>>,
}

impl InputActivity for IdleMonitor {
    fn active_between(&self, from: Instant, until: Instant) -> Option<bool> {
        let seen = self.seen.lock().unwrap_or_else(|p| p.into_inner());
        seen.watching
            .then(|| seen.resumed.iter().any(|&r| r > from && r < until))
    }
}

struct State {
    seat: Option<wl_seat::WlSeat>,
    notifier: Option<ext_idle_notifier_v1::ExtIdleNotifierV1>,
    seen: Arc<Mutex<Seen>>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_seat" if state.seat.is_none() => {
                    state.seat = Some(registry.bind(name, 1, qh, ()));
                }
                "ext_idle_notifier_v1" if version >= 2 => {
                    state.notifier = Some(registry.bind(name, 2, qh, ()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<ext_idle_notification_v1::ExtIdleNotificationV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ext_idle_notification_v1::ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_idle_notification_v1::Event::Resumed = event {
            let now = Instant::now();
            let mut seen = state.seen.lock().unwrap_or_else(|p| p.into_inner());
            seen.resumed.push_back(now);
            while seen.resumed.front().is_some_and(|&r| now - r > KEEP) {
                seen.resumed.pop_front();
            }
        }
    }
}

delegate_noop!(State: ignore wl_seat::WlSeat);
delegate_noop!(State: ignore ext_idle_notifier_v1::ExtIdleNotifierV1);

impl IdleMonitor {
    /// Starts watching on the session's Wayland display. Without one, or
    /// without the notifier, the monitor answers `None`.
    pub fn start() -> Self {
        let monitor = Self::default();
        let seen = monitor.seen.clone();
        let spawned = std::thread::Builder::new()
            .name("input-idle".into())
            .spawn(move || {
                if let Err(e) = watch(seen) {
                    tracing::info!(reason = %e, "no input-idle signal: consecutive dictations are not joined");
                }
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "input-idle thread not started");
        }
        monitor
    }
}

fn watch(seen: Arc<Mutex<Seen>>) -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("wayland: {e}"))?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let _registry = conn.display().get_registry(&qh, ());
    let mut state = State {
        seat: None,
        notifier: None,
        seen: seen.clone(),
    };
    queue
        .roundtrip(&mut state)
        .map_err(|e| format!("wayland roundtrip: {e}"))?;
    let (Some(seat), Some(notifier)) = (state.seat.clone(), state.notifier.clone()) else {
        return Err("the compositor offers no ext_idle_notifier_v1 version 2".into());
    };
    let timeout = u32::try_from(IDLE_AFTER.as_millis()).unwrap_or(u32::MAX);
    let _notification = notifier.get_input_idle_notification(timeout, &seat, &qh, ());
    seen.lock().unwrap_or_else(|p| p.into_inner()).watching = true;
    let ended = loop {
        if let Err(e) = queue.blocking_dispatch(&mut state) {
            break format!("wayland dispatch: {e}");
        }
    };
    seen.lock().unwrap_or_else(|p| p.into_inner()).watching = false;
    Err(ended)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_is_reported_only_inside_the_window_and_only_when_watched() {
        let monitor = IdleMonitor::default();
        let t0 = Instant::now();
        let t = |ms| t0 + Duration::from_millis(ms);
        assert_eq!(monitor.active_between(t0, t(100)), None, "no source");
        {
            let mut seen = monitor.seen.lock().unwrap();
            seen.watching = true;
            seen.resumed.push_back(t(50));
        }
        assert_eq!(monitor.active_between(t0, t(100)), Some(true));
        assert_eq!(monitor.active_between(t(50), t(100)), Some(false));
        assert_eq!(monitor.active_between(t0, t(50)), Some(false));
    }
}
