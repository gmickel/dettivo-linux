//! The daemon's connections to the Wayland compositor. Every one is made
//! to the session's display socket by path, so a compositor that
//! restarted on the same display is reached again; the long-lived
//! watchers (the screen lock, input idle) reconnect on their own when
//! the compositor goes away and comes back (ADR 0100).

use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use wayland_client::Connection;

/// The first wait after a lost or refused connection.
pub const RETRY_FIRST: Duration = Duration::from_millis(100);
/// The longest wait: a compositor that comes back is reached this long
/// after at most.
pub const RETRY_MAX: Duration = Duration::from_secs(2);
/// A connection that served this long starts the waits over.
const STEADY: Duration = Duration::from_secs(10);

/// Connects to the compositor on `socket`.
pub fn connect(socket: &Path) -> Result<Connection, String> {
    let stream =
        UnixStream::connect(socket).map_err(|e| format!("wayland {}: {e}", socket.display()))?;
    Connection::from_socket(stream).map_err(|e| format!("wayland: {e}"))
}

/// How following one connection ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Ended {
    /// The connection failed or the compositor went away; names why.
    Lost(String),
    /// Nothing to follow on this compositor; stop.
    Done,
}

/// Follows the compositor on `socket` for as long as `watch` wants:
/// `watch` gets each new connection and returns how it ended. A refused
/// or lost connection is tried again, the wait doubling from
/// [`RETRY_FIRST`] to [`RETRY_MAX`] and starting over after a connection
/// that served; `lost` hears why each one ended. [`Ended::Done`] returns.
pub fn reconnecting(
    socket: &Path,
    mut watch: impl FnMut(Connection) -> Ended,
    mut lost: impl FnMut(&str),
) {
    let mut wait = RETRY_FIRST;
    loop {
        let started = Instant::now();
        let why = match connect(socket) {
            Ok(conn) => match watch(conn) {
                Ended::Done => return,
                Ended::Lost(why) => why,
            },
            Err(why) => why,
        };
        lost(&why);
        if started.elapsed() >= STEADY {
            wait = RETRY_FIRST;
        }
        std::thread::sleep(wait);
        wait = (wait * 2).min(RETRY_MAX);
    }
}
