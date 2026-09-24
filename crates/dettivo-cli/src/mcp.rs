//! `dettivo mcp serve | config | check`: the MCP server run in-process
//! over stdio, the host configuration printed or merged into the host's
//! file, and a check that initialises against the daemon and reports the
//! tool count and the framing (ADR 0019, docs/mcp.md).

use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Subcommand;
use dettivo_mcp::hosts::{self, Host};
use dettivo_mcp::protocol::Config;
use dettivo_mcp::{resources, tools};
use serde_json::{Value, json};

use crate::client::Client;
use crate::exit::{Exit, Failure};
use crate::mcp_probe;
use crate::{Cli, output};

/// `dettivo mcp <what>`.
#[derive(Debug, Subcommand)]
pub enum McpCmd {
    /// Serve MCP over standard input and output (what a host's config runs).
    Serve,
    /// Print a host's server entry, or merge it into the host's file with --write.
    Config {
        /// claude-desktop, claude-code, cursor or codex.
        #[arg(long, value_name = "HOST")]
        host: String,
        /// The server name inside the host's file.
        #[arg(long, default_value = "dettivo")]
        name: String,
        /// The dettivo command path (default: ~/.local/bin/dettivo, then this binary).
        #[arg(long, value_name = "PATH")]
        command: Option<String>,
        /// Merge the entry into the host's file, keeping every other server.
        #[arg(long)]
        write: bool,
        /// Add the DETTIVO_IPC_TOKEN placeholder for peer_token mode.
        #[arg(long)]
        hardened: bool,
    },
    /// Initialise against the daemon and report the tool count and the framing.
    Check,
}

/// Runs `dettivo mcp`.
pub fn run(cli: &Cli, client: &Client, what: &McpCmd) -> Result<(), Failure> {
    match what {
        McpCmd::Serve => serve(client),
        McpCmd::Config {
            host,
            name,
            command,
            write,
            hardened,
        } => config(
            cli,
            client,
            host,
            name,
            command.as_deref(),
            *write,
            *hardened,
        ),
        McpCmd::Check => check(cli, client),
    }
}

fn mcp_client(client: &Client) -> dettivo_mcp::client::Client {
    dettivo_mcp::client::Client {
        socket: client.socket.clone(),
        token: client.token.clone(),
        timeout: client.timeout,
    }
}

fn serve(client: &Client) -> Result<(), Failure> {
    dettivo_mcp::serve(Config::from_daemon(mcp_client(client)))
        .map_err(|e| Failure::reported(Exit::Failure, format!("mcp serve: {e}")))
}

/// `[mcp] hardened` through the daemon; false when no daemon answers.
fn hardened_by_config(client: &Client) -> bool {
    client
        .call("config.get", json!({"key": "mcp.hardened"}))
        .ok()
        .and_then(|v| v["entries"][0]["value"].as_bool())
        .unwrap_or(false)
}

fn config(
    cli: &Cli,
    client: &Client,
    host: &str,
    name: &str,
    command: Option<&str>,
    write: bool,
    hardened: bool,
) -> Result<(), Failure> {
    let host = Host::parse(host).map_err(|e| Failure::new(Exit::InvalidArgs, e))?;
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("dettivo"));
    let opts = hosts::Options {
        host,
        name: name.to_string(),
        command: hosts::resolve_command(command, &exe, &home),
        socket: cli
            .socket
            .as_ref()
            .map(|s| s.to_string_lossy().into_owned()),
        hardened: hardened || hardened_by_config(client),
    };
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let path = host.config_path(|k| std::env::var_os(k), &cwd);
    if write {
        hosts::write(&opts, &path).map_err(|e| Failure::new(Exit::Failure, e))?;
        if cli.json {
            output::result(
                cli,
                "mcp.config",
                &json!({"host": host.name(), "path": path.to_string_lossy(), "written": true, "hardened": opts.hardened}),
            );
        } else if !cli.quiet {
            println!("Wrote MCP config to {}", path.display());
            if opts.hardened {
                println!("Remember to set DETTIVO_IPC_TOKEN in your host environment.");
            }
        }
        return Ok(());
    }
    let text = hosts::render(&opts).map_err(|e| Failure::new(Exit::Failure, e))?;
    if cli.json {
        output::result(
            cli,
            "mcp.config",
            &json!({"host": host.name(), "name": name, "path": path.to_string_lossy(), "server": hosts::server_entry(&opts), "text": text}),
        );
    } else if !cli.quiet {
        print!("{text}");
        println!("{}", next_step(host, &path));
    }
    Ok(())
}

/// The line under a printed entry.
pub fn next_step(host: Host, path: &Path) -> String {
    match host {
        Host::ClaudeCode => format!(
            "\nNext step: run this line, or `--write` for a project {} in the current directory.",
            path.file_name().unwrap_or_default().to_string_lossy()
        ),
        _ => format!(
            "\nNext step: merge this into {}, or run again with --write.",
            path.display()
        ),
    }
}

