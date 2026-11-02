//! The session lock the always-ready microphone follows (ADR 0092): the
//! user's graphical session from logind (`User.Display` of
//! `/org/freedesktop/login1/user/self`), whose `LockedHint` and `Active`
//! properties close the ring while the screen is locked or another
//! session is in front. Without logind the ring follows the compositor's
//! lock (`ready_screen.rs`) alone.

use std::future::Future;
use std::sync::Arc;

use futures_util::StreamExt;
use zbus::zvariant::OwnedObjectPath;
use zbus::{Connection, Proxy};

use crate::daemon::Daemon;
use crate::ready::{LockSource, MicReady};

const LOGIND: &str = "org.freedesktop.login1";

/// Watches the session on the system bus until the daemon stops.
pub fn watch(daemon: Arc<Daemon>) -> impl Future<Output = ()> + Send + 'static {
    follow(daemon, |d| d.dictation().ready(), Connection::system())
}

/// Follows logind on `bus` into the ring `ready` reaches from `owner`.
/// The ring stays closed from this call until the first read of the
/// session; an unreachable logind leaves the lock to the compositor.
fn follow<O: Send + Sync + 'static>(
    owner: Arc<O>,
    ready: fn(&O) -> &MicReady,
    bus: impl Future<Output = zbus::Result<Connection>> + Send + 'static,
) -> impl Future<Output = ()> + Send + 'static {
    ready(&owner).lock_pending(LockSource::Logind);
    async move {
        let report = |locked| ready(&owner).set_lock(LockSource::Logind, locked);
        let followed = async { read(&bus.await?, &report).await }.await;
        if let Err(e) = followed {
            tracing::info!(error = %e, "microphone ready: logind unreachable, its session lock is not followed");
        }
        report(None);
    }
}

async fn read(conn: &Connection, report: &impl Fn(Option<bool>)) -> zbus::Result<()> {
    let user = Proxy::new(
        conn,
        LOGIND,
        "/org/freedesktop/login1/user/self",
        "org.freedesktop.login1.User",
    )
    .await?;
    let (_, path): (String, OwnedObjectPath) = user.get_property("Display").await?;
    let session = Proxy::new(conn, LOGIND, path, "org.freedesktop.login1.Session").await?;
    let mut locked_changes = session.receive_property_changed::<bool>("LockedHint").await;
    let mut active_changes = session.receive_property_changed::<bool>("Active").await;
    let mut locked: bool = session.get_property("LockedHint").await?;
    let mut active: bool = session.get_property("Active").await?;
    loop {
        report(Some(locked || !active));
        tokio::select! {
            Some(change) = locked_changes.next() => locked = change.get().await?,
            Some(change) = active_changes.next() => active = change.get().await?,
            else => return Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "ready_session_tests.rs"]
mod tests;
