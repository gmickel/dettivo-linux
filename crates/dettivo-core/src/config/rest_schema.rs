//! The `[rest]` section: the loopback REST shim (ADR 0028).

use serde::{Deserialize, Serialize};

/// `[rest]`: the loopback HTTP shim over the contract (ADR 0028). The
/// daemon hosts it when `enabled`; `dettivo rest serve` hosts it as a
/// process either way.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rest {
    /// The daemon starts the listener at start.
    pub enabled: bool,
    /// The TCP port; `0` takes an ephemeral one (tests), reported in
    /// `system.capabilities.rest.port`.
    pub port: u16,
    /// The address to bind: a loopback address only; anything else is
    /// refused at validation naming this key.
    pub bind: String,
    /// The largest request body accepted, in bytes; a longer one is
    /// answered `413` before the rest is read.
    pub max_body_bytes: u64,
    /// The time one request may take end to end.
    pub request_timeout_ms: u64,
}

impl Default for Rest {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 45_831,
            bind: "127.0.0.1".into(),
            max_body_bytes: 52_428_800,
            request_timeout_ms: 30_000,
        }
    }
}

impl Rest {
    /// The finding for a `bind` outside loopback, or `None` when the
    /// address is `127.0.0.1`, another `127.0.0.0/8` address, `::1` or
    /// `localhost`.
    pub fn bind_error(&self) -> Option<String> {
        let bind = self
            .bind
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']');
        let loopback = match bind.parse::<std::net::IpAddr>() {
            Ok(addr) => addr.is_loopback(),
            Err(_) => bind.eq_ignore_ascii_case("localhost"),
        };
        if loopback {
            None
        } else {
            Some(format!(
                "rest.bind must be a loopback address (127.0.0.1, ::1 or localhost); {:?} is not",
                self.bind
            ))
        }
    }
}
