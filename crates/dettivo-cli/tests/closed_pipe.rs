//! `dettivo ... | head` closes the pipe while the CLI still writes. The CLI
//! then ends quietly, as other Unix tools do, instead of panicking.

use std::process::{Command, Stdio};

#[test]
fn a_closed_stdout_ends_the_cli_without_a_panic() {
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    let output = Command::new(env!("CARGO_BIN_EXE_dettivo"))
        .args(["completions", "bash"])
        .stdout(writer)
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("panicked"), "stderr: {stderr}");
    assert_ne!(output.status.code(), Some(101), "stderr: {stderr}");
}
