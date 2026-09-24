//! MPRIS pause and resume (FR-H5): every `org.mpris.MediaPlayer2.*` player
//! that is `Playing` when capture starts is paused and remembered; the
//! same players are played again when capture stops or is cancelled. A
//! player the user paused is never resumed, and one that vanished
//! meanwhile is skipped.

use std::time::Duration;

use zbus::Connection;

use crate::portal::on_own_runtime;

/// The bus name prefix every player owns.
pub const PREFIX: &str = "org.mpris.MediaPlayer2.";

/// How long one call may take; a stuck player never stalls a session.
const CALL_TIMEOUT: Duration = Duration::from_secs(3);

async fn player(conn: &Connection, name: &str) -> Result<zbus::Proxy<'static>, String> {
    zbus::Proxy::new(
        conn,
        name.to_string(),
        "/org/mpris/MediaPlayer2",
        "org.mpris.MediaPlayer2.Player",
    )
    .await
    .map_err(|e| format!("{name}: {e}"))
}

/// Every player name on the bus.
pub async fn players(conn: &Connection) -> Result<Vec<String>, String> {
    let dbus = zbus::fdo::DBusProxy::new(conn)
        .await
        .map_err(|e| format!("bus: {e}"))?;
    let names = dbus.list_names().await.map_err(|e| format!("bus: {e}"))?;
    Ok(names
        .into_iter()
        .map(|n| n.to_string())
        .filter(|n| n.starts_with(PREFIX))
        .collect())
}

/// Pauses every playing player and returns their names.
pub async fn pause_playing(conn: &Connection) -> Result<Vec<String>, String> {
    let mut paused = Vec::new();
    for name in players(conn).await? {
        let proxy = match player(conn, &name).await {
            Ok(p) => p,
            Err(e) => {
                tracing::debug!(error = %e, "mpris: player skipped");
                continue;
            }
        };
        let status =
            tokio::time::timeout(CALL_TIMEOUT, proxy.get_property::<String>("PlaybackStatus"))
                .await
                .map_err(|_| format!("{name}: PlaybackStatus timed out"))
                .and_then(|r| r.map_err(|e| format!("{name}: {e}")));
        match status {
            Ok(s) if s == "Playing" => {
                match tokio::time::timeout(CALL_TIMEOUT, proxy.call_method("Pause", &())).await {
                    Ok(Ok(_)) => {
                        tracing::info!(player = %name, "mpris: paused");
                        paused.push(name);
                    }
                    Ok(Err(e)) => tracing::warn!(player = %name, error = %e, "mpris: pause failed"),
                    Err(_) => tracing::warn!(player = %name, "mpris: pause timed out"),
                }
            }
            Ok(_) => {}
            Err(e) => tracing::debug!(error = %e, "mpris: player skipped"),
        }
    }
    Ok(paused)
}

/// Plays the named players again; a player that vanished is skipped and
/// named in the returned list.
pub async fn resume(conn: &Connection, names: &[String]) -> Vec<(String, String)> {
    let mut skipped = Vec::new();
    for name in names {
        let outcome = match player(conn, name).await {
            Ok(proxy) => {
                match tokio::time::timeout(CALL_TIMEOUT, proxy.call_method("Play", &())).await {
                    Ok(Ok(_)) => Ok(()),
                    Ok(Err(e)) => Err(e.to_string()),
                    Err(_) => Err("Play timed out".to_string()),
                }
            }
            Err(e) => Err(e),
        };
        match outcome {
            Ok(()) => tracing::info!(player = %name, "mpris: resumed"),
            Err(reason) => {
                tracing::info!(player = %name, reason = %reason, "mpris: player skipped on resume");
                skipped.push((name.clone(), reason));
            }
        }
    }
    skipped
}

/// `pause_playing` from synchronous code, on the session bus or the bus at
/// `address`.
pub fn pause_playing_blocking(address: Option<&str>) -> Result<Vec<String>, String> {
    on_own_runtime(|| async { pause_playing(&crate::portal::connect(address).await?).await })
}

/// `resume` from synchronous code, on the session bus or the bus at
/// `address`.
pub fn resume_blocking(
    address: Option<&str>,
    names: &[String],
) -> Result<Vec<(String, String)>, String> {
    on_own_runtime(|| async { Ok(resume(&crate::portal::connect(address).await?, names).await) })
}
