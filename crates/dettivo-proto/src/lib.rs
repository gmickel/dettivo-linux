//! Contract types shared by the daemon and every client.
//!
//! One definition of the JSON-RPC 2.0 envelope, every method's params and
//! result, the error taxonomy, event notifications and topics, capability
//! flags and the shared runtime types, so the daemon, the CLI, the MCP
//! server and the QA runner agree on the wire by construction. The macOS
//! IPC v1 contract is normative (ADR 0008); the copied documents live under
//! `docs/api/` with their commit pin, and every Linux difference is
//! registered in `docs/api/linux-deltas.md`.
//!
//! The types are plain data with serde derives. Every struct rejects
//! unknown fields, ids are lowercase UUID strings, and error codes are
//! checked against the taxonomy, so a fixture or a live message that drifts
//! from the contract fails to deserialise with the offending field named.
//! [`catalog`] lists every method and runs any params or result through its
//! typed shape by name; the golden fixture suite under `fixtures/` proves
//! each of them round-trips byte-stable.
//!
//! Shared client transport and streaming helpers keep connection deadlines,
//! response bounds and transfer cleanup consistent across adapters (ADR 0054).
//! The daemon owns the socket server, routing and authentication decisions.

pub mod capabilities;
mod capabilities_speech;
pub mod catalog;
pub mod envelope;
pub mod error;
pub mod events;
pub mod id;
pub mod methods;
pub mod reserved;
pub mod runtime;
pub mod tier;
pub mod transfer_io;
pub mod transport;
pub mod upload;

/// Package name of this crate, as declared in its manifest.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// The contract version this crate implements (`system.version.api_version`).
pub const API_VERSION: &str = "1.0.0";

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "dettivo-proto");
    }
}
