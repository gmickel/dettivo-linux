//! The session lock the always-ready microphone follows (ADR 0092): the
//! user's graphical session from logind (`User.Display` of
//! `/org/freedesktop/login1/user/self`), whose `LockedHint` and `Active`
//! properties close the ring while the screen is locked or another
//! session is in front. Without logind the ring follows the setting alone.

use std::sync::Arc;

use futures_util::StreamExt;
use zbus::zvariant::OwnedObjectPath;
use zbus::{Connection, Proxy};

use crate::daemon::Daemon;

const LOGIND: &str = "org.freedesktop.login1";

/// Watches the session until the daemon stops.
pub async fn watch(daemon: Arc<Daemon>) {
    if let Err(e) = follow(&daemon).await {
        tracing::info!(error = %e, "microphone ready: logind unreachable, the session lock is not followed");
    }
}

async fn follow(daemon: &Daemon) -> zbus::Result<()> {
    let conn = Connection::system().await?;
    let user = Proxy::new(
        &conn,
        LOGIND,
        "/org/freedesktop/login1/user/self",
        "org.freedesktop.login1.User",
    )
    .await?;
    let (_, path): (String, OwnedObjectPath) = user.get_property("Display").await?;
    let session = Proxy::new(&conn, LOGIND, path, "org.freedesktop.login1.Session").await?;
    let mut locked_changes = session.receive_property_changed::<bool>("LockedHint").await;
    let mut active_changes = session.receive_property_changed::<bool>("Active").await;
    let mut locked: bool = session.get_property("LockedHint").await?;
    let mut active: bool = session.get_property("Active").await?;
    loop {
        daemon.dictation().ready().set_blocked(locked || !active);
        tokio::select! {
            Some(change) = locked_changes.next() => locked = change.get().await?,
            Some(change) = active_changes.next() => active = change.get().await?,
            else => return Ok(()),
        }
    }
}
