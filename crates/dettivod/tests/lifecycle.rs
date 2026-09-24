//! R1 and R4: socket activation readiness, one daemon per session, stale
//! socket recovery, and a clean shutdown that releases the socket.

mod common;

use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{DAEMON, Daemon, Tree};
use serde_json::json;

#[test]
fn a_second_start_exits_1_and_names_the_running_instance() {
    let daemon = Daemon::spawn(Tree::new(), &[]);
    let output = daemon
        .tree
        .command()
        .stderr(Stdio::piped())
        .output()
        .expect("second start");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already running"), "{stderr}");
    assert!(
        stderr.contains(&format!("pid {}", daemon.pid())),
        "{stderr}"
    );
    assert!(
        stderr.contains(daemon.tree.socket().to_str().unwrap()),
        "{stderr}"
    );
    let ping = daemon.result("system.ping", json!({}));
    assert_eq!(ping["ok"], true, "the first daemon keeps serving");
    daemon.stop();
}

/// daemon/F2: a second daemon on the same data directory, even one told
/// to listen elsewhere, exits before its recovery can touch the rows and
/// takes the first one is still working on.
#[test]
fn a_second_daemon_on_the_same_data_exits_before_recovery() {
    let daemon = Daemon::spawn(Tree::new(), &[]);
    let seeded = daemon.result(
        "transcripts.list",
        json!({"kinds": ["dictation"], "limit": 5}),
    );
    let other_socket = daemon.tree.root().join("run/dettivo/other.sock");
    let output = daemon
        .tree
        .command()
        .arg("--socket")
        .arg(&other_socket)
        .stderr(Stdio::piped())
        .output()
        .expect("second start");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already running"), "{stderr}");
    assert!(
        stderr.contains(&format!("pid {}", daemon.pid())),
        "{stderr}"
    );
    assert!(!other_socket.exists(), "the second daemon never listened");
    assert_eq!(
        daemon.result(
            "transcripts.list",
            json!({"kinds": ["dictation"], "limit": 5})
        ),
        seeded,
        "the first daemon's rows are untouched"
    );
    daemon.stop();
}

#[test]
fn a_stale_socket_file_does_not_prevent_a_clean_start() {
    let tree = Tree::new();
    let socket = tree.socket();
    std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
    drop(UnixListener::bind(&socket).unwrap());
    assert!(socket.exists(), "stale file is in place");
    // The listener is closed, so a connect is refused; the test's claim is
    // the daemon's handling below, not the kernel's timing here.
    let daemon = Daemon::spawn(tree, &[]);
    assert_eq!(daemon.result("system.ping", json!({}))["ok"], true);
    assert!(daemon.log().contains("removing stale socket file"));
    daemon.stop();
}

#[test]
fn sigterm_stops_the_daemon_and_releases_socket_and_pid_file() {
    let daemon = Daemon::spawn(Tree::new(), &[]);
    let socket = daemon.tree.socket();
    let pid_file = daemon.tree.pid_file();
    assert!(socket.exists());
    assert_eq!(
        std::fs::read_to_string(&pid_file).unwrap().trim(),
        daemon.pid().to_string()
    );
    let started = Instant::now();
    let log = daemon.stop();
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "shutdown is bounded"
    );
    assert!(!socket.exists(), "socket released");
    assert!(!pid_file.exists(), "pid file released");
    assert!(log.contains("stopped"), "{log}");
}

/// Socket activation through `systemd-socket-activate`, which hands the
/// listener over exactly as the socket unit does. Readiness is measured
/// from the first connection attempt to the first answer and must stay
/// under 300 ms on the development machine (R1); a shared CI runner gets
/// a 1500 ms budget (`DETTIVO_QA_ACTIVATION_BUDGET_MS` overrides either)
/// and the measurement is printed. Skipped where the tool is absent;
/// `scripts/qa-activation.sh` covers the real units.
#[test]
fn socket_activation_answers_within_300ms() {
    let activate = match which("systemd-socket-activate") {
        Some(p) => p,
        None => {
            eprintln!("skip: systemd-socket-activate not installed");
            return;
        }
    };
    let tree = Tree::new();
    let socket = tree.socket();
    std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
    let cmd = tree.command();
    let envs: Vec<(String, String)> = cmd
        .get_envs()
        .filter_map(|(k, v)| Some((k.to_str()?.to_string(), v?.to_str()?.to_string())))
        .collect();
    let mut activator = Command::new(activate);
    activator
        .arg(format!("--listen={}", socket.display()))
        .arg("--")
        .arg(DAEMON)
        .envs(envs)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = activator.spawn().expect("systemd-socket-activate");
    // The activator binds before it waits; give it a moment to do so.
    let bound = Instant::now();
    while !socket.exists() && bound.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(socket.exists(), "activator did not bind");

    let start = Instant::now();
    let reply = {
        use std::io::{BufRead, BufReader, Write};
        let mut stream = UnixStream::connect(&socket).expect("connect to activated socket");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .write_all(br#"{"jsonrpc":"2.0","id":"1","method":"system.ping","params":{}}"#)
            .unwrap();
        stream.write_all(b"\n").unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        line
    };
    let elapsed = start.elapsed();
    assert!(reply.contains("\"ok\":true"), "{reply}");
    let budget_ms: u64 = std::env::var("DETTIVO_QA_ACTIVATION_BUDGET_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if std::env::var_os("CI").is_some() {
            1500
        } else {
            300
        });
    eprintln!("socket activation answered in {elapsed:?} (budget {budget_ms} ms)");
    assert!(
        elapsed < Duration::from_millis(budget_ms),
        "activation took {elapsed:?}, want under {budget_ms} ms"
    );
    let _ = child.kill();
    let _ = child.wait();
}

fn which(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join(name))
            .find(|p| p.is_file())
    })
}
