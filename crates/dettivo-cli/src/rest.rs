//! `dettivo rest serve | status | token`: the loopback REST shim hosted as
//! a process over the Unix socket (the daemon hosts the same shim with
//! `[rest] enabled = true`), the listener's state, and where the token
//! comes from (ADR 0028, docs/rest.md).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use clap::Subcommand;
use dettivo_rest::{Settings, Shim, SocketBackend};
use serde_json::{Value, json};

use crate::client::Client;
use crate::exit::{Exit, Failure};
use crate::{Cli, output};

/// `dettivo rest <what>`.
#[derive(Debug, Subcommand)]
pub enum RestCmd {
    /// Host the shim in this process until interrupted.
    Serve {
        /// The TCP port (default: the daemon's rest.port, else 45831; 0 for an ephemeral one).
        #[arg(long, value_name = "PORT")]
        port: Option<u16>,
        /// The loopback address to bind (default: the daemon's rest.bind, else 127.0.0.1).
        #[arg(long, value_name = "ADDR")]
        bind: Option<String>,
    },
    /// Report the listener: hosted by the daemon or a process, answering, authorised.
    Status,
    /// Print where the REST token comes from (never the token itself).
    Token,
}

/// Runs `dettivo rest`.
pub fn run(cli: &Cli, client: &Client, what: &RestCmd) -> Result<(), Failure> {
    match what {
        RestCmd::Serve { port, bind } => serve(cli, client, *port, bind.as_deref()),
        RestCmd::Status => status(cli, client),
        RestCmd::Token => token(cli, client),
    }
}

/// The token: `--token` and `--token-file` first, then the macOS order.
fn resolve_token(cli: &Cli, client: &Client) -> Option<(String, String)> {
    if let (Some(t), true) = (
        &client.token,
        cli.token.is_some() || cli.token_file.is_some(),
    ) {
        return Some((t.clone(), "flag".into()));
    }
    dettivo_rest::auth::resolve_token().map(|(t, origin)| (t, origin.as_str().to_string()))
}

/// The `[rest]` section as the daemon has it, or the defaults when the
/// daemon does not answer.
fn settings(client: &Client) -> Settings {
    let mut settings = Settings::default();
    let Ok(entries) = client.call("config.get", json!({})) else {
        return settings;
    };
    for entry in entries["entries"].as_array().into_iter().flatten() {
        let value = &entry["value"];
        match entry["key"].as_str().unwrap_or("") {
            "rest.port" => {
                settings.port = value
                    .as_u64()
                    .and_then(|p| u16::try_from(p).ok())
                    .unwrap_or(settings.port)
            }
            "rest.bind" => settings.bind = value.as_str().unwrap_or(&settings.bind).to_string(),
            "rest.max_body_bytes" => {
                settings.max_body_bytes = value.as_u64().unwrap_or(settings.max_body_bytes)
            }
            "rest.request_timeout_ms" => {
                settings.request_timeout_ms = value.as_u64().unwrap_or(settings.request_timeout_ms)
            }
            _ => {}
        }
    }
    settings
}

fn serve(cli: &Cli, client: &Client, port: Option<u16>, bind: Option<&str>) -> Result<(), Failure> {
    let Some((token, source)) = resolve_token(cli, client) else {
        return Err(Failure::new(
            Exit::InvalidArgs,
            format!(
                "no REST token: pass --token or --token-file, or set one of {}",
                dettivo_core_sources()
            ),
        ));
    };
    let mut client = client.clone();
    client.token = Some(token.clone());
    let mut settings = settings(&client);
    if let Some(p) = port {
        settings.port = p;
    }
    if let Some(b) = bind {
        settings.bind = b.to_string();
    }
    eprintln!("dettivo rest: token from {source}");
    let shim = Shim {
        backend: Arc::new(SocketBackend {
            socket: client.socket.clone(),
            token: client.token.clone(),
            timeout: client
                .timeout
                .max(Duration::from_millis(settings.request_timeout_ms)),
        }),
        token,
        settings,
    };
    dettivo_rest::server::run_blocking(shim).map_err(|e| Failure::new(Exit::Failure, e))
}

/// The three sources, named in refusals.
fn dettivo_core_sources() -> &'static str {
    dettivo_rest::auth::SOURCES
}

/// Probes the listener: a `GET /v1/system/ping` with the resolved token.
fn probe(addr: SocketAddr, token: Option<&str>) -> Value {
    let headers: Vec<(String, String)> = token
        .map(|t| vec![("Authorization".to_string(), format!("Bearer {t}"))])
        .unwrap_or_default();
    match dettivo_rest::client::request(
        addr,
        "GET",
        "/v1/system/ping",
        &headers,
        b"",
        None,
        Duration::from_secs(3),
    ) {
        Ok(reply) => {
            let authorized = match reply.status {
                200 => Some(true),
                401 => Some(false),
                _ => None,
            };
            let error = if reply.status == 200 {
                Value::Null
            } else {
                reply
                    .json()
                    .unwrap_or_else(|| json!(String::from_utf8_lossy(&reply.body)))
            };
            json!({"listening": true, "status": reply.status, "authorized": authorized, "error": error})
        }
        Err(e) => json!({"listening": false, "error": e}),
    }
}

