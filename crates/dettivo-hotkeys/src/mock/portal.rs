//! The mock `org.freedesktop.portal.GlobalShortcuts`: `CreateSession`
//! and `BindShortcuts` answered through `Response` signals on the request
//! paths ashpd derives, a `Session` object per session, and `Activated`
//! and `Deactivated` fired on demand. Beside it the host registry
//! `org.freedesktop.host.portal.Registry`: like the stock portal, a
//! session is refused (`NotAllowed`) to a connection that registered no
//! app id, and a registration after the connection's first request is
//! refused. [`MockOptions`] leaves the registry out (an older portal) or
//! answers binds with no triggers (Hyprland's portal).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Type, Value, as_value};
use zbus::{Connection, interface};

use super::Served;

/// How the mock portal behaves.
#[derive(Debug, Clone, Copy)]
pub struct MockOptions {
    /// Every bind answers "Other".
    pub deny: bool,
    /// The portal offers the host registry and requires an app id.
    pub registry: bool,
    /// Binds come back with empty trigger descriptions (Hyprland).
    pub empty_triggers: bool,
}

impl Default for MockOptions {
    fn default() -> Self {
        Self {
            deny: false,
            registry: true,
            empty_triggers: false,
        }
    }
}

#[derive(Default)]
struct PortalState {
    /// Live sessions, oldest first.
    sessions: Vec<OwnedObjectPath>,
    /// The ids each session's last bind registered.
    bound: Vec<(OwnedObjectPath, Vec<String>)>,
    bind_calls: usize,
    options: Option<MockOptions>,
    /// Senders with their registered app id.
    registered: Vec<(String, String)>,
    /// Senders that made a portal request.
    requested: Vec<String>,
}

impl PortalState {
    fn options(&self) -> MockOptions {
        self.options.unwrap_or_default()
    }

    fn bound_to(&self, session: &str) -> Vec<String> {
        self.bound
            .iter()
            .find(|(s, _)| s.as_str() == session)
            .map(|(_, ids)| ids.clone())
            .unwrap_or_default()
    }
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.portal.Error")]
enum PortalError {
    #[zbus(error)]
    ZBus(zbus::Error),
    NotAllowed(String),
}

impl From<zbus::fdo::Error> for PortalError {
    fn from(e: zbus::fdo::Error) -> Self {
        Self::ZBus(e.into())
    }
}

#[derive(Serialize, Type)]
#[zvariant(signature = "dict")]
struct SessionResults {
    #[serde(with = "as_value")]
    session_handle: String,
}

#[derive(Serialize, Type, Clone)]
#[zvariant(signature = "dict")]
struct ShortcutInfo {
    #[serde(with = "as_value")]
    description: String,
    #[serde(with = "as_value")]
    trigger_description: String,
}

#[derive(Serialize, Type)]
#[zvariant(signature = "dict")]
struct BindResults {
    #[serde(with = "as_value")]
    shortcuts: Vec<(String, ShortcutInfo)>,
}

struct PortalIface {
    state: Arc<Mutex<PortalState>>,
}

fn token(options: &HashMap<String, OwnedValue>, key: &str) -> String {
    options
        .get(key)
        .and_then(|v| v.downcast_ref::<&str>().ok())
        .unwrap_or("token")
        .to_string()
}

fn object_path(path: String) -> zbus::fdo::Result<OwnedObjectPath> {
    OwnedObjectPath::try_from(path).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
}

fn sender_segment(header: &Header<'_>) -> String {
    header
        .sender()
        .map(|s| s.as_str().trim_start_matches(':').replace('.', "_"))
        .unwrap_or_default()
}

async fn respond<R: Serialize + Type>(
    conn: &Connection,
    path: &str,
    code: u32,
    results: R,
) -> zbus::Result<()> {
    SignalEmitter::new(conn, path.to_string())?
        .emit(
            "org.freedesktop.portal.Request",
            "Response",
            &(code, results),
        )
        .await
}

struct RegistryIface {
    state: Arc<Mutex<PortalState>>,
}

#[interface(name = "org.freedesktop.host.portal.Registry")]
impl RegistryIface {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }

    async fn register(
        &self,
        #[zbus(header)] header: Header<'_>,
        app_id: String,
        _options: HashMap<String, OwnedValue>,
    ) -> Result<(), PortalError> {
        let sender = sender_segment(&header);
        let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if s.requested.contains(&sender) {
            return Err(PortalError::NotAllowed(
                "Connection already associated with an application ID".into(),
            ));
        }
        s.registered.push((sender, app_id));
        Ok(())
    }
}

/// A session; closing it drops its shortcuts, as the stock portal does.
struct SessionIface {
    state: Arc<Mutex<PortalState>>,
    path: OwnedObjectPath,
}

#[interface(name = "org.freedesktop.portal.Session")]
impl SessionIface {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }

    async fn close(&self) {
        let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
        s.sessions.retain(|p| *p != self.path);
        s.bound.retain(|(p, _)| *p != self.path);
    }
}

