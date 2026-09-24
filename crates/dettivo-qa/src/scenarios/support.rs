//! What the insertion scenarios share: a real (not mocked) daemon in the
//! scenario's profile, the JSON-RPC calls, compositor focus for a pid, and
//! the matrix row every target produces.

use crate::child::ChildOwner;

use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Context, binary};

/// The text every target receives: ASCII, an accent, CJK and digits, so
/// the keymap path is exercised beyond the US layout.
pub const SAMPLE: &str = "Dettivo café 日本語 42";

/// A daemon running the real insertion chain inside the profile.
pub struct InsertDaemon {
    _child: Option<ChildOwner>,
    /// The socket.
    pub socket: PathBuf,
}

impl InsertDaemon {
    /// A socket view when the bench already owns the daemon's process guard.
    pub(crate) fn borrowed(daemon: &super::daemon::DaemonHandle) -> Self {
        Self {
            socket: daemon.socket.clone(),
            _child: None,
        }
    }

    /// Starts `dettivod` with the profile's isolation but without the QA
    /// mock switches, so the chain is the real one; the log goes to
    /// `daemon.log` in the evidence directory.
    pub fn start(ctx: &mut Context<'_>) -> Result<Self, String> {
        Self::start_with(ctx, &BTreeMap::new())
    }

