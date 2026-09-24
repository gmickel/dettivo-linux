//! `dettivo osd show|hide|status` against a stand-in control socket: one
//! JSON line each way, `--json` prints the answer untouched, a refused
//! command exits 4 with the pill's reason, and an absent pill exits 2
//! naming the unit. `doctor` reports the pill without judging it.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::thread;

use serde_json::{Value, json};
use tempfile::TempDir;

const CLI: &str = env!("CARGO_BIN_EXE_dettivo");

struct Pill {
    dir: TempDir,
    requests: std::sync::Arc<std::sync::Mutex<Vec<Value>>>,
}

impl Pill {
    /// A control socket that answers every line with a status object and
    /// refuses the `sleeping` state.
    fn serve() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("dto")
            .tempdir_in("/tmp")
            .unwrap();
        std::fs::create_dir_all(dir.path().join("run/dettivo")).unwrap();
        let listener = UnixListener::bind(dir.path().join("run/dettivo/osd.sock")).unwrap();
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
                let reply = if request["state"] == "sleeping" {
                    json!({"ok": false, "error": "unknown state: sleeping"})
                } else {
                    json!({
                        "ok": true, "host": "window", "position": "top", "monitor": "DP-3",
                        "visible": request["cmd"] == "show", "state": request["state"].as_str().unwrap_or("hidden")
                    })
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
fn show_hide_and_status_are_one_line_each_way() {
    let pill = Pill::serve();
    let show = pill.cli(&[
        "--json",
        "osd",
        "show",
        "inserted",
        "--target",
        "ghostty",
        "--words",
        "Add a regression test",
    ]);
    assert_eq!(show.status.code(), Some(0), "{}", stderr(&show));
    let answer: Value = serde_json::from_str(&stdout(&show)).unwrap();
    assert_eq!(answer["visible"], true);
    assert_eq!(answer["state"], "inserted");

    let hide = pill.cli(&["--quiet", "osd", "hide"]);
    assert_eq!(hide.status.code(), Some(0));
    assert_eq!(stdout(&hide), "");

    let status = pill.cli(&["osd", "status"]);
    assert_eq!(status.status.code(), Some(0));
    assert!(
        stdout(&status).contains("host: window"),
        "{}",
        stdout(&status)
    );

    let refused = pill.cli(&["osd", "show", "sleeping"]);
    assert_eq!(refused.status.code(), Some(4));
    assert!(stderr(&refused).contains("unknown state: sleeping"));

    let seen = pill.requests.lock().unwrap();
    assert_eq!(seen[0]["cmd"], "show");
    assert_eq!(seen[0]["target"], "ghostty");
    assert_eq!(seen[0]["words"], "Add a regression test");
    assert_eq!(seen[1], json!({"cmd": "hide"}));
    assert_eq!(seen[2], json!({"cmd": "status"}));
}

#[test]
fn an_absent_pill_exits_2_naming_the_unit_and_doctor_reports_it() {
    let dir = tempfile::Builder::new()
        .prefix("dto")
        .tempdir_in("/tmp")
        .unwrap();
    let socket = dir.path().join("run/dettivo/dettivo.sock");
    let run = |args: &[&str]| {
        Command::new(CLI)
            .env("DETTIVO_IPC_SOCKET", &socket)
            .env("XDG_RUNTIME_DIR", dir.path().join("run"))
            .args(args)
            .output()
            .unwrap()
    };
    let status = run(&["osd", "status"]);
    assert_eq!(status.status.code(), Some(2), "{}", stderr(&status));
    assert!(stderr(&status).contains("dettivo-osd.service"));

    // Doctor: no pill, no notice -> not running; a notice -> disabled with it.
    let doctor = run(&["--json", "doctor"]);
    let report: Value = serde_json::from_str(&stdout(&doctor)).unwrap();
    assert_eq!(report["osd"]["host"], "absent");
    std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
    std::fs::write(
        socket.parent().unwrap().join("osd.status.json"),
        json!({"host": "disabled", "notice": "no layer shell and host = layer_shell"}).to_string(),
    )
    .unwrap();
    let doctor = run(&["doctor"]);
    assert!(
        stdout(&doctor).contains("osd       disabled: no layer shell and host = layer_shell"),
        "{}",
        stdout(&doctor)
    );
}
