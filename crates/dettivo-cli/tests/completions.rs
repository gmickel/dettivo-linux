//! `dettivo completions <shell>` prints the script the package installs
//! for bash, zsh and fish without touching the daemon, and `--version`
//! carries the package version and the commit (ADR 0034).

use std::process::Command;

const CLI: &str = env!("CARGO_BIN_EXE_dettivo");

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(CLI)
        .args(args)
        .env("DETTIVO_IPC_SOCKET", "/nonexistent/dettivo.sock")
        .output()
        .expect("run dettivo");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn the_three_shells_get_a_script_that_names_the_subcommands() {
    for (shell, marker) in [
        ("bash", "complete -F _dettivo"),
        ("zsh", "#compdef dettivo"),
        ("fish", "complete -c dettivo"),
    ] {
        let (code, stdout, stderr) = run(&["completions", shell]);
        assert_eq!(code, 0, "{shell}: {stderr}");
        assert!(
            stdout.contains(marker),
            "{shell}: no {marker:?} in the script"
        );
        for sub in ["doctor", "dictation", "completions"] {
            assert!(
                stdout.contains(sub),
                "{shell}: the script does not name {sub}"
            );
        }
    }
}

#[test]
fn an_unknown_shell_exits_with_usage_and_the_version_names_the_commit() {
    let (code, _, stderr) = run(&["completions", "powershell9"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("invalid value"), "{stderr}");

    let (code, stdout, _) = run(&["--version"]);
    assert_eq!(code, 0);
    let version = env!("CARGO_PKG_VERSION");
    assert!(
        stdout.starts_with(&format!("dettivo {version} (")) && stdout.trim_end().ends_with(')'),
        "{stdout:?}"
    );
}
