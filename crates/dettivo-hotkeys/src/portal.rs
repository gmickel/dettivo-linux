//! The portal backend (FR-H3): `org.freedesktop.portal.GlobalShortcuts`
//! through `ashpd`. The connection registers the app id first (ADR 0083),
//! then one session binds the four actions under ids that carry their
//! chords, with the chords as preferred triggers; `Activated` maps to a
//! press and `Deactivated` to a release. A desktop without the interface,
//! or one that refuses the request, is reported once with its reason and
//! never retried here. A portal backend that restarts (Hyprland's exits
//! with the compositor) loses the session; the shortcuts are bound again
//! in a new one once it is back, a refusal of that bind is final, and the
//! outcome is what [`HotkeyBackend::current`] reports.

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;

use bind::{STOP_POLL, Unbound, close};

use crate::identity::register;
use crate::{Action, Availability, Handler, HotkeyBackend, HotkeyEvent, Keys, Registration};

mod bind;

/// `Activated` and `Deactivated`: session, shortcut id, timestamp, options.
type SignalBody = (
    zbus::zvariant::OwnedObjectPath,
    String,
    u64,
    std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
);

/// How long the portal may take to answer a bind (a dialog counts).
const BIND_TIMEOUT: Duration = Duration::from_secs(120);

/// The bus names of the portal backends behind `xdg-desktop-portal`.
const BACKEND_PREFIX: &str = "org.freedesktop.impl.portal.desktop.";

/// Tries at binding again after a portal backend restarted.
const REBIND_TRIES: usize = 5;

/// The reason reported when the desktop has no GlobalShortcuts interface.
pub const NO_INTERFACE: &str = "the desktop portal has no GlobalShortcuts interface";

/// Runs an async job on its own thread with its own runtime, so the caller
/// may sit inside the daemon's runtime or none at all.
pub(crate) fn on_own_runtime<T: Send, F: Future<Output = Result<T, String>>>(
    job: impl FnOnce() -> F + Send,
) -> Result<T, String> {
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("runtime: {e}"))?;
                runtime.block_on(job())
            })
            .join()
            .unwrap_or_else(|_| Err("portal thread panicked".to_string()))
    })
}

/// The reason reported when no portal owns `org.freedesktop.portal.Desktop`.
pub const NO_PORTAL: &str =
    "no desktop portal is running (org.freedesktop.portal.Desktop has no owner)";

/// The session bus, or the one at `address` (tests).
pub(crate) async fn connect(address: Option<&str>) -> Result<zbus::Connection, String> {
    match address {
        Some(a) => zbus::connection::Builder::address(a)
            .map_err(|e| format!("bus address: {e}"))?
            .build()
            .await
            .map_err(|e| format!("bus at {a}: {e}")),
        None => zbus::Connection::session()
            .await
            .map_err(|e| format!("no session bus: {e}")),
    }
}

/// A connection to the bus with a portal on it, or why there is none.
async fn portal_connection(address: Option<&str>) -> Result<zbus::Connection, String> {
    let connection = connect(address).await?;
    let owned = zbus::fdo::DBusProxy::new(&connection)
        .await
        .map_err(|e| format!("bus: {e}"))?
        .name_has_owner(
            zbus::names::BusName::try_from("org.freedesktop.portal.Desktop")
                .map_err(|e| e.to_string())?,
        )
        .await
        .map_err(|e| format!("bus: {e}"))?;
    if !owned {
        return Err(NO_PORTAL.to_string());
    }
    Ok(connection)
}

/// The interface on `connection`, or a portal without it (its `version`
/// property is the probe, as `xdg-desktop-portal` answers `InvalidArgs`
/// for an interface it does not implement).
async fn shortcuts_on(connection: zbus::Connection) -> Result<GlobalShortcuts, String> {
    let raw = zbus::Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.GlobalShortcuts",
    )
    .await
    .map_err(|e| format!("portal proxy: {e}"))?;
    raw.get_property::<u32>("version")
        .await
        .map_err(|e| format!("{NO_INTERFACE} ({e})"))?;
    GlobalShortcuts::with_connection(connection)
        .await
        .map_err(|e| match e {
            ashpd::Error::PortalNotFound(_) => NO_INTERFACE.to_string(),
            other => format!("portal: {other}"),
        })
}

/// Probes the interface on the session bus without binding anything.
pub fn probe() -> Availability {
    probe_at(None)
}

/// Probes the interface on the bus at `address` (`None`: the session bus).
pub fn probe_at(address: Option<&str>) -> Availability {
    let reach = || async {
        let connection = portal_connection(address).await?;
        shortcuts_on(connection).await.map(|_| ())
    };
    match on_own_runtime(reach) {
        Ok(()) => Availability::Available,
        Err(reason) => Availability::Unavailable(reason),
    }
}

/// The backend.
pub struct PortalBackend {
    keys: Keys,
    address: Option<String>,
    running: Mutex<Option<Running>>,
}

struct Running {
    stop: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<()>,
    current: Current,
}

/// What the session delivers now, shared with the delivery thread.
type Current = Arc<Mutex<Result<Registration, String>>>;

impl PortalBackend {
    /// A backend binding `keys` as the preferred triggers.
    pub fn new(keys: Keys) -> Self {
        Self {
            keys,
            address: None,
            running: Mutex::new(None),
        }
    }

    /// A backend on the bus at `address` instead of the session bus.
    pub fn at(keys: Keys, address: &str) -> Self {
        Self {
            address: Some(address.to_string()),
            ..Self::new(keys)
        }
    }

