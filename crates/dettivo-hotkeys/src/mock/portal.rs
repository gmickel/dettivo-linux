//! The mock `org.freedesktop.portal.GlobalShortcuts`: `CreateSession`
//! and `BindShortcuts` answered through `Response` signals on the request
//! paths ashpd derives, a `Session` object per session, and `Activated`
//! and `Deactivated` fired on demand.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Type, Value, as_value};
use zbus::{Connection, interface};

use super::Served;

#[derive(Default)]
struct PortalState {
    sessions: Vec<OwnedObjectPath>,
    bound: Vec<String>,
    bind_calls: usize,
    deny: bool,
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

struct SessionIface;

#[interface(name = "org.freedesktop.portal.Session")]
impl SessionIface {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }

    async fn close(&self) {}
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
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let sender = sender_segment(&header);
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
            .at(session_path.clone(), SessionIface)
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
        object_path(request)
    }

    async fn bind_shortcuts(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        _session_handle: ObjectPath<'_>,
        shortcuts: Vec<(String, HashMap<String, OwnedValue>)>,
        _parent_window: String,
        options: HashMap<String, OwnedValue>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let request = format!(
            "/org/freedesktop/portal/desktop/request/{}/{}",
            sender_segment(&header),
            token(&options, "handle_token")
        );
        let deny = {
            let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
            s.bind_calls += 1;
            s.deny
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
                        trigger_description: token(info, "preferred_trigger"),
                    },
                )
            })
            .collect();
        self.state.lock().unwrap_or_else(|p| p.into_inner()).bound =
            bound.iter().map(|(id, _)| id.clone()).collect();
        respond(conn, &request, 0, BindResults { shortcuts: bound }).await?;
        object_path(request)
    }

    async fn list_shortcuts(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] conn: &Connection,
        _session_handle: ObjectPath<'_>,
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
            .bound
            .iter()
            .map(|id| {
                (
                    id.clone(),
                    ShortcutInfo {
                        description: id.clone(),
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
    /// Serves on `address`; `deny` makes every bind answer "Other".
    pub fn serve(address: &str, deny: bool) -> Result<Self, String> {
        let state = Arc::new(Mutex::new(PortalState {
            deny,
            ..PortalState::default()
        }));
        let iface_state = state.clone();
        let served = Served::start(
            move |b| {
                b.name("org.freedesktop.portal.Desktop")?.serve_at(
                    "/org/freedesktop/portal/desktop",
                    PortalIface { state: iface_state },
                )
            },
            address.to_string(),
        )?;
        Ok(Self { served, state })
    }

    /// The ids the last bind registered.
    pub fn bound(&self) -> Vec<String> {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .bound
            .clone()
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
