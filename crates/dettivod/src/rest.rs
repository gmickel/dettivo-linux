//! The REST shim hosted inside the daemon (ADR 0028): when `[rest]
//! enabled = true` the listener starts beside the socket with the shared
//! token resolved in the macOS order, and every route calls the router
//! in-process. No token means no listener, logged with the three sources
//! named; `system.capabilities.rest` reports whether it is up and on
//! which port (the ephemeral one when `port = 0`).

use std::sync::{Arc, RwLock};

use dettivo_core::config::Loaded;
use dettivo_proto::capabilities::RestCaps;
use dettivo_proto::error::JsonRpcError;
use dettivo_rest::{Backend, Settings, Shim};
use serde_json::Value;

use crate::daemon::Daemon;
use crate::handlers::Ctx;
use crate::router;

/// Whether the daemon's listener is up, and on which port.
#[derive(Default)]
pub struct State {
    port: RwLock<Option<u16>>,
}

impl State {
    /// The `rest` capabilities block: `enabled` is the listener's state
    /// right now, `port` the bound port or the configured one.
    pub fn caps(&self, loaded: &Loaded) -> RestCaps {
        let bound = *self.port.read().unwrap_or_else(|p| p.into_inner());
        RestCaps {
            enabled: bound.is_some(),
            port: bound.unwrap_or(loaded.config.rest.port),
            bind: loaded.config.rest.bind.clone(),
        }
    }

    fn set(&self, port: Option<u16>) {
        *self.port.write().unwrap_or_else(|p| p.into_inner()) = port;
    }
}

/// The router as a backend: one call, no connection, no events (the shim
/// answers `events.*` itself before reaching here).
struct InProcess {
    daemon: Arc<Daemon>,
    ctx: Ctx,
}

impl Backend for InProcess {
    fn call(&self, method: &str, params: Value) -> Result<Value, JsonRpcError> {
        crate::daemon::blocking(|| router::call(&self.daemon, &self.ctx, method, &params))
    }
}

/// Starts the listener when `[rest] enabled` is set. Returns the server
/// to stop at shutdown, or `None` when it is off or could not start (the
/// reason is logged; the daemon runs on).
pub async fn start(daemon: &Arc<Daemon>) -> Option<dettivo_rest::Server> {
    let loaded = daemon.config();
    if !loaded.config.rest.enabled {
        return None;
    }
    let Some((token, origin)) = dettivo_core::token::resolve(&loaded.token_file(&daemon.paths))
    else {
        tracing::error!(
            "[rest] enabled but no token found; set one of {}",
            dettivo_core::token::SOURCES
        );
        return None;
    };
    let (notify, _sink) = tokio::sync::mpsc::channel::<String>(1);
    let shim = Shim {
        backend: Arc::new(InProcess {
            daemon: daemon.clone(),
            // An HTTP peer carries no `SO_PEERCRED`, so the self-target
            // allowance (ADR 0024) stays closed to REST callers.
            ctx: Ctx {
                conn: 0,
                notify,
                peer_pid: None,
            },
        }),
        token,
        settings: Settings::from_config(&loaded.config.rest),
    };
    match dettivo_rest::server::start(Arc::new(shim)).await {
        Ok(server) => {
            daemon.rest().set(Some(server.port()));
            tracing::info!(
                port = server.port(),
                bind = %loaded.config.rest.bind,
                token_source = origin.as_str(),
                "REST shim hosted in-process"
            );
            Some(server)
        }
        Err(e) => {
            tracing::error!(error = %e, "REST shim not started");
            None
        }
    }
}

/// Stops the listener and clears the capability.
pub async fn stop(daemon: &Daemon, server: Option<dettivo_rest::Server>) {
    if let Some(server) = server {
        server.stop().await;
        daemon.rest().set(None);
    }
}
