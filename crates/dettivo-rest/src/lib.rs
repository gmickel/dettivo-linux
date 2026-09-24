//! The loopback REST shim (ADR 0028): a hand-rolled HTTP/1.1 server over
//! `tokio` and `httparse` that maps `/v1/{namespace}/{method}` onto the
//! daemon's contract methods, streams raw audio into `transcripts.import`
//! and export bytes out of `transcripts.export`, requires the shared
//! token on every request, and answers errors with the contract's error
//! object under the macOS status map (`docs/api/dettivo-rest-v1.md`).
//!
//! The daemon hosts it in-process when `[rest] enabled = true`
//! (`crates/dettivod/src/rest.rs`); `dettivo rest serve` hosts it as a
//! process over the Unix socket; `dettivo-qa rest` replays the fixtures
//! under `fixtures/` through [`harness`].

pub mod auth;
pub mod backend;
pub mod client;
pub mod harness;
pub mod http;
pub mod routes;
pub mod server;
pub mod status;
pub mod stream;

pub use backend::{Backend, SocketBackend};
pub use server::{Server, Shim, StartError};

/// Package name of this crate, as declared in its manifest.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// The port the contract names (`api.rest.port`).
pub const DEFAULT_PORT: u16 = 45_831;

/// The listener settings, the `[rest]` section without `enabled`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The TCP port; `0` takes an ephemeral one.
    pub port: u16,
    /// The bind address; loopback only.
    pub bind: String,
    /// The largest request body accepted.
    pub max_body_bytes: u64,
    /// The time one request may take end to end.
    pub request_timeout_ms: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self::from_config(&dettivo_core::config::schema::Rest::default())
    }
}

impl Settings {
    /// The settings a `[rest]` section names.
    pub fn from_config(rest: &dettivo_core::config::schema::Rest) -> Self {
        Self {
            port: rest.port,
            bind: rest.bind.clone(),
            max_body_bytes: rest.max_body_bytes,
            request_timeout_ms: rest.request_timeout_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_and_defaults_match_the_contract() {
        assert_eq!(super::CRATE_NAME, "dettivo-rest");
        let s = super::Settings::default();
        assert_eq!(s.port, super::DEFAULT_PORT);
        assert_eq!(s.bind, "127.0.0.1");
        assert_eq!(s.max_body_bytes, 50 * 1024 * 1024);
    }
}
