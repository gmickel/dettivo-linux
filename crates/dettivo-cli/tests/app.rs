//! `dettivo app` against a stand-in instance socket: `open <route>` and a
//! bare `app` are one JSON line each, an unknown route exits 4 naming the
//! routes without touching the socket, a refused route from the app exits
//! 4 with its message, `status` without an app exits 2, and without a
//! socket the app is launched with the route on its command line.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::thread;

use serde_json::{Value, json};
use tempfile::TempDir;

const CLI: &str = env!("CARGO_BIN_EXE_dettivo");

struct App {
    dir: TempDir,
    requests: std::sync::Arc<std::sync::Mutex<Vec<Value>>>,
}

impl App {
    fn serve() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("dta")
            .tempdir_in("/tmp")
            .unwrap();
        std::fs::create_dir_all(dir.path().join("run/dettivo")).unwrap();
        let listener = UnixListener::bind(dir.path().join("run/dettivo/app.sock")).unwrap();
        let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = requests.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    continue;
                }
                let request: Value = serde_json::from_str(line.trim_end()).unwrap();
                let reply = match request["cmd"].as_str() {
                    Some("status") => {
                        json!({"ok": true, "route": "home", "daemon_connected": true})
                    }
                    Some("open") if request["route"] == "settings.agents" => {
                        json!({"ok": false, "error": "unknown route \"settings.agents\""})
                    }
                    _ => json!({"ok": true}),
                };
                seen.lock().unwrap().push(request);
                let mut stream = stream;
                let _ = writeln!(stream, "{reply}");
            }
        });
        Self { dir, requests }
    }

    fn socket(&self) -> PathBuf {
        self.dir.path().join("run/dettivo/dettivo.sock")
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(CLI)
            .env("DETTIVO_IPC_SOCKET", self.socket())
            .env("XDG_RUNTIME_DIR", self.dir.path().join("run"))
            .env("PATH", self.dir.path().join("empty-path"))
            .args(args)
            .output()
            .unwrap()
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn open_raise_and_status_are_one_line_each_way() {
    let app = App::serve();
    let open = app.cli(&["--json", "app", "open", "history"]);
    assert_eq!(open.status.code(), Some(0), "{}", stderr(&open));
    let answer: Value = serde_json::from_str(&stdout(&open)).unwrap();
    assert_eq!(answer["raised"], true);
    assert_eq!(answer["route"], "history");

    let detail = app.cli(&[
        "app",
        "open",
        "history.detail",
        "--id",
        "5eed0000-0000-4000-8000-000000000011",
    ]);
    assert_eq!(detail.status.code(), Some(0), "{}", stderr(&detail));

    let raise = app.cli(&["--quiet", "app"]);
    assert_eq!(raise.status.code(), Some(0), "{}", stderr(&raise));
    assert_eq!(stdout(&raise), "");

    let status = app.cli(&["app", "status"]);
    assert_eq!(status.status.code(), Some(0));
    assert!(
        stdout(&status).contains("route: home"),
        "{}",
        stdout(&status)
    );

    let refused = app.cli(&["app", "open", "settings.agents"]);
    assert_eq!(refused.status.code(), Some(4));
    assert!(stderr(&refused).contains("unknown route"));

    let unknown = app.cli(&["app", "open", "garage"]);
    assert_eq!(unknown.status.code(), Some(4));
    assert!(
        stderr(&unknown).contains("meetings.live"),
        "{}",
        stderr(&unknown)
    );

    let seen = app.requests.lock().unwrap();
    assert_eq!(seen[0], json!({"cmd": "open", "route": "history"}));
    assert_eq!(
        seen[1],
        json!({"cmd": "open", "route": "history.detail", "arg": "5eed0000-0000-4000-8000-000000000011"})
    );
    assert_eq!(seen[2], json!({"cmd": "raise"}));
    assert_eq!(seen[3], json!({"cmd": "status"}));
    assert_eq!(seen[4], json!({"cmd": "open", "route": "settings.agents"}));
    assert_eq!(seen.len(), 5, "an unknown route never reaches the app");
}

#[test]
fn without_a_window_the_app_is_launched_on_the_route_or_named_as_missing() {
    let dir = tempfile::Builder::new()
        .prefix("dta")
        .tempdir_in("/tmp")
        .unwrap();
    let socket = dir.path().join("run/dettivo/dettivo.sock");
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let args_file = dir.path().join("args.txt");
    let fake = bin.join("dettivo-app");
    std::fs::write(
        &fake,
        format!("#!/bin/sh\necho \"$@\" > {}\n", args_file.display()),
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let run = |args: &[&str], path: &std::path::Path| {
        Command::new(CLI)
            .env("DETTIVO_IPC_SOCKET", &socket)
            .env("XDG_RUNTIME_DIR", dir.path().join("run"))
            .env("PATH", path)
            .args(args)
            .output()
            .unwrap()
    };
    let launched = run(&["--json", "app", "open", "meetings"], &bin);
    assert_eq!(launched.status.code(), Some(0), "{}", stderr(&launched));
    let answer: Value = serde_json::from_str(&stdout(&launched)).unwrap();
    assert!(answer["pid"].as_u64().unwrap() > 0);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !args_file.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        std::fs::read_to_string(&args_file).unwrap().trim(),
        "--open meetings"
    );

    let missing = run(&["app"], &dir.path().join("nowhere"));
    assert_eq!(missing.status.code(), Some(2), "{}", stderr(&missing));
    assert!(stderr(&missing).contains("dettivo-app"));

    let status = run(&["app", "status"], &bin);
    assert_eq!(status.status.code(), Some(2));
    assert!(stderr(&status).contains("not running"));
}
