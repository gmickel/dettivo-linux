//! Test doubles on a private session bus: a `dbus-daemon` the test owns,
//! a `org.freedesktop.portal.GlobalShortcuts` implementation that answers
//! `CreateSession` and `BindShortcuts` and fires `Activated` and
//! `Deactivated` on demand (or denies every bind), and an MPRIS player
//! whose `PlaybackStatus` the test reads back.

mod player;
mod portal;

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use zbus::Connection;

pub use player::MockPlayer;
pub use portal::MockPortal;

/// A session bus of our own; killed on drop. Its configuration names no
/// service directory, so nothing on the bus can activate the desktop's
/// real portal or players: only what the test serves exists there.
pub struct PrivateBus {
    address: String,
    child: Child,
    config: std::path::PathBuf,
}

const BUS_CONFIG: &str = r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir=/tmp</listen>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#;

impl PrivateBus {
    /// Starts a `dbus-daemon` and reads its address. `Err` names what is
    /// missing when it cannot start.
    pub fn start() -> Result<Self, String> {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let config =
            std::env::temp_dir().join(format!("dettivo-bus-{}-{n}.conf", std::process::id()));
        std::fs::write(&config, BUS_CONFIG).map_err(|e| format!("{}: {e}", config.display()))?;
        let mut child = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--print-address", "--nofork", "--nopidfile"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("dbus-daemon is not available (pacman -S dbus): {e}"))?;
        let stdout = child.stdout.take().ok_or("dbus-daemon: no stdout")?;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let _ = BufReader::new(stdout).read_line(&mut line);
            let _ = tx.send(line);
        });
        let address = rx
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| "dbus-daemon printed no address".to_string())?
            .trim()
            .to_string();
        if address.is_empty() {
            let _ = child.kill();
            let _ = std::fs::remove_file(&config);
            return Err("dbus-daemon printed no address".into());
        }
        Ok(Self {
            address,
            child,
            config,
        })
    }

    /// The `DBUS_SESSION_BUS_ADDRESS` value.
    pub fn address(&self) -> &str {
        &self.address
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.config);
    }
}

/// A service on its own thread and runtime, stopped on drop.
pub(crate) struct Served {
    pub(crate) connection: Connection,
    runtime: tokio::runtime::Handle,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Served {
    pub(crate) fn start(
        build: impl FnOnce(
            zbus::connection::Builder<'static>,
        ) -> zbus::Result<zbus::connection::Builder<'static>>
        + Send
        + 'static,
        address: String,
    ) -> Result<Self, String> {
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let thread = std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("runtime: {e}")));
                    return;
                }
            };
            runtime.block_on(async move {
                let built = async {
                    let builder = zbus::connection::Builder::address(address.as_str())
                        .map_err(|e| e.to_string())?;
                    build(builder)
                        .map_err(|e| e.to_string())?
                        .build()
                        .await
                        .map_err(|e| e.to_string())
                }
                .await;
                match built {
                    Ok(conn) => {
                        let _ = ready_tx.send(Ok((conn, tokio::runtime::Handle::current())));
                        let _ = shutdown_rx.await;
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                    }
                }
            });
        });
        let (connection, runtime) = ready_rx
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| "service did not start".to_string())??;
        Ok(Self {
            connection,
            runtime,
            shutdown: Some(shutdown_tx),
            thread: Some(thread),
        })
    }

    /// Runs `job` on the service's runtime and waits for it.
    pub(crate) fn run<T: Send + 'static>(
        &self,
        job: impl Future<Output = T> + Send + 'static,
    ) -> Result<T, String> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.runtime.spawn(async move {
            let _ = tx.send(job.await);
        });
        rx.recv_timeout(Duration::from_secs(10))
            .map_err(|_| "service job timed out".to_string())
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
