//! R5 at the process level: the daemon refuses an unknown QA variable or a
//! bad value with the name printed and exit status 2, and honours QA mode
//! when it is on.

mod common;

use std::process::Stdio;

use common::{Daemon, Tree};
use serde_json::json;

#[test]
fn unknown_qa_variables_are_refused_by_name_with_exit_2() {
    for (var, value) in [
        ("DETTIVO_MOCK_BOGUS", "1"),
        ("DETTIVO_E2E_JUMP", "1"),
        ("DETTIVO_E2E_OPEN", "garage"),
        ("DETTIVO_MOCK_MIC", "/nonexistent/mic.wav"),
    ] {
        let tree = Tree::new();
        let output = tree
            .command()
            .env("DETTIVO_QA_MODE", "1")
            .env(var, value)
            .stderr(Stdio::piped())
            .output()
            .expect("run dettivod");
        assert_eq!(output.status.code(), Some(2), "{var}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(var), "{var}: {stderr}");
    }
}

#[test]
fn a_hook_without_qa_mode_is_refused() {
    let tree = Tree::new();
    let output = tree
        .command()
        .env("DETTIVO_E2E_SEED", "1")
        .stderr(Stdio::piped())
        .output()
        .expect("run dettivod");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("DETTIVO_E2E_SEED") && stderr.contains("DETTIVO_QA_MODE"),
        "{stderr}"
    );
}

#[test]
fn qa_mode_on_starts_and_is_logged() {
    let tree = Tree::new();
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MODE", "1"),
            ("DETTIVO_FORCE_CPU", "1"),
            ("RUST_LOG", "info"),
        ],
    );
    assert_eq!(daemon.result("system.ping", json!({}))["ok"], true);
    let log = daemon.stop();
    assert!(log.contains("QA mode on"), "{log}");
    assert!(log.contains("force_cpu=true"), "{log}");
}
