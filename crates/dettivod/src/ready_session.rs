//! The session lock the always-ready microphone follows (ADR 0092): the
//! user's graphical session from logind (`User.Display` of
//! `/org/freedesktop/login1/user/self`), whose `LockedHint` and `Active`
//! properties close the ring while the screen is locked or another
//! session is in front. Without logind the ring follows the compositor's
//! lock (`ready_screen.rs`) alone.

use std::future::Future;
use std::sync::Arc;

use futures_util::StreamExt;
use zbus::proxy::PropertyStream;
use zbus::zvariant::OwnedObjectPath;
use zbus::{Connection, Proxy};

use crate::daemon::Daemon;
use crate::ready::{LockSource, MicReady};

const LOGIND: &str = "org.freedesktop.login1";

/// `User.Display`: the session id and path, `("", "/")` for none.
type Display = (String, OwnedObjectPath);

/// Watches the session on the system bus until the daemon stops.
pub fn watch(daemon: Arc<Daemon>) -> impl Future<Output = ()> + Send + 'static {
    follow(daemon, |d| d.dictation().ready(), Connection::system())
}

/// Follows logind on `bus` into the ring `ready` reaches from `owner`.
/// The ring stays closed from this call until a session `Display` names
/// reports an unlock; an unreachable logind leaves the lock to the
/// compositor.
fn follow<O: Send + Sync + 'static>(
    owner: Arc<O>,
    ready: fn(&O) -> &MicReady,
    bus: impl Future<Output = zbus::Result<Connection>> + Send + 'static,
) -> impl Future<Output = ()> + Send + 'static {
    ready(&owner).lock_pending(LockSource::Logind);
    async move {
        let mic = ready(&owner);
        let followed = async { read(&bus.await?, mic).await }.await;
        if let Err(e) = followed {
            tracing::info!(error = %e, "microphone ready: logind unreachable, its session lock is not followed");
        }
        mic.set_lock(LockSource::Logind, None);
    }
}

/// Follows the session `User.Display` names, moving with it. While it
/// names none (before the login, after the logout) or its session cannot
/// be read, the ring stays closed until `Display` names another.
async fn read(conn: &Connection, ready: &MicReady) -> zbus::Result<()> {
    let user = Proxy::new(
        conn,
        LOGIND,
        "/org/freedesktop/login1/user/self",
        "org.freedesktop.login1.User",
    )
    .await?;
    let mut displays = user.receive_property_changed::<Display>("Display").await;
    let (_, mut path): Display = user.get_property("Display").await?;
    loop {
        if path.as_str() != "/" {
            match session(conn, &path, ready, &mut displays).await {
                Ok(Some(next)) => {
                    path = next;
                    continue;
                }
                Ok(None) => return Ok(()),
                Err(e) => {
                    tracing::info!(error = %e, %path, "microphone ready: logind session unreadable, the ring stays closed");
                }
            }
        }
        ready.lock_pending(LockSource::Logind);
        match moved(&mut displays, &path).await? {
            Some(next) => path = next,
            None => return Ok(()),
        }
    }
}

/// Reports the session at `path` until `Display` names another (or none,
/// `/`), which it returns; `None` once logind's changes end, keeping the
/// last report.
async fn session(
    conn: &Connection,
    path: &OwnedObjectPath,
    ready: &MicReady,
    displays: &mut PropertyStream<'_, Display>,
) -> zbus::Result<Option<OwnedObjectPath>> {
    let session = Proxy::new(conn, LOGIND, path, "org.freedesktop.login1.Session").await?;
    let mut locked_changes = session.receive_property_changed::<bool>("LockedHint").await;
    let mut active_changes = session.receive_property_changed::<bool>("Active").await;
    let mut locked: bool = session.get_property("LockedHint").await?;
    let mut active: bool = session.get_property("Active").await?;
    loop {
        ready.set_lock(LockSource::Logind, Some(locked || !active));
        tokio::select! {
            Some(change) = locked_changes.next() => locked = change.get().await?,
            Some(change) = active_changes.next() => active = change.get().await?,
            Some(change) = displays.next() => {
                let (_, next) = change.get().await?;
                if next != *path {
                    return Ok(Some(next));
                }
            }
            else => return Ok(None),
        }
    }
}

/// The next session `Display` names other than `from`; `None` once
/// logind's changes end.
async fn moved(
    displays: &mut PropertyStream<'_, Display>,
    from: &OwnedObjectPath,
) -> zbus::Result<Option<OwnedObjectPath>> {
    while let Some(change) = displays.next().await {
        let (_, path) = change.get().await?;
        if path != *from {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "ready_session_tests.rs"]
mod tests;
