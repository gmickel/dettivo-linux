//! Acquiring the socket: a systemd-passed listener when the unit activated
//! us, otherwise a bind of our own with the runtime directory at `0700`
//! and the socket at `0600`. Exactly one daemon per session: a live daemon
//! on the path makes this start fail with its identity, a stale socket
//! file from a crash is unlinked (R4).

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener as StdListener, UnixStream as StdStream};
use std::path::{Path, PathBuf};

use tokio::net::UnixListener;

use crate::daemon::Daemon;

/// The listener plus what this process owns and must clean up.
pub struct Acquired {
    /// The bound, non-blocking listener.
    pub listener: UnixListener,
    /// The socket path to unlink at shutdown (`None` when systemd owns it).
    pub owned_socket: Option<PathBuf>,
    /// The pid file to remove at shutdown.
    pub pid_file: PathBuf,
    /// Where the listener came from, for the start log line.
    pub origin: Origin,
}

/// How the listener was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Passed in by systemd socket activation (`LISTEN_FDS`).
    SystemdActivation,
    /// Bound by this process.
    Bound,
}

/// Why the socket could not be acquired; the message names the running
/// instance when there is one.
#[derive(Debug)]
pub enum AcquireError {
    /// Another daemon answers on the socket.
    AlreadyRunning {
        /// The socket that is in use.
        socket: PathBuf,
        /// The pid the running instance recorded, when readable.
        pid: Option<u32>,
    },
    /// The file system refused.
    Io {
        /// What was being done.
        what: String,
        /// The error.
        error: io::Error,
    },
}

impl std::fmt::Display for AcquireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRunning { socket, pid } => match pid {
                Some(pid) => write!(
                    f,
                    "dettivod is already running (pid {pid}) on {}",
                    socket.display()
                ),
                None => write!(f, "dettivod is already running on {}", socket.display()),
            },
            Self::Io { what, error } => write!(f, "{what}: {error}"),
        }
    }
}

impl std::error::Error for AcquireError {}

/// Acquires the listener for `daemon`'s socket path.
pub async fn acquire(daemon: &Daemon) -> Result<Acquired, AcquireError> {
    let socket = daemon.socket_path();
    let pid_file = daemon.pid_file();
    if let Some(listener) = from_systemd()? {
        write_pid(&pid_file)?;
        return Ok(Acquired {
            listener,
            owned_socket: None,
            pid_file,
            origin: Origin::SystemdActivation,
        });
    }
    let listener = bind(&socket, &pid_file)?;
    write_pid(&pid_file)?;
    Ok(Acquired {
        listener,
        owned_socket: Some(socket),
        pid_file,
        origin: Origin::Bound,
    })
}

/// The listener systemd passed as fd 3 when `LISTEN_PID` names this
/// process, per `sd_listen_fds(3)`.
fn from_systemd() -> Result<Option<UnixListener>, AcquireError> {
    let pid_matches = std::env::var("LISTEN_PID")
        .ok()
        .and_then(|p| p.parse::<u32>().ok())
        .is_some_and(|p| p == std::process::id());
    let fds = std::env::var("LISTEN_FDS")
        .ok()
        .and_then(|n| n.parse::<u32>().ok())
        .unwrap_or(0);
    if !pid_matches || fds == 0 {
        return Ok(None);
    }
    if fds != 1 {
        return Err(AcquireError::Io {
            what: format!("LISTEN_FDS={fds}: the socket unit passes exactly one listening socket"),
            error: io::Error::new(io::ErrorKind::InvalidInput, "unexpected activation shape"),
        });
    }
    // SAFETY: fd 3 is the one listening socket systemd hands to the process
    // it activated (LISTEN_PID matched our pid and LISTEN_FDS is 1 above),
    // nothing else in this process has opened or will close that
    // descriptor, and ownership transfers to the returned listener exactly
    // once. FD_CLOEXEC is set so a child the daemon spawns (secret-tool in
    // peer_token mode) never inherits the listener.
    #[allow(unsafe_code)]
    let std_listener = unsafe {
        use std::os::unix::io::FromRawFd;
        let flags = libc::fcntl(3, libc::F_GETFD);
        if flags >= 0 {
            libc::fcntl(3, libc::F_SETFD, flags | libc::FD_CLOEXEC);
        }
        StdListener::from_raw_fd(3)
    };
    // LISTEN_* stay in the environment: the runtime's worker threads are
    // already up when this runs, and mutating the environment then is unsound.
    to_tokio(std_listener, "adopt the activated socket").map(Some)
}

/// Binds `socket`, creating its directory `0700`, refusing when a daemon
/// already answers there and unlinking a stale file.
fn bind(socket: &Path, pid_file: &Path) -> Result<UnixListener, AcquireError> {
    let dir = dettivo_core::paths::Paths::socket_dir(socket);
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|error| AcquireError::Io {
            what: format!("create {}", dir.display()),
            error,
        })?;
    }
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|error| {
        AcquireError::Io {
            what: format!("chmod 0700 {}", dir.display()),
            error,
        }
    })?;
    if socket.exists() {
        match StdStream::connect(socket) {
            Ok(_) => {
                // The running instance writes its pid file right after its
                // bind; a start racing that instant waits briefly for it.
                return Err(AcquireError::AlreadyRunning {
                    socket: socket.to_path_buf(),
                    pid: read_pid_soon(pid_file),
                });
            }
            Err(_) => {
                tracing::info!(path = %socket.display(), "removing stale socket file");
                fs::remove_file(socket).map_err(|error| AcquireError::Io {
                    what: format!("remove stale {}", socket.display()),
                    error,
                })?;
            }
        }
    }
    let std_listener = StdListener::bind(socket).map_err(|error| AcquireError::Io {
        what: format!("bind {}", socket.display()),
        error,
    })?;
    fs::set_permissions(socket, fs::Permissions::from_mode(0o600)).map_err(|error| {
        AcquireError::Io {
            what: format!("chmod 0600 {}", socket.display()),
            error,
        }
    })?;
    to_tokio(std_listener, "register the socket")
}

fn to_tokio(std_listener: StdListener, what: &str) -> Result<UnixListener, AcquireError> {
    std_listener
        .set_nonblocking(true)
        .and_then(|()| UnixListener::from_std(std_listener))
        .map_err(|error| AcquireError::Io {
            what: what.to_string(),
            error,
        })
}

fn write_pid(pid_file: &Path) -> Result<(), AcquireError> {
    dettivo_core::atomic::write(
        pid_file,
        format!("{}\n", std::process::id()).as_bytes(),
        0o600,
        0o700,
    )
    .map_err(|error| AcquireError::Io {
        what: format!("write {}", pid_file.display()),
        error,
    })
}

/// `read_pid`, retried for up to half a second while the file is absent.
fn read_pid_soon(pid_file: &Path) -> Option<u32> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    loop {
        if let Some(pid) = read_pid(pid_file) {
            return Some(pid);
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// The pid a running instance recorded, when the file is readable.
pub fn read_pid(pid_file: &Path) -> Option<u32> {
    fs::read_to_string(pid_file)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

/// Removes what this process owns at shutdown.
pub fn release(owned_socket: Option<&Path>, pid_file: &Path) {
    if let Some(socket) = owned_socket {
        let _ = fs::remove_file(socket);
    }
    if read_pid(pid_file) == Some(std::process::id()) {
        let _ = fs::remove_file(pid_file);
    }
}
