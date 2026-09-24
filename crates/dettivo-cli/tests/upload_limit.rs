//! CLI uploads respect the configured IPC limit as well as the raw chunk cap.
mod common;
use common::*;

#[test]
fn a_reduced_daemon_line_limit_still_reaches_import_after_commit() {
    let tree = Tree::new();
    tree.write_config("[ipc]\nmax_line_bytes = 4096\n[hotkeys]\nbackend = \"none\"\n");
    let file = tree.root().join("audio.wav");
    std::fs::write(&file, vec![43; 300_000]).unwrap();
    let daemon = Daemon::spawn(tree, &[]);
    let output = daemon.cli(&[
        "--json",
        "history",
        "import",
        file.to_str().unwrap(),
        "--mode",
        "enhanced",
        "--no-wait",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["error"]["data"]["app_code"], "NOT_IMPLEMENTED");
    assert!(stderr(&output).contains("an import runs the raw layer"));
}
