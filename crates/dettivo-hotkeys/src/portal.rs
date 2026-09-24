//! The portal backend (FR-H3): `org.freedesktop.portal.GlobalShortcuts`
//! through `ashpd`. One session, the four actions bound with their
//! preferred triggers, `Activated` mapped to a press and `Deactivated` to
//! a release. A desktop without the interface, or one that refuses the
//! request, is reported once with its reason and never retried here.

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ashpd::desktop::CreateSessionOptions;
use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;

use crate::{Action, Availability, Handler, HotkeyBackend, HotkeyEvent, Keys, Registration};

/// `Activated` and `Deactivated`: session, shortcut id, timestamp, options.
type SignalBody = (
    zbus::zvariant::OwnedObjectPath,
    String,
    u64,
    std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
);

/// How long the portal may take to answer a bind (a dialog counts).
const BIND_TIMEOUT: Duration = Duration::from_secs(120);

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

/// The interface, or why it cannot be reached: no bus, no portal on the
/// bus, or a portal without the interface (its `version` property is the
/// probe, as `xdg-desktop-portal` answers `InvalidArgs` for an interface
/// it does not implement).
async fn proxy(address: Option<&str>) -> Result<GlobalShortcuts, String> {
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
    match on_own_runtime(|| async { proxy(address).await.map(|_| ()) }) {
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
}

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

    fn shortcuts(&self) -> Vec<NewShortcut> {
        Action::ALL
            .into_iter()
            .map(|a| {
                let trigger = self.keys.chord(a).portal_trigger();
                NewShortcut::new(a.id(), a.description()).preferred_trigger(trigger.as_str())
            })
            .collect()
    }

    /// Creates the session, binds, then delivers until `stop`.
    async fn serve(
        address: Option<String>,
        shortcuts: Vec<NewShortcut>,
        handler: Handler,
        stop: Arc<AtomicBool>,
        ready: std::sync::mpsc::Sender<Result<Registration, String>>,
    ) {
        let outcome: Result<_, String> = async {
            let portal = proxy(address.as_deref()).await?;
            let session = portal
                .create_session(CreateSessionOptions::default())
                .await
                .map_err(|e| format!("portal session refused: {e}"))?;
            // One stream for both signals keeps a press ahead of its release.
            let mut signals = portal
                .receive_all_signals()
                .await
                .map_err(|e| format!("portal signals: {e}"))?;
            let bound = tokio::time::timeout(
                BIND_TIMEOUT,
                portal.bind_shortcuts(&session, &shortcuts, None, BindShortcutsOptions::default()),
            )
            .await
            .map_err(|_| "the portal did not answer the bind request".to_string())?
            .map_err(|e| format!("portal bind refused: {e}"))?
            .response()
            .map_err(|e| format!("the portal denied the shortcuts: {e}"))?;
            let actions: Vec<Action> = bound
                .shortcuts()
                .iter()
                .filter_map(|s| Action::from_id(s.id()))
                .collect();
            Ok((session, actions, async move {
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
                            let Some(action) = Action::from_id(&id) else { continue };
                            match member.as_str() {
                                "Activated" => handler(HotkeyEvent::Press(action)),
                                "Deactivated" => handler(HotkeyEvent::Release(action)),
                                _ => {}
                            }
                        }
                        () = tokio::time::sleep(Duration::from_millis(250)) => {
                            if stop.load(Ordering::Relaxed) {
                                return;
                            }
                        }
                    }
                }
            }))
        }
        .await;
        match outcome {
            Ok((session, actions, deliver)) => {
                let _ = ready.send(Ok(Registration {
                    backend: "portal",
                    bound: actions,
                }));
                deliver.await;
                let _ = session.close().await;
            }
            Err(e) => {
                let _ = ready.send(Err(e));
            }
        }
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
        let shortcuts = self.shortcuts();
        let address = self.address.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel();
        let stop_flag = stop.clone();
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
                runtime.block_on(Self::serve(address, shortcuts, handler, stop_flag, tx));
            })
            .map_err(|e| format!("portal thread: {e}"))?;
        let registration = rx
            .recv()
            .unwrap_or_else(|_| Err("the portal thread ended before binding".to_string()));
        match registration {
            Ok(r) => {
                *self.running.lock().unwrap_or_else(|p| p.into_inner()) =
                    Some(Running { stop, thread });
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
}
