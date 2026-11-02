//! What a test reads back from the mock portal and does to it: the ids
//! bound, the binds asked, the live sessions, options changed under it,
//! and `Activated` and `Deactivated` fired for a shortcut id.

use std::collections::HashMap;

use zbus::object_server::SignalEmitter;
use zbus::zvariant::Value;

use super::{MockOptions, MockPortal};

impl MockPortal {
    /// Behaves as `options` say from the next request on.
    pub fn set_options(&self, options: MockOptions) {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).options = Some(options);
    }

    /// How many sessions are open; a closed one is gone.
    pub fn sessions(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sessions
            .len()
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
