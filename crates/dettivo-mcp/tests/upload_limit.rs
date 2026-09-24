//! MCP uploads use the same encoded IPC bound as the other adapters.
mod common;
use common::*;
use dettivo_mcp::{client::Client, tools::transfer::import_audio};
use serde_json::json;
use std::time::Duration;

#[test]
fn a_reduced_daemon_line_limit_still_reaches_import_after_commit() {
    let tree = Tree::new();
    tree.write_config("[ipc]\nmax_line_bytes = 4096\n[hotkeys]\nbackend = \"none\"\n");
    let file = tree.root().join("audio.wav");
    std::fs::write(&file, vec![43; 300_000]).unwrap();
    let daemon = Daemon::spawn(tree, &[]);
    let client = Client {
        socket: daemon.tree.socket(),
        token: None,
        timeout: Duration::from_secs(5),
    };
    let error = import_audio(&client, &json!({"file_path":file, "mode":"enhanced"})).unwrap_err();
    assert!(
        error
            .text("import_audio")
            .contains("an import runs the raw layer"),
        "{}",
        error.text("import_audio")
    );
}
