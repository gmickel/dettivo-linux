//! Stand-ins for the compositor on private sockets, for this crate's
//! tests and the daemon's: a Wayland compositor that speaks just enough of
//! the wire protocol for the virtual keyboard and the input-idle watcher,
//! and a Hyprland instance answering `j/activewindow` on its IPC socket.
//! Each can crash the way a compositor does (its socket file stays and
//! refuses connections) and be started again, so a restart can be
//! replayed without the live desktop.

mod hyprland;
mod wayland;

use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use hyprland::HyprlandStandIn;
pub use wayland::{Global, WaylandStandIn};

/// An accept loop on its own thread that can be stopped like a crash:
/// the listener closes, every client is cut off, the socket file stays.
struct Server {
    stop: Arc<AtomicBool>,
    clients: Arc<Mutex<Vec<UnixStream>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    /// Serves each connection on `listener` with `serve` on a thread of
    /// its own.
    fn start(
        listener: UnixListener,
        serve: impl Fn(UnixStream) + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let clients = Arc::new(Mutex::new(Vec::new()));
        let serve = Arc::new(serve);
        let (s, c) = (stop.clone(), clients.clone());
        let thread = std::thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        if let Ok(copy) = stream.try_clone() {
                            c.lock().unwrap_or_else(|p| p.into_inner()).push(copy);
                        }
                        let serve = serve.clone();
                        std::thread::spawn(move || serve(stream));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Ok(Self {
            stop,
            clients,
            thread: Some(thread),
        })
    }

    /// Stops accepting and cuts every client off; the socket file stays.
    fn crash(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        for client in self
            .clients
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .drain(..)
        {
            let _ = client.shutdown(std::net::Shutdown::Both);
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.crash();
    }
}

/// Binds `path`, removing a socket file a crashed predecessor left.
fn bind(path: &std::path::Path) -> std::io::Result<UnixListener> {
    let _ = std::fs::remove_file(path);
    UnixListener::bind(path)
}
