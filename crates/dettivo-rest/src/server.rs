//! The listener: bound to a loopback address only, one task per
//! connection, keep-alive, a per-request timeout, and the route handler
//! run off the runtime's workers (every backend call blocks on the
//! socket). A port already in use names the port and the likely owner;
//! no token means no listener.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::backend::Backend;
use crate::http::{self, ReadError, Response};
use crate::{Settings, routes, status};

/// Everything a request needs: the backend, the token and the limits.
pub struct Shim {
    /// Where calls go.
    pub backend: Arc<dyn Backend>,
    /// The token every request must carry.
    pub token: String,
    /// The listener settings.
    pub settings: Settings,
}

/// Why the listener did not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartError {
    /// `bind` is not a loopback address.
    NotLoopback(String),
    /// No token in any of the three sources.
    NoToken,
    /// The port is taken.
    AddrInUse(SocketAddr),
    /// Another bind failure.
    Io(String),
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotLoopback(bind) => write!(
                f,
                "rest.bind {bind:?} is not a loopback address; the REST shim binds 127.0.0.1 or ::1 only"
            ),
            Self::NoToken => write!(
                f,
                "no REST token: set one of {}",
                dettivo_core::token::SOURCES
            ),
            Self::AddrInUse(addr) => write!(
                f,
                "{addr} is already in use: another `dettivo rest serve` or a daemon with [rest] enabled owns it (`dettivo rest status` shows which); stop it or pick another port"
            ),
            Self::Io(e) => write!(f, "cannot bind the REST listener: {e}"),
        }
    }
}

/// A running listener.
pub struct Server {
    /// The address it is bound to (the port is the ephemeral one when
    /// `0` was asked for).
    pub addr: SocketAddr,
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}

impl Server {
    /// The bound port.
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// Stops accepting and ends every connection task.
    pub async fn stop(self) {
        let _ = self.stop.send(true);
        let _ = self.task.await;
    }
}

/// The loopback address `bind` names.
pub fn loopback_ip(bind: &str) -> Result<IpAddr, StartError> {
    let trimmed = bind.trim().trim_start_matches('[').trim_end_matches(']');
    let ip = if trimmed.eq_ignore_ascii_case("localhost") {
        IpAddr::from([127, 0, 0, 1])
    } else {
        trimmed
            .parse()
            .map_err(|_| StartError::NotLoopback(bind.to_string()))?
    };
    if ip.is_loopback() {
        Ok(ip)
    } else {
        Err(StartError::NotLoopback(bind.to_string()))
    }
}

/// Binds and starts serving on the current runtime.
pub async fn start(shim: Arc<Shim>) -> Result<Server, StartError> {
    if shim.token.is_empty() {
        return Err(StartError::NoToken);
    }
    let ip = loopback_ip(&shim.settings.bind)?;
    let addr = SocketAddr::new(ip, shim.settings.port);
    let listener = TcpListener::bind(addr).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            StartError::AddrInUse(addr)
        } else {
            StartError::Io(e.to_string())
        }
    })?;
    let addr = listener
        .local_addr()
        .map_err(|e| StartError::Io(e.to_string()))?;
    let (stop, stop_rx) = watch::channel(false);
    let task = tokio::spawn(accept_loop(listener, shim, stop_rx));
    tracing::info!(%addr, "REST shim listening");
    Ok(Server { addr, stop, task })
}

/// Connections served at once; the next one is closed unanswered, so an
/// unauthenticated peer cannot hold unbounded request buffers open.
pub const MAX_CONNECTIONS: usize = 64;

async fn accept_loop(listener: TcpListener, shim: Arc<Shim>, mut stop: watch::Receiver<bool>) {
    let mut tasks = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, peer)) => {
                    if !peer.ip().is_loopback() {
                        tracing::info!(%peer, "non-loopback REST peer refused");
                        continue;
                    }
                    if tasks.len() >= MAX_CONNECTIONS {
                        tracing::warn!(open = tasks.len(), "REST connection limit reached; connection closed");
                        drop(stream);
                        continue;
                    }
                    tasks.spawn(serve(stream, shim.clone(), stop.clone()));
                }
                Err(e) => {
                    tracing::warn!(error = %e, "REST accept failed");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            },
            // A finished connection leaves the set as it closes rather
            // than at shutdown; the guard keeps an empty set from ending
            // the loop.
            Some(_) = tasks.join_next(), if !tasks.is_empty() => {}
            _ = stop.changed() => break,
        }
    }
    drop(listener);
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    tracing::info!("REST shim stopped");
}

