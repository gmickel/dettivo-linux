//! The settings drive's checks after the key loop (fn-26 R1, R3, R4,
//! R5): the refusal under a field, the hand edit that refreshes the open
//! route, the column's open action, the Agents host write and the
//! Diagnostics report, each leaving its evidence. Each check belongs to
//! one section, so a drive over one section (the GUI pack's per-section
//! steps) runs its own checks and skips the rest.

use std::time::{Duration, Instant};

use serde_json::json;

use super::settings_roundtrip::Run;

/// What the checks record in `roundtrip.json`.
pub(super) struct Checks {
    pub refusal: String,
    pub hand_edit_seen: String,
    pub opened: String,
    pub host_file: String,
    pub doctor_lines: usize,
}

/// Runs the checks of every section driven (`only`, or all), on the
/// same app.
pub(super) fn after_keys(run: &mut Run<'_, '_>, only: Option<&str>) -> Result<Checks, String> {
    let wants = |section: &str| only.is_none_or(|s| s == section);
    let mut checks = Checks {
        refusal: String::new(),
        hand_edit_seen: String::new(),
        opened: String::new(),
        host_file: String::new(),
        doctor_lines: 0,
    };
    if wants("general") {
        checks.refusal = refusal(run)?;
        checks.opened = open_action(run)?;
    }
    if wants("hotkeys") {
        checks.hand_edit_seen = hand_edit(run)?;
    }
    if wants("agents") {
        checks.host_file = agents_write(run)?;
    }
    if wants("diagnostics") {
        checks.doctor_lines = diagnostics(run)?;
    }
    Ok(checks)
}

/// An invalid value: the daemon's message under the field, the file
/// untouched.
fn refusal(run: &mut Run<'_, '_>) -> Result<String, String> {
    run.open("general", "daemon.shutdown_timeout_ms")?;
    run.click_named("daemon.shutdown_timeout_ms")?;
    std::thread::sleep(Duration::from_millis(150));
    run.driver
        .type_text(run.app, "soon\n")
        .map_err(|e| e.to_string())?;
    let refusal = run
        .driver
        .wait_for_label(
            run.app,
            "daemon.shutdown_timeout_ms refused",
            run.ctx.timeout,
        )
        .map_err(|e| format!("the refusal under the field: {e}"))?;
    let refusal_text = run.driver.read_value(run.app, &refusal).unwrap_or_default();
    let (_, source) = run.get("daemon.shutdown_timeout_ms")?;
    if source == "file" {
        return Err("the refused value reached the file".into());
    }
    run.capture("refused")?;
    run.ctx.timings.mark("refused");
    Ok(refusal_text)
}

/// A hand edit: the open route refreshes and says so.
fn hand_edit(run: &mut Run<'_, '_>) -> Result<String, String> {
    let text = std::fs::read_to_string(run.config_file()).map_err(|e| e.to_string())?;
    let edited = text.replacen("[hotkeys]\n", "[hotkeys]\nhold = \"F7\"\n", 1);
    if edited == text {
        return Err("config.toml lost its [hotkeys] table".into());
    }
    run.open("hotkeys", "hotkeys.hold")?;
    std::fs::write(run.config_file(), edited).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + run.ctx.timeout;
    let hand_edit_seen = loop {
        let tree = run.snapshot()?;
        let field_shows = tree
            .iter()
            .any(|e| e.name == "hotkeys.hold" && e.value.as_deref() == Some("F7"));
        let notice = tree
            .iter()
            .find(|e| e.name.starts_with("The file changed on disk"))
            .map(|e| e.name.clone());
        if field_shows {
            break notice.unwrap_or_else(|| "field refreshed".into());
        }
        if Instant::now() > deadline {
            return Err("the hotkeys.hold field never showed the hand-edited F7".into());
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    run.capture("hand-edit")?;
    run.daemon
        .call("config.unset", json!({"key": "hotkeys.hold"}))?;
    run.ctx.timings.mark("hand-edit");
    Ok(hand_edit_seen)
}

/// The column's action hands the file to the desktop.
fn open_action(run: &mut Run<'_, '_>) -> Result<String, String> {
    run.click_named("Open config.toml")?;
    let deadline = Instant::now() + run.ctx.timeout;
    loop {
        let text = std::fs::read_to_string(&run.opened).unwrap_or_default();
        if text.contains("config.toml") {
            return Ok(text.trim().to_string());
        }
        if Instant::now() > deadline {
            return Err("Open config.toml never reached xdg-open".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Agents: a host entry through `dettivo mcp config --write`.
fn agents_write(run: &mut Run<'_, '_>) -> Result<String, String> {
    // A fresh open lands at the top of the route where the host rows are;
    // the section's own key loop left the same route scrolled to its
    // last key, and a click there never reaches the hosts.
    run.open("general", "")?;
    run.open("agents", "")?;
    let host_file = run.ctx.profile.root.join("home/.mcp.json");
    let written = || {
        std::fs::read_to_string(&host_file)
            .map(|t| t.contains("dettivo"))
            .unwrap_or(false)
    };
    // The route's model re-reads every host after a key of its own
    // section changed and its actions are disabled while it does, so a
    // click that lands in that window is asked again after a pause.
    let deadline = Instant::now() + run.ctx.timeout;
    loop {
        run.click_named("Write Claude Code config")?;
        let pause = Instant::now() + Duration::from_secs(3);
        while !written() && Instant::now() < pause {
            std::thread::sleep(Duration::from_millis(100));
        }
        if written() {
            break;
        }
        if Instant::now() > deadline {
            run.capture("agents-not-written")?;
            return Err(format!("{} never gained the entry", host_file.display()));
        }
    }
    run.driver
        .wait_for_label(run.app, "Claude Code state: configured", run.ctx.timeout)
        .map_err(|e| format!("the host row after the write: {e}"))?;
    run.capture("agents-written")?;
    run.ctx.timings.mark("agents");
    Ok(host_file.to_string_lossy().into_owned())
}

/// Diagnostics: the doctor report with its copy action.
fn diagnostics(run: &mut Run<'_, '_>) -> Result<usize, String> {
    run.open("diagnostics", "")?;
    run.driver
        .wait_for_label(run.app, "Doctor report", run.ctx.timeout)
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + run.ctx.timeout;
    let doctor_lines = loop {
        let tree = run.snapshot()?;
        let lines = tree
            .iter()
            .filter(|e| e.role == "label" && e.name.starts_with("daemon"))
            .count();
        if lines > 0 {
            break tree.iter().filter(|e| e.role == "label").count();
        }
        if Instant::now() > deadline {
            return Err("the doctor report never named the daemon".into());
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    run.click_named("Copy report")?;
    run.capture("diagnostics-report")?;
    run.ctx.timings.mark("diagnostics");
    Ok(doctor_lines)
}
