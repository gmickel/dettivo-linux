//! The daemon-side hotkey service: picks the backend `[hotkeys] backend`
//! names (`auto` takes the portal when the desktop answers and otherwise
//! nothing, since compositor bindings call the CLI), starts it on a thread
//! of its own so a portal dialog never delays the socket, restarts it when
//! the section changes, and reports what runs and why the rest does not.

use std::sync::{Arc, Mutex, Weak};

use dettivo_core::config::Loaded;
use dettivo_core::config::schema::Hotkeys as Section;
use dettivo_hotkeys::evdev::EvdevBackend;
use dettivo_hotkeys::portal::{PortalBackend, probe};
use dettivo_hotkeys::{Availability, HotkeyBackend, HotkeyEvent, Keys};
use dettivo_proto::methods::hotkeys::{BackendAvailability, StatusResult};

use crate::daemon::Daemon;

struct Inner {
    section: Option<Section>,
    backend: Option<Arc<dyn HotkeyBackend>>,
    active: String,
    bound: Vec<String>,
    error: Option<String>,
    last_press_at: Option<String>,
}

/// The service.
pub struct Hotkeys {
    inner: Mutex<Inner>,
    daemon: Mutex<Weak<Daemon>>,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self::new()
    }
}

impl Hotkeys {
    /// Nothing running yet.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                section: None,
                backend: None,
                active: "none".into(),
                bound: Vec::new(),
                error: None,
                last_press_at: None,
            }),
            daemon: Mutex::new(Weak::new()),
        }
    }

    /// Starts the configured backend in the background.
    pub fn start(&self, daemon: &Arc<Daemon>) {
        *self.daemon.lock().unwrap_or_else(|p| p.into_inner()) = Arc::downgrade(daemon);
        let section = daemon.config().config.hotkeys.clone();
        self.inner.lock().unwrap_or_else(|p| p.into_inner()).section = Some(section.clone());
        let daemon = daemon.clone();
        std::thread::Builder::new()
            .name("hotkeys".into())
            .spawn(move || daemon.hotkeys().activate(&daemon, &section))
            .map(|_| ())
            .unwrap_or_else(|e| tracing::warn!(error = %e, "hotkeys: cannot start the thread"));
    }

    /// Restarts the backend when `[hotkeys]` changed.
    pub fn apply(&self, daemon: &Arc<Daemon>, loaded: &Loaded) {
        let section = loaded.config.hotkeys.clone();
        let changed = self
            .inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .section
            .as_ref()
            .is_some_and(|s| {
                s.backend != section.backend
                    || keys_of(s) != keys_of(&section)
                    || s.evdev_devices != section.evdev_devices
            });
        if !changed {
            return;
        }
        tracing::info!("hotkeys: [hotkeys] changed, restarting the backend");
        self.shutdown();
        self.start(daemon);
    }

    fn set(
        &self,
        active: &str,
        bound: Vec<String>,
        error: Option<String>,
        backend: Option<Arc<dyn HotkeyBackend>>,
    ) {
        let mut g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        g.active = active.to_string();
        g.bound = bound;
        g.error = error;
        g.backend = backend;
    }

    fn activate(&self, daemon: &Arc<Daemon>, section: &Section) {
        let keys = match keys_of(section) {
            Ok(k) => k,
            Err(e) => {
                tracing::warn!(error = %e, "hotkeys: chord not usable");
                self.set("none", Vec::new(), Some(e.to_string()), None);
                return;
            }
        };
        // One worker drains the events in order; the backend's handler only
        // queues, so key repeat or a noisy device never fans out into threads.
        // The worker ends when the backend drops its handler.
        let weak = Arc::downgrade(daemon);
        let (tx, rx) = std::sync::mpsc::channel::<HotkeyEvent>();
        let spawned = std::thread::Builder::new()
            .name("hotkey-actions".into())
            .spawn(move || {
                for event in rx {
                    match weak.upgrade() {
                        Some(d) => crate::actions::on_hotkey(&d, event),
                        None => break,
                    }
                }
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "hotkeys: cannot start the action worker");
            self.set("none", Vec::new(), Some(e.to_string()), None);
            return;
        }
        let handler: dettivo_hotkeys::Handler = Arc::new(move |event: HotkeyEvent| {
            if tx.send(event).is_err() {
                tracing::debug!("hotkeys: action worker gone; event dropped");
            }
        });
        let backend: Arc<dyn HotkeyBackend> = match section.backend.as_str() {
            "none" => {
                tracing::info!("hotkeys: backend none; compositor bindings drive the CLI");
                self.set("none", Vec::new(), None, None);
                return;
            }
            "auto" => match probe() {
                Availability::Available => Arc::new(PortalBackend::new(keys)),
                Availability::Unavailable(reason) => {
                    tracing::info!(reason = %reason, "hotkeys: no portal; compositor bindings drive the CLI");
                    self.set("none", Vec::new(), None, None);
                    return;
                }
            },
            "portal" => Arc::new(PortalBackend::new(keys)),
            "evdev" => Arc::new(EvdevBackend::new(keys, section.evdev_devices.clone())),
            other => {
                let e =
                    format!("unknown hotkeys.backend {other:?}; use auto, portal, evdev or none");
                tracing::warn!(error = %e, "hotkeys: backend not started");
                self.set("none", Vec::new(), Some(e), None);
                return;
            }
        };
        match backend.start(handler) {
            Ok(registration) => {
                tracing::info!(
                    backend = registration.backend,
                    bound = registration.bound.len(),
                    "hotkeys: backend started"
                );
                let bound = registration
                    .bound
                    .iter()
                    .map(|a| a.id().to_string())
                    .collect();
                self.set(registration.backend, bound, None, Some(backend));
            }
            Err(e) if section.backend == "auto" => {
                // Nothing was asked for by name: the compositor bindings
                // remain the path, and `hotkeys.status` carries the reason.
                tracing::info!(backend = backend.name(), reason = %e, "hotkeys: backend not started; compositor bindings drive the CLI");
                self.set("none", Vec::new(), Some(e), None);
            }
            Err(e) => {
                tracing::warn!(backend = backend.name(), error = %e, "hotkeys: backend not started");
                self.set("none", Vec::new(), Some(e), None);
            }
        }
    }

    /// Records that a press or release reached the daemon through its
    /// backend; `hotkeys.status.last_press_at` carries the moment, which
    /// is how the first-run Keys step confirms a key live.
    pub fn note_press(&self) {
        self.inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .last_press_at = Some(crate::events::now_iso());
    }

    /// The status for `hotkeys.status`; the portal and evdev availability
    /// are probed now, so the answer is current.
    pub fn status(&self, loaded: &Loaded) -> StatusResult {
        let section = &loaded.config.hotkeys;
        let (active, bound, error, last_press_at) = {
            let g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            (
                g.active.clone(),
                g.bound.clone(),
                g.error.clone(),
                g.last_press_at.clone(),
            )
        };
        let portal = availability(probe());
        let evdev = availability(match keys_of(section) {
            Ok(keys) => EvdevBackend::new(keys, section.evdev_devices.clone()).availability(),
            Err(e) => Availability::Unavailable(e.to_string()),
        });
        StatusResult {
            backend: active,
            requested: section.backend.clone(),
            portal,
            evdev,
            bound,
            error,
            last_press_at,
        }
    }

    /// The `system.capabilities.hotkeys` block.
    pub fn capabilities(&self, loaded: &Loaded) -> dettivo_proto::capabilities::HotkeyCaps {
        let status = self.status(loaded);
        let mut available = Vec::new();
        if status.portal.available {
            available.push("portal".to_string());
        }
        if status.evdev.available {
            available.push("evdev".to_string());
        }
        dettivo_proto::capabilities::HotkeyCaps {
            backend: status.backend,
            available,
        }
    }

    /// Stops the backend.
    pub fn shutdown(&self) {
        let backend = self
            .inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .backend
            .take();
        if let Some(b) = backend {
            b.stop();
        }
        self.set("none", Vec::new(), None, None);
    }
}

fn keys_of(section: &Section) -> Result<Keys, dettivo_hotkeys::chord::ChordError> {
    Keys::parse(
        &section.hold,
        &section.toggle,
        &section.cancel,
        &section.reinsert,
    )
}

fn availability(a: Availability) -> BackendAvailability {
    BackendAvailability {
        available: a.is_available(),
        reason: a.reason().map(str::to_string),
    }
}
