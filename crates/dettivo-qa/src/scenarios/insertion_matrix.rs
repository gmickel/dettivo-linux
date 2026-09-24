//! The insertion matrix (QA-8, ADR 0007): every target this desktop can
//! reach receives the sample text through the real chain, the text is
//! read back, and `insertion-matrix.json` records backend, latency and
//! outcome per target. A target that is not installed or not reachable
//! in this session is recorded as `skipped` with the reason.

use crate::child::ChildOwner;

use std::collections::BTreeMap;
use std::process::Stdio;
use std::time::Duration;

use serde_json::json;

use super::support::{InsertDaemon, MatrixRow, SAMPLE, installed, wait_for_line};
use super::{Context, Scenario, binary};
use crate::driver::{Driver, Launch};

/// The accessible name of the target's text field.
pub const FIELD: &str = "Insert target";

/// The scenario.
pub struct InsertionMatrix;

impl Scenario for InsertionMatrix {
    fn id(&self) -> &'static str {
        "insertion_matrix"
    }

    fn summary(&self) -> &'static str {
        "text lands in every reachable target through the real chain; insertion-matrix.json"
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let daemon = InsertDaemon::start(ctx)?;
        ctx.timings.mark("daemon");
        let mut rows = vec![qt_target(driver, ctx, &daemon)];
        ctx.timings.mark("dettivo-insert-target");
        for terminal in TERMINALS {
            rows.push(terminal_target(ctx, &daemon, terminal));
            ctx.timings.mark(terminal.name);
        }
        rows.push(MatrixRow::skipped(
            "chromium",
            if installed("chromium") {
                "no read-back path in this pack (agent-browser drive pending)".into()
            } else {
                "chromium is not installed".into()
            },
        ));
        std::fs::write(
            ctx.evidence_dir.join("insertion-matrix.json"),
            serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("insertion-matrix.json".into());
        let failed: Vec<String> = rows
            .iter()
            .filter(|r| r.outcome == "fail")
            .map(|r| format!("{}: {}", r.target, r.reason.as_deref().unwrap_or("failed")))
            .collect();
        if failed.is_empty() {
            Ok(())
        } else {
            Err(failed.join("; "))
        }
    }
}

/// The Qt insert target: launched through the driver, the field clicked,
/// the text read back over the accessibility tree.
fn qt_target(driver: &mut dyn Driver, ctx: &mut Context<'_>, daemon: &InsertDaemon) -> MatrixRow {
    let name = "dettivo-insert-target";
    match drive_qt_target(driver, ctx, daemon) {
        Ok(row) => row,
        Err(e) => MatrixRow {
            target: name.into(),
            outcome: "fail".into(),
            backend: None,
            latency_ms: None,
            reason: Some(e),
        },
    }
}

fn drive_qt_target(
    driver: &mut dyn Driver,
    ctx: &mut Context<'_>,
    daemon: &InsertDaemon,
) -> Result<MatrixRow, String> {
    let program = binary(ctx.repo_root, "dettivo-insert-target")?;
    let env: BTreeMap<String, String> = ctx.profile.env();
    let app = driver
        .launch(
            &Launch {
                program,
                args: Vec::new(),
                env,
            },
            ctx.timeout,
        )
        .map_err(|e| format!("launch: {e}"))?;
    ctx.profile.track_pid("dettivo-insert-target", app.pid);
    let field = driver
        .wait_for_label(&app, FIELD, ctx.timeout)
        .map_err(|e| format!("field {FIELD:?}: {e}"))?;
    driver
        .click(&app, &field)
        .map_err(|e| format!("click: {e}"))?;
    let target = daemon.wait_focused(app.pid, ctx.timeout)?;
    let result = daemon.call(
        "insert.perform",
        json!({
            "mode": "raw",
            "text": SAMPLE,
            "expected_target_bundle_id": target["target"]["app_id"],
            "expected_target_pid": app.pid.to_string(),
        }),
    )?;
    std::thread::sleep(Duration::from_millis(300));
    let field = driver
        .find(&app, FIELD)
        .map_err(|e| format!("field after insertion: {e}"))?;
    let read_back = driver
        .read_value(&app, &field)
        .map_err(|e| format!("read back: {e}"));
    let shot = ctx.evidence_dir.join("dettivo-insert-target.png");
    if driver.screenshot(&app, &shot).is_ok() {
        ctx.evidence.push("dettivo-insert-target.png".into());
    }
    let _ = driver.close(&app);
    Ok(MatrixRow::from_result(
        "dettivo-insert-target",
        &result,
        read_back,
    ))
}

/// A terminal target: the emulator runs `cat` into a file, the daemon
/// types the sample plus Return, and the file is read back.
struct Terminal {
    name: &'static str,
    /// Arguments before the shell command.
    prefix: &'static [&'static str],
}

const TERMINALS: &[Terminal] = &[
    Terminal {
        name: "foot",
        prefix: &[],
    },
    Terminal {
        name: "ghostty",
        prefix: &["-e"],
    },
    Terminal {
        name: "alacritty",
        prefix: &["-e"],
    },
    Terminal {
        name: "kitty",
        prefix: &[],
    },
];

fn terminal_target(ctx: &mut Context<'_>, daemon: &InsertDaemon, terminal: &Terminal) -> MatrixRow {
    if !installed(terminal.name) {
        return MatrixRow::skipped(terminal.name, format!("{} is not installed", terminal.name));
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return MatrixRow::skipped(
            terminal.name,
            "needs a Wayland display (the CI drive runs under Xvfb)".into(),
        );
    }
    match drive_terminal(ctx, daemon, terminal) {
        Ok(row) => row,
        Err(e) => MatrixRow {
            target: terminal.name.into(),
            outcome: "fail".into(),
            backend: None,
            latency_ms: None,
            reason: Some(e),
        },
    }
}

fn drive_terminal(
    ctx: &mut Context<'_>,
    daemon: &InsertDaemon,
    terminal: &Terminal,
) -> Result<MatrixRow, String> {
    let capture = ctx.profile.root.join(format!("{}.txt", terminal.name));
    let script = format!("cat > '{}'", capture.display());
    let mut env: BTreeMap<String, String> = ctx.profile.env();
    env.remove("QT_QPA_PLATFORM");
    let child = crate::profile::command(terminal.name, &env)
        .args(terminal.prefix)
        .args(["sh", "-c", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(ChildOwner::new)
        .map_err(|e| format!("launch {}: {e}", terminal.name))?;
    let pid = child.id();
    ctx.profile.track(terminal.name, &child);
    let target = daemon.wait_focused(pid, ctx.timeout)?;
    let result = daemon.call(
        "insert.perform",
        json!({
            "mode": "raw",
            "text": format!("{SAMPLE}\n"),
            "expected_target_bundle_id": target["target"]["app_id"],
            "expected_target_pid": pid.to_string(),
        }),
    )?;
    let read_back = wait_for_line(&capture, Duration::from_secs(5));
    Ok(MatrixRow::from_result(terminal.name, &result, read_back))
}