    /// `start` plus `extra` variables (a mock microphone for a scenario
    /// that dictates through the real chain).
    pub fn start_with(
        ctx: &mut Context<'_>,
        extra: &BTreeMap<String, String>,
    ) -> Result<Self, String> {
        let program = binary(ctx.repo_root, "dettivod")?;
        let mut env: BTreeMap<String, String> = ctx.profile.env();
        env.remove("DETTIVO_MOCK_MODE");
        env.remove("DETTIVO_MOCK_INSERT");
        env.insert("RUST_LOG".into(), "info".into());
        env.extend(extra.clone());
        let log = std::fs::File::create(ctx.evidence_dir.join("daemon.log"))
            .map_err(|e| format!("daemon.log: {e}"))?;
        ctx.evidence.push("daemon.log".into());
        let child = crate::profile::command(&program, &env)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .map(ChildOwner::new)
            .map_err(|e| format!("start dettivod: {e}"))?;
        ctx.profile.track("dettivod", &child);
        let socket = ctx.profile.socket();
        let deadline = Instant::now() + ctx.timeout;
        while UnixStream::connect(&socket).is_err() {
            if Instant::now() > deadline {
                return Err(format!("dettivod did not open {}", socket.display()));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(Self {
            socket,
            _child: Some(child),
        })
    }

    /// One request; an error response is returned as `Err` with its message.
    pub fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let line = json!({"jsonrpc": "2.0", "id": "qa", "method": method, "params": params});
        let answer = crate::replay::call(&self.socket, &line.to_string())
            .map_err(|e| format!("{method}: {e}"))?;
        if let Some(error) = answer.get("error") {
            return Err(format!(
                "{method}: {} ({})",
                error["message"].as_str().unwrap_or("error"),
                error["data"]["app_code"].as_str().unwrap_or("?")
            ));
        }
        Ok(answer["result"].clone())
    }

    /// Asks the compositor to focus `pid` until the daemon sees it as the
    /// focused window, and returns the target block.
    pub fn wait_focused(&self, pid: u32, timeout: Duration) -> Result<Value, String> {
        let deadline = Instant::now() + timeout;
        loop {
            focus_pid(pid);
            let last = self.call("insert.target", json!({}))?;
            if last["target"]["pid"].as_u64() == Some(u64::from(pid)) {
                return Ok(last);
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "pid {pid} never became the focused window; insert.target says {last}"
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// Asks the compositor to focus the window of `pid`. Hyprland answers its
/// IPC socket; under an X11 window manager the driver's click already
/// moved focus, so this is a no-op there.
pub fn focus_pid(pid: u32) {
    let Some(signature) = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE") else {
        focus_pid_x11(pid);
        return;
    };
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let socket = runtime.join("hypr").join(&signature).join(".socket.sock");
    // Hyprland 0.56 reads dispatches as Lua (`hl.dsp.focus({ window = ... })`);
    // an older Hyprland answers that with an error and takes the classic
    // `focuswindow` form instead.
    let forms = [
        format!("dispatch hl.dsp.focus({{ window = \"pid:{pid}\" }})"),
        format!("dispatch focuswindow pid:{pid}"),
    ];
    for form in forms {
        let Ok(mut stream) = UnixStream::connect(&socket) else {
            return;
        };
        let _ = stream.write_all(form.as_bytes());
        let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
        let mut answer = String::new();
        let _ = std::io::Read::read_to_string(&mut stream, &mut answer);
        if !answer.trim_start().starts_with("error") {
            return;
        }
    }
}

/// Under a plain X server (Xvfb in CI) the window manager's first focus
/// is not guaranteed, so the window is activated through `xdotool` when
/// it is installed; without it the manager's own choice stands.
fn focus_pid_x11(pid: u32) {
    if std::env::var_os("DISPLAY").is_none() || !installed("xdotool") {
        return;
    }
    let found = std::process::Command::new("xdotool")
        .args(["search", "--onlyvisible", "--pid", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .output();
    let Ok(found) = found else { return };
    let Some(window) = String::from_utf8_lossy(&found.stdout)
        .lines()
        .next()
        .map(str::to_string)
    else {
        return;
    };
    let _ = std::process::Command::new("xdotool")
        .args(["windowactivate", "--sync", &window])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    // Without a window manager the activation request has no taker;
    // setting the input focus directly still makes the window the one the
    // probe reports.
    let _ = std::process::Command::new("xdotool")
        .args(["windowfocus", "--sync", &window])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// One row of `insertion-matrix.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatrixRow {
    /// The target's name (`dettivo-insert-target`, `foot`, ...).
    pub target: String,
    /// `pass`, `fail` or `skipped`.
    pub outcome: String,
    /// The backend that inserted, when one ran.
    pub backend: Option<String>,
    /// The backend's latency in milliseconds, when one ran.
    pub latency_ms: Option<u64>,
    /// Why a row failed or was skipped.
    pub reason: Option<String>,
}

impl MatrixRow {
    /// A target that is not installed or not reachable in this session.
    pub fn skipped(target: &str, reason: String) -> Self {
        Self {
            target: target.to_string(),
            outcome: "skipped".into(),
            backend: None,
            latency_ms: None,
            reason: Some(reason),
        }
    }

    /// A target whose read-back is compared with what was sent.
    pub fn from_result(target: &str, result: &Value, read_back: Result<String, String>) -> Self {
        let backend = result["backend"]["name"].as_str().map(str::to_string);
        let latency_ms = result["backend"]["latency_ms"].as_u64();
        let outcome = result["outcome"].as_str().unwrap_or("failed");
        let (verdict, reason) = match read_back {
            Ok(text) if outcome == "inserted" && text.trim_end() == SAMPLE => ("pass", None),
            Ok(text) if outcome == "inserted" => (
                "fail",
                Some(format!(
                    "read back {:?} ({} characters), expected {SAMPLE:?} ({})",
                    text.trim_end(),
                    text.chars().count(),
                    SAMPLE.chars().count()
                )),
            ),
            Ok(_) => (
                "fail",
                Some(format!(
                    "outcome {outcome}: {}",
                    result["reason"].as_str().unwrap_or("no reason")
                )),
            ),
            Err(e) => ("fail", Some(e)),
        };
        Self {
            target: target.to_string(),
            outcome: verdict.into(),
            backend,
            latency_ms,
            reason,
        }
    }
}

/// Whether a program is on `PATH`.
pub fn installed(program: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(program).is_file()))
        .unwrap_or(false)
}

/// Waits until `path` holds a line, returning it.
pub fn wait_for_line(path: &std::path::Path, timeout: Duration) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(text) = std::fs::read_to_string(path) {
            if text.contains('\n') {
                return Ok(text.lines().next().unwrap_or("").to_string());
            }
        }
        if Instant::now() > deadline {
            return Err(format!("{} received no line", path.display()));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(unsafe_code)] // Reap only the test's direct child when reproducing a broken guard.
    fn insertion_daemon_is_reaped_after_normal_and_failed_startup() {
        use std::os::unix::fs::PermissionsExt;
        let mut leaked = Vec::new();
        for ready in [false, true] {
            let repo = tempfile::tempdir().unwrap();
            let bin = repo
                .path()
                .join("target")
                .join(super::super::build_profile());
            std::fs::create_dir_all(&bin).unwrap();
            let program = bin.join("dettivod");
            std::fs::write(
                &program,
                r#"#!/usr/bin/python3
import os, socket, time
with open(os.environ['QA_TEST_PID'], 'w') as file:
    file.write(str(os.getpid()))
if os.environ['QA_TEST_READY'] == 'true':
    listener = socket.socket(socket.AF_UNIX)
    listener.bind(os.environ['DETTIVO_IPC_SOCKET'])
    listener.listen()
time.sleep(30)
"#,
            )
            .unwrap();
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
            let mut profile = crate::profile::Profile::create("insertowner", None).unwrap();
            let pid_file = profile.root.join("pid");
            let mut timings = crate::evidence::Timings::start();
            let mut evidence = Vec::new();
            let mut ctx = Context {
                profile: &mut profile,
                evidence_dir: repo.path(),
                repo_root: repo.path(),
                timings: &mut timings,
                evidence: &mut evidence,
                timeout: Duration::from_millis(300),
            };
            let result = InsertDaemon::start_with(
                &mut ctx,
                &BTreeMap::from([
                    ("QA_TEST_PID".into(), pid_file.display().to_string()),
                    ("QA_TEST_READY".into(), ready.to_string()),
                ]),
            );
            assert_eq!(result.is_ok(), ready);
            drop(result);
            let pid: u32 = std::fs::read_to_string(pid_file).unwrap().parse().unwrap();
            if std::path::Path::new(&format!("/proc/{pid}")).exists() {
                leaked.push((ready, pid));
                let _ = std::process::Command::new("kill")
                    .args(["-KILL", &pid.to_string()])
                    .status();
                unsafe extern "C" {
                    fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
                }
                // The exact direct child created above, including the failing pre-fix case.
                unsafe {
                    waitpid(pid as i32, std::ptr::null_mut(), 0);
                }
            }
        }
        assert!(
            leaked.is_empty(),
            "insertion daemon survived normal/startup-error guard drop: {leaked:?}"
        );
    }

    #[test]
    fn rows_compare_the_read_back_with_the_sample() {
        let result =
            json!({"outcome": "inserted", "backend": {"name": "xdotool", "latency_ms": 40}});
        let pass = MatrixRow::from_result("t", &result, Ok(format!("{SAMPLE}\n")));
        assert_eq!(pass.outcome, "pass");
        assert_eq!(pass.backend.as_deref(), Some("xdotool"));
        assert_eq!(pass.latency_ms, Some(40));
        let short = MatrixRow::from_result("t", &result, Ok("Dettivo".into()));
        assert_eq!(short.outcome, "fail");
        let failed = json!({"outcome": "failed", "reason": "target_is_self"});
        let row = MatrixRow::from_result("t", &failed, Ok(String::new()));
        assert_eq!(
            row.reason.as_deref(),
            Some("outcome failed: target_is_self")
        );
        assert_eq!(MatrixRow::skipped("foot", "x".into()).outcome, "skipped");
    }
}