#[interface(name = "org.freedesktop.portal.GlobalShortcuts")]
impl PortalIface {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }

    async fn create_session(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        options: HashMap<String, OwnedValue>,
    ) -> Result<OwnedObjectPath, PortalError> {
        let sender = sender_segment(&header);
        {
            let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
            let known = s.registered.iter().any(|(who, _)| *who == sender);
            if s.options().registry && !known {
                return Err(PortalError::NotAllowed("An app id is required".into()));
            }
            s.requested.push(sender.clone());
        }
        let request = format!(
            "/org/freedesktop/portal/desktop/request/{sender}/{}",
            token(&options, "handle_token")
        );
        let session = format!(
            "/org/freedesktop/portal/desktop/session/{sender}/{}",
            token(&options, "session_handle_token")
        );
        let session_path = object_path(session.clone())?;
        conn.object_server()
            .at(
                session_path.clone(),
                SessionIface {
                    state: self.state.clone(),
                    path: session_path.clone(),
                },
            )
            .await?;
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sessions
            .push(session_path);
        respond(
            conn,
            &request,
            0,
            SessionResults {
                session_handle: session,
            },
        )
        .await?;
        Ok(object_path(request)?)
    }

    async fn bind_shortcuts(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        session_handle: ObjectPath<'_>,
        shortcuts: Vec<(String, HashMap<String, OwnedValue>)>,
        _parent_window: String,
        options: HashMap<String, OwnedValue>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let request = format!(
            "/org/freedesktop/portal/desktop/request/{}/{}",
            sender_segment(&header),
            token(&options, "handle_token")
        );
        let (deny, empty) = {
            let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
            s.bind_calls += 1;
            (s.options().deny, s.options().empty_triggers)
        };
        if deny {
            respond(
                conn,
                &request,
                2,
                BindResults {
                    shortcuts: Vec::new(),
                },
            )
            .await?;
            return object_path(request);
        }
        let bound: Vec<(String, ShortcutInfo)> = shortcuts
            .iter()
            .map(|(id, info)| {
                (
                    id.clone(),
                    ShortcutInfo {
                        description: token(info, "description"),
                        trigger_description: if empty {
                            String::new()
                        } else {
                            token(info, "preferred_trigger")
                        },
                    },
                )
            })
            .collect();
        {
            let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
            let session: OwnedObjectPath = session_handle.into();
            s.bound.retain(|(p, _)| *p != session);
            s.bound
                .push((session, bound.iter().map(|(id, _)| id.clone()).collect()));
        }
        respond(conn, &request, 0, BindResults { shortcuts: bound }).await?;
        object_path(request)
    }

    async fn list_shortcuts(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        session_handle: ObjectPath<'_>,
        options: HashMap<String, OwnedValue>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let request = format!(
            "/org/freedesktop/portal/desktop/request/{}/{}",
            sender_segment(&header),
            token(&options, "handle_token")
        );
        let bound: Vec<(String, ShortcutInfo)> = self
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .bound_to(session_handle.as_str())
            .into_iter()
            .map(|id| {
                (
                    id.clone(),
                    ShortcutInfo {
                        description: id,
                        trigger_description: String::new(),
                    },
                )
            })
            .collect();
        respond(conn, &request, 0, BindResults { shortcuts: bound }).await?;
        object_path(request)
    }
}

/// The mock portal, owning `org.freedesktop.portal.Desktop` on the bus.
pub struct MockPortal {
    served: Served,
    state: Arc<Mutex<PortalState>>,
}

impl MockPortal {
    /// Serves on `address` with the registry; `deny` makes every bind
    /// answer "Other".
    pub fn serve(address: &str, deny: bool) -> Result<Self, String> {
        Self::serve_with(
            address,
            MockOptions {
                deny,
                ..MockOptions::default()
            },
        )
    }

    /// Serves on `address` behaving as `options` say.
    pub fn serve_with(address: &str, options: MockOptions) -> Result<Self, String> {
        let state = Arc::new(Mutex::new(PortalState {
            options: Some(options),
            ..PortalState::default()
        }));
        let iface_state = state.clone();
        let registry_state = state.clone();
        let served = Served::start(
            move |b| {
                let b = b.name("org.freedesktop.portal.Desktop")?.serve_at(
                    "/org/freedesktop/portal/desktop",
                    PortalIface { state: iface_state },
                )?;
                if !options.registry {
                    return Ok(b);
                }
                b.serve_at(
                    "/org/freedesktop/portal/desktop",
                    RegistryIface {
                        state: registry_state,
                    },
                )
            },
            address.to_string(),
        )?;
        Ok(Self { served, state })
    }

    /// The app ids registered, in order.
    pub fn registered(&self) -> Vec<String> {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .registered
            .iter()
            .map(|(_, id)| id.clone())
            .collect()
    }

    /// The ids bound across the live sessions; a closed session's ids
    /// are gone.
    pub fn bound(&self) -> Vec<String> {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .bound
            .iter()
            .flat_map(|(_, ids)| ids.iter().cloned())
            .collect()
    }

    /// How many bind requests arrived.
    pub fn bind_calls(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .bind_calls
    }

    fn signal(&self, name: &'static str, id: &str) -> Result<(), String> {
        let session = self
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sessions
            .last()
            .cloned()
            .ok_or("no portal session")?;
        let conn = self.served.connection.clone();
        let id = id.to_string();
        self.served.run(async move {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let options: HashMap<&str, Value<'_>> = HashMap::new();
            SignalEmitter::new(&conn, "/org/freedesktop/portal/desktop")
                .map_err(|e| e.to_string())?
                .emit(
                    "org.freedesktop.portal.GlobalShortcuts",
                    name,
                    &(session, id, stamp, options),
                )
                .await
                .map_err(|e| e.to_string())
        })?
    }

    /// Fires `Activated` for a shortcut id.
    pub fn press(&self, id: &str) -> Result<(), String> {
        self.signal("Activated", id)
    }

    /// Fires `Deactivated` for a shortcut id.
    pub fn release(&self, id: &str) -> Result<(), String> {
        self.signal("Deactivated", id)
    }
}