fn status(cli: &Cli, client: &Client) -> Result<(), Failure> {
    let token = resolve_token(cli, client);
    let mut client = client.clone();
    client.token = token.as_ref().map(|(token, _)| token.clone());
    let caps = client
        .call("system.capabilities", json!({}))
        .map(|c| c["rest"].clone())
        .unwrap_or(Value::Null);
    let settings = settings(&client);
    let port = caps["port"]
        .as_u64()
        .and_then(|p| u16::try_from(p).ok())
        .unwrap_or(settings.port);
    let bind = caps["bind"].as_str().unwrap_or(&settings.bind).to_string();
    let addr = dettivo_rest::server::loopback_ip(&bind)
        .map(|ip| SocketAddr::new(ip, port))
        .map_err(|e| Failure::new(Exit::Failure, e.to_string()))?;
    let probed = probe(addr, token.as_ref().map(|(t, _)| t.as_str()));
    let daemon_hosted = caps["enabled"] == Value::Bool(true);
    let host = if daemon_hosted {
        "daemon"
    } else if probed["listening"] == Value::Bool(true) {
        "process"
    } else {
        "none"
    };
    let report = json!({
        "bind": bind,
        "port": port,
        "host": host,
        "daemon_hosted": daemon_hosted,
        "listening": probed["listening"],
        "authorized": probed["authorized"],
        "status": probed["status"],
        "error": probed["error"],
        "token_source": token.as_ref().map(|(_, s)| s.clone()),
        "base_url": format!("http://{addr}"),
    });
    if cli.json {
        output::result(cli, "rest.status", &report);
    } else if !cli.quiet {
        print!("{}", status_lines(&report));
    }
    match probed["status"].as_u64() {
        Some(200) => Ok(()),
        Some(status) => Err(Failure::reported(
            match status {
                401 => Exit::PermissionDenied,
                503 => Exit::Unavailable,
                _ => Exit::Failure,
            },
            format!(
                "REST listener on {addr} returned HTTP {status}: {}",
                probed["error"]
            ),
        )),
        None => Err(Failure::reported(
            Exit::Unavailable,
            format!(
                "no REST listener on {addr}; start the daemon with [rest] enabled = true or run `dettivo rest serve`"
            ),
        )),
    }
}

/// The human `rest status` lines.
pub fn status_lines(report: &Value) -> String {
    let host = match report["host"].as_str().unwrap_or("none") {
        "daemon" => "hosted by the daemon ([rest] enabled)".to_string(),
        "process" => "hosted by a `dettivo rest serve` process".to_string(),
        _ => "not listening (set [rest] enabled = true or run `dettivo rest serve`)".to_string(),
    };
    let auth = match report["status"].as_u64() {
        Some(200) => "answers 200 with the resolved token".to_string(),
        Some(401) => "answers, but the resolved token is refused (401)".to_string(),
        Some(503) => "answers 503; the backend is unavailable".to_string(),
        Some(status) => format!("answers HTTP {status}; request failed"),
        None => "-".to_string(),
    };
    format!(
        "listener  {} {host}\nauth      {auth}\ntoken     {}\n",
        report["base_url"].as_str().unwrap_or("?"),
        report["token_source"]
            .as_str()
            .map(|s| format!("from {s}"))
            .unwrap_or_else(|| format!("none (set one of {})", dettivo_core_sources())),
    )
}

fn token(cli: &Cli, client: &Client) -> Result<(), Failure> {
    let report = match resolve_token(cli, client) {
        Some((_, source)) => {
            json!({"found": true, "source": source, "file": dettivo_rest::auth::token_file().to_string_lossy()})
        }
        None => {
            json!({"found": false, "source": Value::Null, "file": dettivo_rest::auth::token_file().to_string_lossy()})
        }
    };
    if cli.json {
        output::result(cli, "rest.token", &report);
    } else if !cli.quiet {
        match report["source"].as_str() {
            Some("file") => println!("token from file {}", report["file"].as_str().unwrap_or("?")),
            Some(source) => println!("token from {source}"),
            None => println!(
                "no token: set one of {} (the file is {})",
                dettivo_core_sources(),
                report["file"].as_str().unwrap_or("?")
            ),
        }
    }
    if report["found"] == Value::Bool(true) {
        Ok(())
    } else {
        Err(Failure::reported(Exit::Failure, "no REST token resolvable"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_lines_name_the_host_and_the_token() {
        let lines = status_lines(&json!({
            "base_url": "http://127.0.0.1:45831", "host": "daemon", "listening": true,
            "authorized": true, "status": 200, "token_source": "environment"
        }));
        assert!(lines.contains("hosted by the daemon"));
        assert!(lines.contains("answers 200"));
        assert!(lines.contains("from environment"));
        let lines = status_lines(&json!({"base_url": "http://127.0.0.1:1", "host": "none"}));
        assert!(lines.contains("not listening"));
        assert!(lines.contains("DETTIVO_IPC_TOKEN"));
    }
}
