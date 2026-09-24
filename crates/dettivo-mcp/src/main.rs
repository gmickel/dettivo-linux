//! `dettivo-mcp`: the MCP server over stdio, the same server `dettivo
//! mcp serve` runs in-process. It takes the CLI's connection flags and
//! nothing else; standard output is protocol only.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use dettivo_mcp::client::Client;
use dettivo_mcp::protocol::Config;

/// The MCP server over stdio.
#[derive(Debug, Parser)]
#[command(
    name = "dettivo-mcp",
    version,
    about = "Serve the Dettivo daemon to MCP clients over stdio"
)]
struct Cli {
    /// Socket path (default: DETTIVO_IPC_SOCKET, then $XDG_RUNTIME_DIR/dettivo/dettivo.sock).
    #[arg(long, value_name = "PATH")]
    socket: Option<PathBuf>,
    /// Shared token for peer_token mode (default: DETTIVO_IPC_TOKEN).
    #[arg(long, value_name = "TOKEN")]
    token: Option<String>,
    /// Read the shared token from this file.
    #[arg(long, value_name = "PATH")]
    token_file: Option<PathBuf>,
    /// Give up on a daemon request after this long.
    #[arg(long, value_name = "MS", default_value_t = 5000)]
    timeout_ms: u64,
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let _ = e.print();
            return if e.use_stderr() {
                ExitCode::from(4)
            } else {
                ExitCode::SUCCESS
            };
        }
    };
    let client = match Client::resolve(
        cli.socket.as_deref(),
        cli.token.as_deref(),
        cli.token_file.as_deref(),
        cli.timeout_ms,
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("dettivo-mcp: {}", e.message());
            return ExitCode::from(4);
        }
    };
    match dettivo_mcp::serve(Config::from_daemon(client)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dettivo-mcp: {e}");
            ExitCode::from(1)
        }
    }
}