    fn shortcuts(keys: &Keys) -> Vec<NewShortcut> {
        Action::ALL
            .into_iter()
            .map(|a| {
                let trigger = keys.chord(a).portal_trigger();
                NewShortcut::new(keys.shortcut_id(a), a.description())
                    .preferred_trigger(trigger.as_str())
            })
            .collect()
    }

    /// Registers the app id, creates the session, binds, then delivers
    /// until `stop`, binding again whenever a portal backend comes back
    /// after going away (ADR 0100) and keeping `current` to what the
    /// session delivers. Closing the session on stop drops its shortcuts,
    /// so a rebind leaves only the new ids bound.
    async fn serve(
        address: Option<String>,
        keys: Keys,
        handler: Handler,
        stop: Arc<AtomicBool>,
        current: Current,
        ready: std::sync::mpsc::Sender<Result<Registration, String>>,
    ) {
        let shortcuts = Self::shortcuts(&keys);
        let opened: Result<_, String> = async {
            let connection = portal_connection(address.as_deref()).await?;
            let identity = register(&connection).await?;
            let owners = zbus::fdo::DBusProxy::new(&connection)
                .await
                .map_err(|e| format!("bus: {e}"))?
                .receive_name_owner_changed()
                .await
                .map_err(|e| format!("bus: {e}"))?;
            let portal = shortcuts_on(connection).await?;
            // One stream for both signals keeps a press ahead of its release.
            let signals = portal
                .receive_all_signals()
                .await
                .map_err(|e| format!("portal signals: {e}"))?;
            let (session, bound) = Self::open(&portal, &shortcuts, &stop)
                .await
                .map_err(Unbound::reason)?;
            let registration = Self::registration(&keys, identity, &bound);
            Ok((portal, session, identity, registration, signals, owners))
        }
        .await;
        let (portal, mut session, identity, registration, mut signals, mut owners) = match opened {
            Ok(opened) => opened,
            Err(e) => {
                let _ = ready.send(Err(e));
                return;
            }
        };
        *current.lock().unwrap_or_else(|p| p.into_inner()) = Ok(registration.clone());
        let _ = ready.send(Ok(registration));
        // Portal backends seen going away; one coming back gets a rebind.
        let mut gone = std::collections::HashSet::new();
        loop {
            tokio::select! {
                Some(message) = signals.next() => {
                    let member = message
                        .header()
                        .member()
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default();
                    let body = message.body().deserialize::<SignalBody>();
                    let Ok((_, id, _, _)) = body else { continue };
                    let Some(action) = keys.action_for(&id) else { continue };
                    match member.as_str() {
                        "Activated" => handler(HotkeyEvent::Press(action)),
                        "Deactivated" => handler(HotkeyEvent::Release(action)),
                        _ => {}
                    }
                }
                Some(change) = owners.next() => {
                    let Ok(args) = change.args() else { continue };
                    let name = args.name().to_string();
                    if !name.starts_with(BACKEND_PREFIX) {
                        continue;
                    }
                    if args.new_owner().is_none() {
                        gone.insert(name);
                        continue;
                    }
                    if !gone.remove(&name) {
                        continue;
                    }
                    close(&session).await;
                    let now = match Self::rebind(&portal, &shortcuts, &stop).await {
                        Ok((new, bound)) => {
                            tracing::info!("hotkeys: the portal backend restarted; shortcuts bound again");
                            session = new;
                            Ok(Self::registration(&keys, identity, &bound))
                        }
                        Err(Unbound::Stopped) => break,
                        Err(e) => {
                            let e = format!(
                                "the shortcuts were not bound again after the portal backend restarted: {}",
                                e.reason()
                            );
                            tracing::warn!(error = %e, "hotkeys: the portal shortcuts stay unbound until the portal backend restarts again or the daemon reloads [hotkeys]");
                            Err(e)
                        }
                    };
                    *current.lock().unwrap_or_else(|p| p.into_inner()) = now;
                }
                () = tokio::time::sleep(STOP_POLL * 5) => {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                }
            }
        }
        close(&session).await;
    }
}

impl HotkeyBackend for PortalBackend {
    fn name(&self) -> &'static str {
        "portal"
    }

    fn availability(&self) -> Availability {
        probe_at(self.address.as_deref())
    }

    fn start(&self, handler: Handler) -> Result<Registration, String> {
        let keys = self.keys.clone();
        let address = self.address.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel();
        let stop_flag = stop.clone();
        let current: Current = Arc::new(Mutex::new(Err("the portal is binding".to_string())));
        let shared = current.clone();
        let thread = std::thread::Builder::new()
            .name("hotkeys-portal".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = tx.send(Err(format!("runtime: {e}")));
                        return;
                    }
                };
                runtime.block_on(Self::serve(address, keys, handler, stop_flag, shared, tx));
            })
            .map_err(|e| format!("portal thread: {e}"))?;
        let registration = rx
            .recv()
            .unwrap_or_else(|_| Err("the portal thread ended before binding".to_string()));
        match registration {
            Ok(r) => {
                *self.running.lock().unwrap_or_else(|p| p.into_inner()) = Some(Running {
                    stop,
                    thread,
                    current,
                });
                Ok(r)
            }
            Err(e) => {
                let _ = thread.join();
                Err(e)
            }
        }
    }

    fn stop(&self) {
        let running = self
            .running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        if let Some(r) = running {
            r.stop.store(true, Ordering::Relaxed);
            let _ = r.thread.join();
        }
    }

    fn current(&self) -> Option<Result<Registration, String>> {
        let running = self.running.lock().unwrap_or_else(|p| p.into_inner());
        let r = running.as_ref()?;
        Some(r.current.lock().unwrap_or_else(|p| p.into_inner()).clone())
    }
}
