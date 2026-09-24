//! Smoke test: the binary answers `--version` with its name.

use std::process::Command;

#[test]
fn version_flag_prints_name() {
    let output = Command::new(env!("CARGO_BIN_EXE_dettivo-engine-whisper"))
        .arg("--version")
        .output()
        .expect("binary runs");
    assert!(output.status.success(), "exit status: {}", output.status);
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    assert!(
        stdout.contains("dettivo-engine-whisper"),
        "stdout: {stdout:?}"
    );
}
