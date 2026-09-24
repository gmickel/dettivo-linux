//! `dettivo app [open <route> | status]`: the desktop app through its
//! single-instance socket (`app.sock` beside the daemon socket). A running
//! window is raised, or routed, with one JSON line; without one the app
//! is launched with the route on its command line. No daemon is involved.

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::Subcommand;
use serde_json::{Value, json};

use crate::exit::{Exit, Failure};
use crate::{Cli, output};

/// The route names the app accepts, as `docs/app.md` lists them.
pub const ROUTES: &[&str] = &[
    "onboarding",
    "home",
    "history",
    "history.detail",
    "meetings",
    "meetings.live",
    "meetings.detail",
    "settings",
    "settings.general",
    "settings.vocabulary",
    "settings.hotkeys",
    "settings.models",
    "settings.polish",
    "settings.insertion",
    "settings.meetings",
    "settings.agents",
    "settings.diagnostics",
    "agents",
];

/// The app binary.
pub const APP_BINARY: &str = "dettivo-app";

/// `dettivo app <what>`.
#[derive(Debug, Subcommand)]
pub enum AppCmd {
    /// Open a route in the running window, or launch the app on it.
    Open {
        /// The route: home, history, history.detail, meetings, meetings.live, meetings.detail, settings.`<section>`, agents, onboarding.
        route: String,
        /// The item a detail route opens (`history.detail --id <uuid>`).
        #[arg(long)]
        id: Option<String>,
    },
    /// The running app's route, window and daemon link.
    Status,
}

/// `<dir>/app.sock` beside the daemon socket.
pub fn socket_for(daemon_socket: &Path) -> PathBuf {
    daemon_socket
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("app.sock")
}

/// Runs `dettivo app` (raise or launch) and its subcommands.
pub fn run(cli: &Cli, daemon_socket: &Path, what: Option<&AppCmd>) -> Result<(), Failure> {
    let socket = socket_for(daemon_socket);
    match what {
        Some(AppCmd::Status) => {
            let reply = crate::osd::call(&socket, &json!({"cmd": "status"}), cli.timeout_ms)
                .map_err(not_running)?;
            output::result(cli, "app", &reply);
            Ok(())
        }
        Some(AppCmd::Open { route, id }) => {
            if !ROUTES.contains(&route.as_str()) {
                return Err(Failure::new(
                    Exit::InvalidArgs,
                    format!(
                        "unknown route {route:?}; the routes are {}",
                        ROUTES.join(", ")
                    ),
                ));
            }
            forward_or_launch(cli, &socket, Some(route), id.as_deref())
        }
        None => forward_or_launch(cli, &socket, None, None),
    }
}

/// One line to the running app, or a launch when nothing answers.
fn forward_or_launch(
    cli: &Cli,
    socket: &Path,
    route: Option<&str>,
    id: Option<&str>,
) -> Result<(), Failure> {
    let request = match (route, id) {
        (Some(r), Some(item)) => json!({"cmd": "open", "route": r, "arg": item}),
        (Some(r), None) => json!({"cmd": "open", "route": r}),
        (None, _) => json!({"cmd": "raise"}),
    };
    if socket.exists() {
        match crate::osd::call(socket, &request, cli.timeout_ms) {
            Ok(reply) => {
                if reply.get("ok") == Some(&Value::Bool(false)) {
                    let message = reply
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("the app refused the request");
                    return Err(Failure::new(Exit::InvalidArgs, message.to_string()));
                }
                output::result(cli, "app", &json!({"raised": true, "route": route}));
                return Ok(());
            }
            Err(f) if f.exit == Exit::Unavailable => {}
            Err(f) => return Err(f),
        }
    }
    let binary = find_binary().ok_or_else(|| {
        Failure::new(
            Exit::Unavailable,
            format!("{APP_BINARY} is not running and is not installed beside dettivo or on PATH"),
        )
    })?;
    let mut command = Command::new(&binary);
    if let Some(r) = route {
        command.args(["--open", r]);
    }
    if let Some(item) = id {
        command.args(["--id", item]);
    }
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|e| Failure::new(Exit::Failure, format!("launch {}: {e}", binary.display())))?;
    output::result(
        cli,
        "app",
        &json!({"launched": binary.to_string_lossy(), "pid": child.id(), "route": route}),
    );
    Ok(())
}

/// Beside this binary first (a checkout or a package), then `PATH`.
fn find_binary() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let beside = dir.join(APP_BINARY);
            if beside.is_file() {
                return Some(beside);
            }
        }
    }
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|d| d.join(APP_BINARY))
            .find(|p| p.is_file())
    })
}

fn not_running(failure: Failure) -> Failure {
    if failure.exit == Exit::Unavailable {
        Failure::new(
            Exit::Unavailable,
            format!(
                "{APP_BINARY} is not running; start it with `dettivo app` or from the launcher"
            ),
        )
    } else {
        failure
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_instance_socket_sits_beside_the_daemon_socket() {
        assert_eq!(
            socket_for(Path::new("/run/user/7/dettivo/dettivo.sock")),
            PathBuf::from("/run/user/7/dettivo/app.sock")
        );
    }

    #[test]
    fn the_route_table_matches_the_app_documentation() {
        let docs =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/app.md"))
                .unwrap();
        for route in ROUTES {
            assert!(
                docs.contains(&format!("`{route}`")),
                "{route} is not in docs/app.md"
            );
        }
    }
}
