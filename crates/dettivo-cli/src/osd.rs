//! `dettivo osd show|hide|status`: one JSON line to `dettivo-osd` over its
//! control socket (`osd.sock` beside the daemon socket) and one line back.
//! No daemon is involved, so a script can show a state or ask which host
//! the pill runs on while the daemon is down.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use clap::Subcommand;
use serde_json::{Value, json};

use crate::exit::{Exit, Failure};
use crate::{Cli, output};

/// The user unit that runs the pill outside Omarchy.
pub const OSD_UNIT: &str = "dettivo-osd.service";

/// `dettivo osd <what>`. Parsed once per run, so the show variant's
/// options outweighing the bare verbs costs nothing.
#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum OsdCmd {
    /// Show one pill state (listening, transcribing, enhancing, inserted, copied, error).
    Show {
        /// The state.
        state: String,
        /// The title (error only; the other states name themselves).
        #[arg(long)]
        title: Option<String>,
        /// The hint after the title while listening.
        #[arg(long)]
        hint: Option<String>,
        /// The engine name while transcribing.
        #[arg(long)]
        engine: Option<String>,
        /// The first words (enhancing, inserted).
        #[arg(long)]
        words: Option<String>,
        /// The target application (inserted).
        #[arg(long)]
        target: Option<String>,
        /// Why (copied, error).
        #[arg(long)]
        reason: Option<String>,
        /// What to do next (copied, error).
        #[arg(long)]
        action: Option<String>,
        /// The bar level, 0 to 1 (listening).
        #[arg(long)]
        level: Option<f64>,
    },
    /// Hide the pill.
    Hide,
    /// Which host runs the pill, where it sits and what it shows; the disabled notice when no process runs.
    Status,
    /// Hold the Omarchy panel's bus name while the plugin hosts the pill (exits 0 at once unless `[omarchy] osd = "panel"`).
    HostPanel,
}

/// The control socket beside the daemon socket: `<dir>/osd.sock`.
pub fn socket_for(daemon_socket: &Path) -> PathBuf {
    daemon_socket
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("osd.sock")
}

/// The notice `dettivo-osd` leaves when it exits disabled: `<dir>/osd.status.json`.
pub fn status_file_for(daemon_socket: &Path) -> PathBuf {
    socket_for(daemon_socket).with_file_name("osd.status.json")
}

/// Runs an `osd` subcommand.
pub fn run(cli: &Cli, client: &crate::client::Client, what: &OsdCmd) -> Result<(), Failure> {
    let daemon_socket: &Path = &client.socket;
    let request = match what {
        OsdCmd::HostPanel => return host_panel(cli, client),
        OsdCmd::Show {
            state,
            title,
            hint,
            engine,
            words,
            target,
            reason,
            action,
            level,
        } => {
            let mut req = json!({ "cmd": "show", "state": state });
            let fields: [(&str, Option<Value>); 8] = [
                ("title", title.clone().map(Value::String)),
                ("hint", hint.clone().map(Value::String)),
                ("engine", engine.clone().map(Value::String)),
                ("words", words.clone().map(Value::String)),
                ("target", target.clone().map(Value::String)),
                ("reason", reason.clone().map(Value::String)),
                ("action", action.clone().map(Value::String)),
                ("level", level.map(|l| json!(l))),
            ];
            for (key, value) in fields {
                if let Some(v) = value {
                    req[key] = v;
                }
            }
            req
        }
        OsdCmd::Hide => json!({ "cmd": "hide" }),
        OsdCmd::Status => json!({ "cmd": "status" }),
    };
    let socket = socket_for(daemon_socket);
    let reply = match call(&socket, &request, cli.timeout_ms) {
        Ok(reply) => reply,
        // No process answers: the notice the last one left says why
        // (the panel plugin holds the pill, or [osd] is off).
        Err(absent) if matches!(what, OsdCmd::Status) => {
            match std::fs::read_to_string(status_file_for(daemon_socket))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            {
                Some(notice) => notice,
                None => return Err(absent),
            }
        }
        Err(e) => return Err(e),
    };
    if reply.get("ok") == Some(&Value::Bool(false)) {
        let message = reply
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("the pill refused the command");
        if cli.json && !cli.quiet {
            println!("{reply}");
        }
        return Err(Failure::new(Exit::InvalidArgs, message.to_string()));
    }
    output::result(cli, "osd", &reply);
    Ok(())
}

