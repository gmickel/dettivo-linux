//! `dettivo insert`, `insert undo` and `insert target` against a daemon
//! whose insertion chain is the QA mock.

mod common;

use common::*;
use serde_json::Value;

#[test]
fn insert_commands_reach_the_daemon_and_validate_their_flags() {
    let d = Daemon::spawn(
        Tree::new(),
        &[("DETTIVO_QA_MODE", "1"), ("DETTIVO_MOCK_INSERT", "1")],
    );
    let out = d.cli(&[
        "insert",
        "--text",
        "Hello from the CLI",
        "--expected-target-pid",
        "4141",
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stdout(&out).starts_with("inserted into org.gnome.TextEditor via mock"),
        "{}",
        stdout(&out)
    );
    let written =
        std::fs::read_to_string(d.tree.root().join("state/dettivo/qa/inserted.txt")).unwrap();
    assert_eq!(written, "Hello from the CLI\n");

    let json: Value = serde_json::from_str(&stdout(&d.cli(&[
        "--json",
        "insert",
        "--mode",
        "clipboard_only",
        "--text",
        "copy",
    ])))
    .unwrap();
    assert_eq!(json["outcome"], "copied_to_clipboard");

    let conflict = d.cli(&["insert", "--text", "no", "--expected-target-pid", "1"]);
    assert_eq!(conflict.status.code(), Some(1), "{}", stderr(&conflict));
    assert!(
        stderr(&conflict).contains("CONFLICT"),
        "{}",
        stderr(&conflict)
    );

    let undo = d.cli(&["insert", "undo"]);
    assert_eq!(undo.status.code(), Some(0), "{}", stderr(&undo));
    assert_eq!(stdout(&undo), "not undone: unsupported_backend\n");

    let target = d.cli(&["insert", "target"]);
    assert_eq!(target.status.code(), Some(0), "{}", stderr(&target));
    assert!(
        stdout(&target).contains("target    org.gnome.TextEditor pid 4141 (probe mock)"),
        "{}",
        stdout(&target)
    );
    assert!(
        stdout(&target).contains("chosen    mock\n"),
        "{}",
        stdout(&target)
    );

    let bad = d.cli(&["insert"]);
    assert_eq!(bad.status.code(), Some(4), "{}", stderr(&bad));
    let bad_mode = d.cli(&["insert", "--mode", "loud", "--text", "x"]);
    assert_eq!(bad_mode.status.code(), Some(4));
    let missing = d.cli(&[
        "insert",
        "--kind",
        "dictation",
        "--id",
        "0f8fad5b-d9cb-469f-a165-70867728950e",
    ]);
    assert_eq!(missing.status.code(), Some(1), "{}", stderr(&missing));
    assert!(
        stderr(&missing).contains("NOT_FOUND"),
        "{}",
        stderr(&missing)
    );
}
