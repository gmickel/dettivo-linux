//! Structured logs on standard error, which systemd forwards to the
//! journal. `RUST_LOG` overrides the configured level for the run; without
//! it, `[daemon] log_level` applies at start and again on every reload.
//!
//! Log discipline (R7): no log site anywhere in this crate formats a
//! request line, request parameters, a client-chosen id or method string,
//! a configuration value, transcript text, audio content or a prompt. Log
//! sites carry method names from the static catalog, counts, byte lengths,
//! error codes and file paths the daemon itself resolved.

use std::sync::OnceLock;

use dettivo_core::config::LogLevel;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::reload;
use tracing_subscriber::util::SubscriberInitExt;

type FilterHandle = reload::Handle<EnvFilter, tracing_subscriber::Registry>;

/// The handle a reload swaps the filter through; `None` while `RUST_LOG`
/// owns the level for the run.
static FILTER: OnceLock<Option<FilterHandle>> = OnceLock::new();

/// Installs the global subscriber once; later calls are no-ops.
pub fn init(level: LogLevel) {
    let from_env = EnvFilter::try_from_default_env().ok();
    let configured = from_env.is_none();
    let filter = from_env.unwrap_or_else(|| EnvFilter::new(level.as_directive()));
    let (filter, handle) = reload::Layer::new(filter);
    let installed = tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(false)
                .with_target(false),
        )
        .try_init()
        .is_ok();
    let _ = FILTER.set((installed && configured).then_some(handle));
}

/// Applies a reloaded `[daemon] log_level`; a run under `RUST_LOG` keeps
/// its filter. Returns whether the level changed the filter.
pub fn set_level(level: LogLevel) -> bool {
    let Some(Some(handle)) = FILTER.get() else {
        return false;
    };
    handle.reload(EnvFilter::new(level.as_directive())).is_ok()
}