/// One request line, one reply line. An absent socket is exit 2 naming
/// the unit, like an absent daemon.
pub fn call(socket: &Path, request: &Value, timeout_ms: u64) -> Result<Value, Failure> {
    let timeout = std::time::Duration::from_millis(timeout_ms.max(1));
    let mut stream = UnixStream::connect(socket).map_err(|e| {
        Failure::new(
            Exit::Unavailable,
            format!(
                "dettivo-osd is not running at {} ({e}); start it with `systemctl --user start {OSD_UNIT}` or run dettivo-osd",
                socket.display()
            ),
        )
    })?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .map_err(|e| Failure::new(Exit::Failure, format!("socket setup: {e}")))?;
    let mut line = request.to_string();
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .map_err(|e| Failure::new(Exit::Unavailable, format!("write to the pill failed: {e}")))?;
    let mut answer = String::new();
    let n = BufReader::new(stream).read_line(&mut answer).map_err(|e| {
        let exit = match e.kind() {
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => Exit::Timeout,
            _ => Exit::Unavailable,
        };
        Failure::new(exit, format!("no answer from the pill: {e}"))
    })?;
    if n == 0 {
        return Err(Failure::new(
            Exit::Unavailable,
            "the pill closed the connection without answering",
        ));
    }
    serde_json::from_str(answer.trim_end()).map_err(|e| {
        Failure::new(
            Exit::Failure,
            format!("unreadable answer from the pill: {e}"),
        )
    })
}

/// `dettivo osd host-panel`: claims `dev.dettivo.OmarchyPanel` on the
/// session bus and holds it until the parent (the shell's process item)
/// goes away or the process is signalled, so `dettivo-osd` sees the
/// panel and steps aside. `[omarchy] osd` other than `panel` (or a daemon
/// that says so) exits 0 at once without claiming, and a name another
/// panel holds exits 1 naming it.
fn host_panel(cli: &Cli, client: &crate::client::Client) -> Result<(), Failure> {
    let mode = client
        .call("config.get", json!({ "key": "omarchy.osd" }))
        .ok()
        .and_then(|r| r["entries"][0]["value"].as_str().map(str::to_string))
        .unwrap_or_else(|| "panel".to_string());
    if mode != "panel" {
        output::result(
            cli,
            "osd.host_panel",
            &json!({ "claimed": false, "reason": format!("[omarchy] osd = \"{mode}\"; dettivo-osd keeps the pill") }),
        );
        return Ok(());
    }
    let name = crate::omarchy::PANEL_BUS_NAME;
    let connection = zbus::blocking::Connection::session()
        .map_err(|e| Failure::new(Exit::Unavailable, format!("session bus: {e}")))?;
    connection
        .request_name(name)
        .map_err(|e| Failure::new(Exit::Failure, format!("cannot claim {name}: {e}")))?;
    output::result(
        cli,
        "osd.host_panel",
        &json!({ "claimed": true, "name": name }),
    );
    use std::io::Write as _;
    let _ = std::io::stdout().flush();
    let parent = std::os::unix::process::parent_id();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if std::os::unix::process::parent_id() != parent {
            return Ok(());
        }
    }
}

/// The `osd` block of `dettivo doctor`: the live status when the socket
/// answers, the disabled notice the process left behind, or "not running".
pub fn doctor_facts(daemon_socket: &Path) -> Value {
    let socket = socket_for(daemon_socket);
    if socket.exists() {
        if let Ok(status) = call(&socket, &json!({ "cmd": "status" }), 2000) {
            return status;
        }
    }
    let notice = status_file_for(daemon_socket);
    if let Ok(text) = std::fs::read_to_string(&notice) {
        if let Ok(v) = serde_json::from_str::<Value>(&text) {
            return v;
        }
    }
    json!({ "host": "absent", "socket": socket.to_string_lossy() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_control_socket_sits_beside_the_daemon_socket() {
        let daemon = Path::new("/run/user/7/dettivo/dettivo.sock");
        assert_eq!(
            socket_for(daemon),
            PathBuf::from("/run/user/7/dettivo/osd.sock")
        );
        assert_eq!(
            status_file_for(daemon),
            PathBuf::from("/run/user/7/dettivo/osd.status.json")
        );
    }

    #[test]
    fn an_absent_pill_is_unavailable_and_names_the_unit() {
        let dir = tempfile::tempdir().unwrap();
        let err = call(&dir.path().join("osd.sock"), &json!({"cmd": "status"}), 100).unwrap_err();
        assert_eq!(err.exit, Exit::Unavailable);
        assert!(err.message.contains(OSD_UNIT));
        assert_eq!(
            doctor_facts(&dir.path().join("dettivo.sock"))["host"],
            "absent"
        );
    }
}
