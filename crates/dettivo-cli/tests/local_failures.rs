//! Adapter errors keep stdout machine-readable, including parse failures.
mod common;

use common::*;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::time::Duration;

fn assert_failure(output: &std::process::Output, exit: i32) {
    assert_eq!(output.status.code(), Some(exit), "{}", stderr(output));
    assert!(!stderr(output).is_empty());
    let body: serde_json::Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("JSON stdout: {e}; stderr={}", stderr(output)));
    assert_eq!(body["error"]["code"], -32603);
    assert_eq!(body["error"]["data"]["origin"], "cli");
    assert_eq!(body["error"]["data"]["exit_code"], exit);
    assert!(!body["error"]["message"].as_str().unwrap().is_empty());
}

#[test]
fn locally_invalid_arguments_have_json_errors() {
    let output = cli_in(&Tree::new(), &["--json", "call", "system.ping", "[]"], &[]);
    assert_failure(&output, 4);
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/local-errors.json")).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        expected
    );
}

#[test]
fn clap_failures_have_json_errors_and_quiet_wins() {
    for args in [
        vec!["--json", "unknown-command"],
        vec!["--json", "--timeout-ms", "no", "status", "ping"],
        vec!["status", "--json"],
    ] {
        assert_failure(&cli_in(&Tree::new(), &args, &[]), 4);
        let mut quiet = args;
        quiet.insert(0, "--quiet");
        let output = cli_in(&Tree::new(), &quiet, &[]);
        assert_eq!(output.status.code(), Some(4));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn unavailable_socket_has_a_json_error() {
    assert_failure(&cli_in(&Tree::new(), &["--json", "status", "ping"], &[]), 2);
}

#[test]
fn local_file_failure_has_a_json_error() {
    assert_failure(
        &cli_in(
            &Tree::new(),
            &["--json", "history", "import", "/nonexistent/file.wav"],
            &[],
        ),
        4,
    );
}

#[test]
fn local_timeout_has_a_json_error() {
    let tree = Tree::new();
    std::fs::create_dir_all(tree.socket().parent().unwrap()).unwrap();
    let listener = UnixListener::bind(tree.socket()).unwrap();
    let thread = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        reader.read_line(&mut String::new()).unwrap();
        std::thread::sleep(Duration::from_millis(150));
    });
    let output = cli_in(
        &tree,
        &["--json", "--timeout-ms", "20", "status", "ping"],
        &[],
    );
    thread.join().unwrap();
    assert_failure(&output, 5);
}

#[test]
fn mcp_serve_keeps_its_transport_cap_even_with_the_global_json_flag() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let tree = Tree::new();
    tree.write_config("[mcp]\nmax_message_bytes = 64\n[hotkeys]\nbackend = \"none\"\n");
    let daemon = Daemon::spawn(tree, &[]);
    let mut command = Command::new(CLI);
    daemon.tree.env(&mut command);
    let mut child = command
        .args(["--json", "mcp", "serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("max_message_bytes"));
    assert!(
        output.stdout.is_empty(),
        "MCP cap bypassed: {}",
        stdout(&output)
    );
}
