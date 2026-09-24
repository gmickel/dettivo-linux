//! The accept loop and per-connection line handling. Lines are read with
//! a hard cap: a longer line is answered `INVALID_PARAMS`, the rest of it
//! is discarded up to the newline, and the connection stays open (R3).
//! SIGTERM or SIGINT stops accepting, drains connections for at most
//! `daemon.shutdown_timeout_ms`, and releases the socket.

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::watch;
use tokio::task::JoinSet;

use crate::auth::{self, Peer};
use crate::daemon::Daemon;
use crate::listener::{self, Acquired};
use crate::router::{self, Outcome, Session};
use crate::watcher;

/// Serves until a termination signal, then drains the connections within
/// `[daemon] shutdown_timeout_ms` and releases the socket. Returns the
/// deadline that budget set, so the caller's remaining steps share it.
pub async fn run(daemon: Arc<Daemon>, acquired: Acquired) -> std::time::Instant {
    let Acquired {
        listener,
        owned_socket,
        pid_file,
        origin,
    } = acquired;
    let (stop_tx, stop_rx) = watch::channel(false);
    tracing::info!(
        socket = %daemon.socket_path().display(),
        origin = ?origin,
        auth = ?daemon.auth_mode(),
        "listening"
    );
    let mut tasks = JoinSet::new();
    tasks.spawn(watcher::run(daemon.clone(), stop_rx.clone()));
    let own_uid = auth::own_uid();
    let mut signals = Signals::install();
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    let daemon = daemon.clone();
                    let stop = stop_rx.clone();
                    tasks.spawn(async move { serve(daemon, stream, own_uid, stop).await });
                }
                Err(e) => {
                    tracing::warn!(error = %e, "accept failed");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            },
            // A finished connection leaves the set at once instead of
            // staying in it until shutdown; the guard keeps an empty set
            // from ending the loop.
            Some(_) = tasks.join_next(), if !tasks.is_empty() => {}
            () = signals.wait() => break,
        }
    }
    tracing::info!("shutting down");
    let _ = stop_tx.send(true);
    drop(listener);
    let budget = Duration::from_millis(daemon.config().config.daemon.shutdown_timeout_ms);
    let deadline = std::time::Instant::now() + budget;
    let drained = tokio::time::timeout_at(deadline.into(), async {
        while tasks.join_next().await.is_some() {}
    })
    .await
    .is_ok();
    if !drained {
        tracing::warn!("shutdown budget exhausted, exiting with connections open");
        tasks.abort_all();
    }
    listener::release(owned_socket.as_deref(), &pid_file);
    tracing::info!("stopped");
    deadline
}

/// One connection: peer check, then a line loop until EOF or stop.
static NEXT_CONN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

async fn serve(
    daemon: Arc<Daemon>,
    stream: UnixStream,
    own_uid: u32,
    mut stop: watch::Receiver<bool>,
) {
    let conn = NEXT_CONN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let (notify_tx, mut notify_rx) = tokio::sync::mpsc::channel::<String>(256);
    let cred = match stream.peer_cred() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "peer credentials unavailable, refusing");
            return;
        }
    };
    // A bounded send buffer per connection: replies are small, and the
    // event stream's overflow rule (256 queued lines, then a drop count)
    // means what it says instead of hiding behind a 200 KB kernel buffer.
    {
        use std::os::fd::AsRawFd;
        let size: libc::c_int = 16 * 1024;
        // SAFETY: a valid socket fd and a properly sized c_int for SO_SNDBUF.
        #[allow(unsafe_code)]
        let rc = unsafe {
            libc::setsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_SNDBUF,
                (&size as *const libc::c_int).cast(),
                std::mem::size_of::<libc::c_int>() as libc::socklen_t,
            )
        };
        if rc != 0 {
            tracing::debug!("SO_SNDBUF not applied");
        }
    }
    let (read_half, mut write_half) = stream.into_split();
    if let Peer::OtherUser { uid } = auth::check_peer(cred.uid(), own_uid) {
        tracing::info!(uid, "connection from another user refused");
        let refusal = router::Outcome::Reply(
            serde_json::to_string(&dettivo_proto::envelope::ErrorResponse::new(
                dettivo_proto::envelope::RequestId::Null,
                auth::unauthorized("Unauthorized client"),
            ))
            .unwrap_or_default(),
        );
        if let Outcome::Reply(line) = refusal {
            let _ = write_half.write_all(line.as_bytes()).await;
            let _ = write_half.write_all(b"\n").await;
        }
        return;
    }
    let max_line = daemon.config().config.ipc.max_line_bytes;
    let session = Session {
        daemon: daemon.clone(),
        ctx: crate::handlers::Ctx {
            conn,
            notify: notify_tx,
            peer_pid: cred.pid().and_then(|p| u32::try_from(p).ok()),
        },
    };
    let mut reader = BufReader::new(read_half);
    let mut line: Vec<u8> = Vec::new();
    let mut discarding = false;
    loop {
        let filled = tokio::select! {
            r = reader.fill_buf() => r,
            Some(line) = notify_rx.recv() => {
                if write_half.write_all(line.as_bytes()).await.is_err()
                    || write_half.write_all(b"\n").await.is_err()
                {
                    break;
                }
                continue;
            }
            _ = stop.changed() => break,
        };
        let buf = match filled {
            Ok([]) => break,
            Ok(b) => b,
            Err(_) => break,
        };
        let (chunk, ended) = match buf.iter().position(|&b| b == b'\n') {
            Some(i) => (&buf[..i], true),
            None => (buf, false),
        };
        let consumed = chunk.len() + usize::from(ended);
        let outcome = if discarding {
            if ended {
                discarding = false;
                Some(router::oversized(max_line))
            } else {
                None
            }
        } else if (line.len() + chunk.len()) as u64 > max_line {
            line.clear();
            if ended {
                Some(router::oversized(max_line))
            } else {
                discarding = true;
                None
            }
        } else {
            line.extend_from_slice(chunk);
            if ended {
                // The handlers block (inference, insertion, the store); the
                // worker steps aside so pings on other connections answer
                // meanwhile.
                let outcome = crate::daemon::blocking(|| router::handle_line(&session, &line));
                line.clear();
                Some(outcome)
            } else {
                None
            }
        };
        reader.consume(consumed);
        if let Some(Outcome::Reply(text)) = outcome {
            if write_half.write_all(text.as_bytes()).await.is_err()
                || write_half.write_all(b"\n").await.is_err()
            {
                break;
            }
        }
    }
    daemon.bus().disconnect(conn);
    daemon.self_target().disarm(conn);
    tracing::debug!(
        conn,
        subscriptions = daemon.bus().count(),
        "connection closed"
    );
}

/// SIGTERM and SIGINT, whichever comes first.
struct Signals {
    term: Option<tokio::signal::unix::Signal>,
    int: Option<tokio::signal::unix::Signal>,
}

impl Signals {
    fn install() -> Self {
        use tokio::signal::unix::{SignalKind, signal};
        Self {
            term: signal(SignalKind::terminate()).ok(),
            int: signal(SignalKind::interrupt()).ok(),
        }
    }

    async fn wait(&mut self) {
        match (&mut self.term, &mut self.int) {
            (Some(term), Some(int)) => {
                tokio::select! {
                    _ = term.recv() => {}
                    _ = int.recv() => {}
                }
            }
            (Some(term), None) => {
                term.recv().await;
            }
            (None, Some(int)) => {
                int.recv().await;
            }
            (None, None) => std::future::pending::<()>().await,
        }
    }
}