async fn serve(mut stream: TcpStream, shim: Arc<Shim>, mut stop: watch::Receiver<bool>) {
    let timeout = Duration::from_millis(shim.settings.request_timeout_ms.max(1));
    let mut carry = Vec::new();
    loop {
        let (mut reader, mut writer) = stream.split();
        // One deadline per request, from the first byte read to the last
        // byte written: reading, the handler and the response share it.
        let deadline = tokio::time::Instant::now() + timeout;
        let read = tokio::select! {
            r = tokio::time::timeout_at(deadline, http::read_request(&mut reader, shim.settings.max_body_bytes, &mut carry)) => r,
            _ = stop.changed() => return,
        };
        let request = match read {
            Ok(Ok(r)) => r,
            Ok(Err(ReadError::Closed)) => return,
            Ok(Err(ReadError::BodyTooLarge { declared, cap })) => {
                let _ = http::write_response(&mut writer, &status::too_large(declared, cap), false)
                    .await;
                return;
            }
            Ok(Err(ReadError::HeadTooLarge)) => {
                let response = status::error(&status::invalid(format!(
                    "request head exceeds {} bytes",
                    http::MAX_HEAD_BYTES
                )));
                let _ = http::write_response(&mut writer, &response, false).await;
                return;
            }
            Ok(Err(ReadError::Malformed(m))) => {
                let response = status::error(&status::invalid(format!("malformed request: {m}")));
                let _ = http::write_response(&mut writer, &response, false).await;
                return;
            }
            Ok(Err(ReadError::Io(_))) | Err(_) => return,
        };
        let keep_alive = request.keep_alive;
        let handler = shim.clone();
        let method = format!("{} {}", request.method, request.path);
        let mut work = tokio::task::spawn_blocking(move || routes::handle(&handler, &request));
        let handled = tokio::time::timeout_at(deadline, &mut work).await;
        let response: Response = match handled {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => status::error(&status::internal(format!("handler failed: {e}"))),
            Err(_) => {
                // The blocking work cannot be interrupted; it stays tracked
                // to its end so the log says whether the late outcome was a
                // success the client never saw.
                tokio::spawn(async move {
                    match work.await {
                        Ok(r) => {
                            tracing::warn!(request = %method, status = r.status, "REST request finished after its deadline")
                        }
                        Err(e) => {
                            tracing::warn!(request = %method, error = %e, "REST request failed after its deadline")
                        }
                    }
                });
                status::error(&status::internal(format!(
                    "request exceeded rest.request_timeout_ms ({} ms); its outcome is unknown, check the state before retrying",
                    timeout.as_millis()
                )))
            }
        };
        let written = tokio::time::timeout_at(
            deadline,
            http::write_response(&mut writer, &response, keep_alive),
        )
        .await;
        if !matches!(written, Ok(Ok(()))) || !keep_alive {
            return;
        }
    }
}

/// Runs a listener on a runtime of its own until SIGTERM or SIGINT.
pub fn run_blocking(shim: Shim) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("cannot start the async runtime: {e}"))?;
    runtime.block_on(async move {
        let server = start(Arc::new(shim)).await.map_err(|e| e.to_string())?;
        eprintln!("dettivo rest: listening on http://{}", server.addr);
        wait_for_signal().await;
        server.stop().await;
        Ok(())
    })
}

async fn wait_for_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    match (
        signal(SignalKind::terminate()),
        signal(SignalKind::interrupt()),
    ) {
        (Ok(mut term), Ok(mut int)) => {
            tokio::select! {
                _ = term.recv() => {}
                _ = int.recv() => {}
            }
        }
        _ => std::future::pending::<()>().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_binds_are_accepted() {
        assert_eq!(
            loopback_ip("127.0.0.1").unwrap(),
            IpAddr::from([127, 0, 0, 1])
        );
        assert_eq!(
            loopback_ip("localhost").unwrap(),
            IpAddr::from([127, 0, 0, 1])
        );
        assert!(loopback_ip("::1").unwrap().is_loopback());
        assert!(loopback_ip("[::1]").unwrap().is_loopback());
        assert_eq!(
            loopback_ip("0.0.0.0").unwrap_err(),
            StartError::NotLoopback("0.0.0.0".into())
        );
        assert!(matches!(
            loopback_ip("example.com"),
            Err(StartError::NotLoopback(_))
        ));
        assert!(
            StartError::NoToken
                .to_string()
                .contains("DETTIVO_IPC_TOKEN")
        );
    }
}
