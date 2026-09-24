//! A key the environment sets is shown as such (fn-35 R3, ADR 0033): the
//! profile's daemon runs with `DETTIVO_DATA_DIR` set, the General route
//! shows the `env · DETTIVO_DATA_DIR` badge beside `paths.data_dir`, the
//! control is disabled so typing into it changes nothing, and
//! `config.get` keeps answering with source `environment`; the Agents
//! route does the same for `ipc.socket`, which every drive's daemon gets
//! through `DETTIVO_IPC_SOCKET`.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::{Value, json};

use super::app_support::{app_command, app_env, capture, close, daemon_config, wait_status};
use super::daemon::DaemonHandle;
use super::{Context, Scenario, binary};
use crate::driver::{App, Driver};

/// The keys the drive locks, with the variable, the route and what is
/// typed at the disabled control.
const LOCKED: &[(&str, &str, &str)] = &[
    ("paths.data_dir", "DETTIVO_DATA_DIR", "general"),
    ("ipc.socket", "DETTIVO_IPC_SOCKET", "agents"),
];

/// The scenario.
pub struct SettingsEnvOverride;

impl Scenario for SettingsEnvOverride {
    fn id(&self) -> &'static str {
        "settings_env_override"
    }

    fn summary(&self) -> &'static str {
        "a key set through the environment shows its source badge and a disabled control naming the variable"
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        binary(ctx.repo_root, "dettivo-app")?;
        binary(ctx.repo_root, "dettivod").map(|_| ())
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let app_binary = binary(ctx.repo_root, "dettivo-app")?;
        let data_dir = ctx.profile.root.join("data/dettivo");
        let extra = BTreeMap::from([(
            "DETTIVO_DATA_DIR".to_string(),
            data_dir.to_string_lossy().into_owned(),
        )]);
        let daemon = DaemonHandle::spawn(
            &dettivod,
            ctx.profile,
            &daemon_config(ctx.repo_root),
            &extra,
            ctx.timeout,
        )?;
        ctx.timings.mark("daemon");
        let app = driver
            .launch(
                &crate::driver::Launch {
                    program: app_binary,
                    args: Vec::new(),
                    env: app_env(ctx, "settings", Some("general")),
                },
                ctx.timeout,
            )
            .map_err(|e| format!("launch dettivo-app: {e}"))?;
        ctx.profile.track_pid("dettivo-app", app.pid);
        let result = Self::drive(driver, ctx, &app, &daemon);
        close(driver, &app)?;
        result
    }
}

impl SettingsEnvOverride {
    fn get(daemon: &DaemonHandle, key: &str) -> Result<(Value, String), String> {
        let answer = daemon.call("config.get", json!({"key": key}))?;
        let entry = answer["entries"]
            .as_array()
            .and_then(|e| e.first())
            .ok_or_else(|| format!("config.get {key} answered no entry"))?;
        Ok((
            entry["value"].clone(),
            entry["source"].as_str().unwrap_or_default().to_string(),
        ))
    }

    fn drive(
        driver: &mut dyn Driver,
        ctx: &mut Context<'_>,
        app: &App,
        daemon: &DaemonHandle,
    ) -> Result<(), String> {
        let mut rows = Vec::new();
        for (key, variable, section) in LOCKED {
            let (before, source) = Self::get(daemon, key)?;
            if source != "environment" {
                return Err(format!(
                    "{key} comes from {source}; the daemon did not take {variable}"
                ));
            }
            app_command(
                ctx,
                &json!({"cmd": "open", "route": format!("settings.{section}"), "arg": key}),
            )?;
            app_command(ctx, &json!({"cmd": "raise"}))?;
            let _ = wait_status(ctx, Duration::from_secs(2), "an active window", |s| {
                s["active"] == json!(true)
            });
            let badge = format!("env · {variable}");
            driver
                .wait_for_label(app, &badge, ctx.timeout)
                .map_err(|e| format!("{key}: the badge {badge:?}: {e}"))?;
            let tree = capture(driver, app, ctx, &format!("env-{section}"))?;
            if !tree.iter().any(|e| e.name == badge) {
                return Err(format!("{key}: the badge {badge:?} is not in the tree"));
            }
            // Found after the capture: a driver may renumber the elements on
            // a snapshot, and a token from before it is not clickable.
            let control = driver
                .wait_for_label(app, key, ctx.timeout)
                .map_err(|e| format!("{key}: the control: {e}"))?;
            // The disabled control: a click and a keystroke change nothing,
            // the file gains nothing, and the daemon keeps the variable's
            // value with its source.
            let shown_before = driver.read_value(app, &control).unwrap_or_default();
            // A driver that finds no actionable element behind a disabled
            // control refuses the click, which is the control doing its job;
            // the keystrokes go to the window either way.
            let click = match driver.click(app, &control) {
                Ok(()) => "clicked".to_string(),
                Err(e) => format!("refused: {e}"),
            };
            std::thread::sleep(Duration::from_millis(150));
            driver
                .type_text(app, "qa-typed\n")
                .map_err(|e| format!("{key}: type: {e}"))?;
            std::thread::sleep(Duration::from_millis(800));
            let (after, source_after) = Self::get(daemon, key)?;
            if source_after != "environment" || after != before {
                return Err(format!(
                    "{key} changed through a control the environment locks: {after} from {source_after}"
                ));
            }
            let fresh = driver
                .wait_for_label(app, key, ctx.timeout)
                .map_err(|e| format!("{key}: the control after typing: {e}"))?;
            let shown_after = driver.read_value(app, &fresh).unwrap_or_default();
            // A driver that reached the control reads it back; one that
            // found only its label reads the label's text, which is not
            // the field.
            if control.bounds.is_some() && shown_after.contains("qa-typed") {
                return Err(format!(
                    "{key}: the control took the keystrokes ({shown_after:?}) although {variable} locks it"
                ));
            }
            let file = std::fs::read_to_string(ctx.profile.root.join("cfg/dettivo/config.toml"))
                .unwrap_or_default();
            if file.contains("qa-typed") {
                return Err(format!("{key}: the typed text reached config.toml"));
            }
            rows.push(json!({
                "key": key,
                "variable": variable,
                "route": section,
                "badge": badge,
                "control_role": control.role,
                "click": click,
                "value": before,
                "shown_before": shown_before,
                "shown_after": shown_after,
            }));
            ctx.timings.mark(key);
        }
        std::fs::write(
            ctx.evidence_dir.join("env-override.json"),
            serde_json::to_string_pretty(&json!({"keys": rows})).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("env-override.json".into());
        Ok(())
    }
}
