//! The MCP server behind `dettivo mcp serve` (ADR 0019): a hand-rolled
//! JSON-RPC 2.0 layer over stdio that mirrors the macOS server tool for
//! tool. Every tool is one translation to a daemon method through the
//! Unix socket, every result is bounded before it leaves, and the framing
//! (line-delimited or `Content-Length`) is whatever the client spoke first.
//!
//! The library is shared by the `dettivo-mcp` binary, the `dettivo mcp`
//! verbs in the CLI, and the QA runner, which drives a spawned server
//! through [`harness`] in both framings.

pub mod bounds;
pub mod client;
pub mod harness;
pub mod harness_steps;
pub mod hosts;
pub mod messages;
pub mod protocol;
pub mod resources;
pub mod tools;
pub mod transport;

/// Package name of this crate, as declared in its manifest.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// The name the server reports in `initialize`.
pub const SERVER_NAME: &str = "dettivo-mcp";

/// The version the server reports in `initialize`.
pub const SERVER_VERSION: &str = "1.0.0";

/// The protocol version answered when a client names none.
pub const DEFAULT_PROTOCOL_VERSION: &str = "2024-11-05";

/// Runs the server over standard input and output until the client
/// closes its end. Standard output carries protocol only; log lines go
/// to standard error when `DETTIVO_MCP_DEBUG=1`.
pub fn serve(config: protocol::Config) -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut server = protocol::Server::new(config);
    server.run(stdin.lock(), stdout.lock())
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "dettivo-mcp");
    }
}
