//! A stand-in Hyprland instance: `$XDG_RUNTIME_DIR/hypr/<signature>/`
//! with the `hyprland.lock` Hyprland writes (its pid, then its Wayland
//! display) and an IPC socket answering `j/activewindow` with one window.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use super::{Server, bind};

/// The pid the stand-in's focused window reports.
pub const WINDOW_PID: u32 = 4242;

/// One instance.
pub struct HyprlandStandIn {
    socket: PathBuf,
    queries: Arc<AtomicUsize>,
    server: Server,
}

impl HyprlandStandIn {
    /// Starts instance `signature` under `runtime` for Wayland display
    /// `display`, its focused window a `foot` terminal.
    pub fn start(runtime: &Path, signature: &str, display: &str) -> std::io::Result<Self> {
        let dir = runtime.join("hypr").join(signature);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join("hyprland.lock"),
            format!("{}\n{display}\n", std::process::id()),
        )?;
        let socket = dir.join(".socket.sock");
        let queries = Arc::new(AtomicUsize::new(0));
        let counted = queries.clone();
        let server = Server::start(bind(&socket)?, move |mut stream| {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let mut request = [0u8; 256];
            let n = stream.read(&mut request).unwrap_or(0);
            if &request[..n] == b"j/activewindow" {
                counted.fetch_add(1, Ordering::Relaxed);
                let window = format!(
                    r#"{{"address":"0x5","class":"foot","initialClass":"foot","title":"stand-in","pid":{WINDOW_PID},"xwayland":false}}"#
                );
                let _ = stream.write_all(window.as_bytes());
            } else if n > 0 {
                let _ = stream.write_all(b"unknown request");
            }
            // The answer ends at end of file, as Hyprland's does.
            let _ = stream.shutdown(std::net::Shutdown::Both);
        })?;
        Ok(Self {
            socket,
            queries,
            server,
        })
    }

    /// The IPC socket.
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// How many `j/activewindow` queries it answered.
    pub fn queries(&self) -> usize {
        self.queries.load(Ordering::Relaxed)
    }

    /// Dies like a crashed compositor: the socket file stays and refuses
    /// connections.
    pub fn crash(&mut self) {
        self.server.crash();
    }
}
