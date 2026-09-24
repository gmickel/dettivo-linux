//! Reloads `config.toml` when it changes on disk, so GUI and CLI edits
//! converge without a restart (R5). A poll on modification time and size
//! every 250 ms needs no inotify handle and survives editors that replace
//! the file, which a watch on the inode would lose.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::sync::watch;

use crate::daemon::Daemon;

/// Poll interval.
pub const INTERVAL: Duration = Duration::from_millis(250);

/// Runs until `stop` flips.
pub async fn run(daemon: Arc<Daemon>, mut stop: watch::Receiver<bool>) {
    let path = daemon.paths.config_file.clone();
    let mut last = stamp(&path);
    loop {
        tokio::select! {
            () = tokio::time::sleep(INTERVAL) => {}
            _ = stop.changed() => return,
        }
        let now = stamp(&path);
        if now != last {
            last = now;
            let loaded = daemon.reload();
            daemon.hotkeys().apply(&daemon, &loaded);
            match loaded.error {
                None => tracing::info!(path = %path.display(), "configuration reloaded"),
                Some(err) => tracing::warn!(
                    path = %path.display(),
                    key = err.key.as_deref().unwrap_or("-"),
                    lineno = err.line.unwrap_or(0),
                    "configuration changed but is invalid, keeping the values in force"
                ),
            }
        }
    }
}

/// Modification time and length; `None` when the file is absent.
fn stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}