/// How long each framing's handshake with the served child may take.
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

fn check(cli: &Cli, client: &Client) -> Result<(), Failure> {
    let caps = client.call("system.capabilities", json!({}))?;
    let version = client.call("system.version", json!({}))?;
    let shown = tools::available(Some(&caps));
    let hidden: Vec<&str> = tools::MEETING_TOOLS
        .iter()
        .copied()
        .filter(|m| !shown.iter().any(|t| t.name == *m))
        .collect();
    let hardened = caps["auth"]["ipc_mode"] == "peer_token" || hardened_by_config(client);
    // The framing line is evidence: `mcp serve` runs as a child once per
    // framing and answers the handshake, or the check fails naming the
    // framing and the step.
    let probes = mcp_probe::probe_both(&client.socket, client.token.as_deref(), PROBE_TIMEOUT);
    let framing: Vec<&str> = probes
        .iter()
        .filter(|p| p.ok())
        .map(|p| p.framing)
        .collect();
    let report = json!({
        "server": {"name": dettivo_mcp::SERVER_NAME, "version": dettivo_mcp::SERVER_VERSION},
        "api_version": version["api_version"],
        "socket": client.socket.to_string_lossy(),
        "auth_mode": caps["auth"]["ipc_mode"],
        "hardened": hardened,
        "tools": shown.len(),
        "hidden_tools": hidden,
        "resource_templates": resources::templates().len(),
        "framing": framing,
        "transport": probes.iter().map(mcp_probe::Probe::json).collect::<Vec<_>>(),
    });
    if cli.json {
        output::result(cli, "mcp.check", &report);
    } else if !cli.quiet {
        print!("{}", check_lines(&report));
    }
    let failed: Vec<String> = probes
        .iter()
        .filter(|p| !p.ok())
        .map(mcp_probe::Probe::human)
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(Failure::reported(
            Exit::Failure,
            format!(
                "mcp serve did not answer the handshake: {}",
                failed.join("; ")
            ),
        ))
    }
}

/// The human `mcp check` lines.
pub fn check_lines(report: &Value) -> String {
    let hidden: Vec<&str> = report["hidden_tools"]
        .as_array()
        .map(|h| h.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let transport: Vec<String> = report["transport"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|row| match row["error"].as_str() {
                    None => format!(
                        "{}: initialize, tools/list {}, get_status",
                        row["framing"].as_str().unwrap_or("?"),
                        row["tools_listed"].as_u64().unwrap_or(0)
                    ),
                    Some(why) => {
                        format!("{}: FAILED ({why})", row["framing"].as_str().unwrap_or("?"))
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    format!(
        "server    {} {} (api {})\nsocket    {} ({}{})\ntools     {}{}\nresources {} templates\nframing   {} (mirrored from the client)\n",
        report["server"]["name"].as_str().unwrap_or("?"),
        report["server"]["version"].as_str().unwrap_or("?"),
        report["api_version"].as_str().unwrap_or("?"),
        report["socket"].as_str().unwrap_or("?"),
        report["auth_mode"].as_str().unwrap_or("?"),
        if report["hardened"] == Value::Bool(true) {
            ", hardened"
        } else {
            ""
        },
        report["tools"].as_u64().unwrap_or(0),
        if hidden.is_empty() {
            String::new()
        } else {
            format!(" (hidden: {})", hidden.join(", "))
        },
        report["resource_templates"].as_u64().unwrap_or(0),
        transport.join("; "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_lines_name_the_count_the_framing_and_hidden_tools() {
        let report = json!({
            "server": {"name": "dettivo-mcp", "version": "1.0.0"},
            "api_version": "1.0.0",
            "socket": "/run/s.sock",
            "auth_mode": "peer_token",
            "hardened": true,
            "tools": 15,
            "hidden_tools": ["start_meeting"],
            "resource_templates": 6,
            "transport": [
                {"framing": "line-delimited", "initialize": true, "tools_listed": 15, "get_status": true, "error": null},
                {"framing": "content-length", "initialize": true, "tools_listed": null, "get_status": false, "error": "the server closed its output"},
            ],
        });
        let text = check_lines(&report);
        assert_eq!(
            text,
            "server    dettivo-mcp 1.0.0 (api 1.0.0)\nsocket    /run/s.sock (peer_token, hardened)\ntools     15 (hidden: start_meeting)\nresources 6 templates\nframing   line-delimited: initialize, tools/list 15, get_status; content-length: FAILED (the server closed its output) (mirrored from the client)\n"
        );
        assert!(
            next_step(Host::ClaudeCode, Path::new("/p/.mcp.json"))
                .contains("`--write` for a project .mcp.json")
        );
        assert!(next_step(Host::Codex, Path::new("/h/.codex/config.toml")).contains("--write"));
    }
}
