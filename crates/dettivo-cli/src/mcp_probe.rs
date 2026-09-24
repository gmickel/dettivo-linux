//! The transport half of `dettivo mcp check`: `dettivo mcp serve` runs as
//! a child of this process, once per framing, and the check speaks MCP to
//! it the way a host would (`initialize`, `notifications/initialized`,
//! `tools/list`, then the harmless daemon-backed `get_status` tool), so
//! the framing line in the report names what actually answered. The
//! handshake is bounded: a server that hangs is killed at the deadline
//! and reported, never waited on.

use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use dettivo_mcp::harness::{PROTOCOL_VERSION, Session};
use dettivo_mcp::transport::Framing;
use dettivo_mcp::{SERVER_NAME, SERVER_VERSION};
use serde_json::{Value, json};

/// What one framing's handshake proved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    /// The framing spoken.
    pub framing: &'static str,
    /// `initialize` answered with this server's name and version.
    pub initialize: bool,
    /// The tool count `tools/list` answered.
    pub tools_listed: Option<usize>,
    /// The `get_status` tool answered a healthy daemon.
    pub get_status: bool,
    /// The step that failed and why, when one did.
    pub error: Option<String>,
}

impl Probe {
    /// Every step ran and answered.
    pub fn ok(&self) -> bool {
        self.initialize && self.tools_listed.is_some() && self.get_status && self.error.is_none()
    }

    /// The report row.
    pub fn json(&self) -> Value {
        json!({
            "framing": self.framing,
            "initialize": self.initialize,
            "tools_listed": self.tools_listed,
            "get_status": self.get_status,
            "error": self.error,
        })
    }

    /// One clause of the human framing line.
    pub fn human(&self) -> String {
        match &self.error {
            None => format!(
                "{}: initialize, tools/list {}, get_status",
                self.framing,
                self.tools_listed.unwrap_or(0)
            ),
            Some(why) => format!("{}: FAILED ({why})", self.framing),
        }
    }
}

/// The command a host would run: this binary, `mcp serve`, against
/// `socket`, with the token in the environment when the daemon wants one.
fn serve_command(socket: &Path, token: Option<&str>) -> std::io::Result<Command> {
    let mut command = Command::new(std::env::current_exe()?);
    command.arg("--socket").arg(socket).args(["mcp", "serve"]);
    command.env_remove("DETTIVO_IPC_SOCKET");
    match token {
        Some(token) => command.env("DETTIVO_IPC_TOKEN", token),
        None => command.env_remove("DETTIVO_IPC_TOKEN"),
    };
    Ok(command)
}

fn handshake(session: &mut Session, probe: &mut Probe) -> Result<(), String> {
    let init = session.result(
        "initialize",
        json!({"protocolVersion": PROTOCOL_VERSION, "capabilities": {}, "clientInfo": {"name": "dettivo mcp check", "version": SERVER_VERSION}}),
    )?;
    if init["serverInfo"]["name"] != SERVER_NAME || init["serverInfo"]["version"] != SERVER_VERSION
    {
        return Err(format!(
            "initialize answered serverInfo {}",
            init["serverInfo"]
        ));
    }
    probe.initialize = true;
    session.notify("notifications/initialized", json!({}))?;
    let tools = session.result("tools/list", json!({}))?;
    let listed = tools["tools"]
        .as_array()
        .ok_or_else(|| format!("tools/list answered {tools}"))?
        .len();
    probe.tools_listed = Some(listed);
    let status = session.call_tool("get_status", json!({}))?;
    if status["isError"] == true || status["structuredContent"]["ok"] != true {
        return Err(format!("get_status answered {}", Session::text_of(&status)));
    }
    probe.get_status = true;
    Ok(())
}

/// Speaks `framing` to a fresh `mcp serve` child within `timeout`.
pub fn probe(socket: &Path, token: Option<&str>, framing: Framing, timeout: Duration) -> Probe {
    let mut probe = Probe {
        framing: framing.name(),
        initialize: false,
        tools_listed: None,
        get_status: false,
        error: None,
    };
    let command = match serve_command(socket, token) {
        Ok(c) => c,
        Err(e) => {
            probe.error = Some(format!("spawn: {e}"));
            return probe;
        }
    };
    let mut session = match Session::spawn(command, framing) {
        Ok(s) => s,
        Err(e) => {
            probe.error = Some(format!("spawn: {e}"));
            return probe;
        }
    };
    let pid = session.pid();
    let (tx, rx) = mpsc::channel();
    let mut inner = probe.clone();
    let worker = std::thread::spawn(move || {
        let outcome = handshake(&mut session, &mut inner);
        // The child leaves with the session: its stdin closes and the
        // session's drop reaps it.
        drop(session);
        let _ = tx.send((inner, outcome));
    });
    match rx.recv_timeout(timeout) {
        Ok((mut done, outcome)) => {
            if let Err(why) = outcome {
                done.error = Some(why);
            }
            let _ = worker.join();
            done
        }
        Err(_) => {
            // The server never answered: kill it so the blocked read ends,
            // then report the deadline.
            let _ = Command::new("kill")
                .arg("-KILL")
                .arg(pid.to_string())
                .status();
            let _ = worker.join();
            probe.error = Some(format!("no answer within {} ms", timeout.as_millis()));
            probe
        }
    }
}

/// Both framings, in the order the server auto-detects them.
pub fn probe_both(socket: &Path, token: Option<&str>, timeout: Duration) -> Vec<Probe> {
    [Framing::Line, Framing::ContentLength]
        .into_iter()
        .map(|framing| probe(socket, token, framing, timeout))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_probe_reports_what_ran_and_where_it_stopped() {
        let ok = Probe {
            framing: "line-delimited",
            initialize: true,
            tools_listed: Some(19),
            get_status: true,
            error: None,
        };
        assert!(ok.ok());
        assert_eq!(
            ok.human(),
            "line-delimited: initialize, tools/list 19, get_status"
        );
        let stopped = Probe {
            framing: "content-length",
            initialize: true,
            tools_listed: None,
            get_status: false,
            error: Some("the server closed its output".into()),
        };
        assert!(!stopped.ok());
        assert_eq!(
            stopped.human(),
            "content-length: FAILED (the server closed its output)"
        );
        assert_eq!(stopped.json()["tools_listed"], Value::Null);
    }

    #[test]
    fn a_server_that_never_answers_is_killed_at_the_deadline() {
        // `sleep` stands in for a server that accepts the connection and
        // says nothing; the probe must come back with the deadline named.
        let mut command = Command::new("sleep");
        command.arg("30");
        let session = Session::spawn(command, Framing::Line).unwrap();
        let pid = session.pid();
        let started = std::time::Instant::now();
        let (tx, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut session = session;
            let read = session.read();
            let _ = tx.send(read.clone());
            read
        });
        assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
        let _ = Command::new("kill")
            .arg("-KILL")
            .arg(pid.to_string())
            .status();
        let read = worker.join().unwrap();
        assert!(read.is_err(), "{read:?}");
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
