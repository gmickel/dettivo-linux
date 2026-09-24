//! What `dettivo setup omarchy --check` and `dettivo doctor` report: the
//! socket unit, the snippet, the plugin's install and enabled state, the
//! shell and whether the panel holds its bus name. Read-only: nothing
//! here changes the machine.

use std::path::Path;

use dettivo_hotkeys::snippet::Compositor;
use serde_json::{Value, json};

use crate::client::SOCKET_UNIT;
use crate::omarchy::{PANEL_BUS_NAME, PLUGIN_ID, on_path, plugins_dir, run_command};
use crate::setup;

/// The plugin's state in the shell: installed, enabled, its version.
pub fn plugin_facts(config_home: &Path) -> Value {
    let dir = plugins_dir(config_home).join(PLUGIN_ID);
    let manifest = std::fs::read_to_string(dir.join("manifest.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let enabled = if on_path("omarchy") {
        let list = run_command("omarchy", &["plugin", "list", "--json"]);
        serde_json::from_str::<Value>(&list.stdout)
            .ok()
            .and_then(|rows| {
                rows.as_array()?
                    .iter()
                    .find(|r| r["id"] == PLUGIN_ID)
                    .map(|r| r["enabled"] == Value::Bool(true))
            })
            .map(Value::Bool)
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    json!({
        "id": PLUGIN_ID,
        "path": dir.to_string_lossy(),
        "installed": manifest.is_some(),
        "git": dir.join(".git").exists(),
        "version": manifest.as_ref().map(|m| m["version"].clone()).unwrap_or(Value::Null),
        "enabled": enabled,
    })
}

/// Whether the panel holds its bus name right now.
pub fn panel_on_bus() -> Option<bool> {
    let connection = zbus::blocking::Connection::session().ok()?;
    let proxy = zbus::blocking::fdo::DBusProxy::new(&connection).ok()?;
    let name = zbus::names::BusName::try_from(PANEL_BUS_NAME).ok()?;
    proxy.name_has_owner(name).ok()
}

/// What `--check` and `doctor` report: every step's state.
pub fn check(config_home: &Path) -> Value {
    let systemd = on_path("systemctl");
    let socket = if systemd {
        let enabled = run_command("systemctl", &["--user", "is-enabled", SOCKET_UNIT]);
        let active = run_command("systemctl", &["--user", "is-active", SOCKET_UNIT]);
        json!({
            "state": if enabled.stdout == "enabled" && active.stdout == "active" { "ok" } else { "failed" },
            "enabled": enabled.stdout,
            "active": active.stdout,
        })
    } else {
        json!({ "state": "skipped", "detail": "systemctl is not on PATH" })
    };
    let snippet = setup::check(Compositor::HyprlandLua, config_home);
    let snippet_state = if snippet["main_config"]["sourced"] == Value::Bool(true) {
        "ok"
    } else {
        "failed"
    };
    let shell_present = on_path("omarchy-shell");
    let shell_running = shell_present && run_command("omarchy-shell", &["shell", "ping"]).ok();
    let plugin = plugin_facts(config_home);
    let plugin_state = if !on_path("omarchy") {
        "skipped"
    } else if plugin["installed"] == Value::Bool(true) && plugin["enabled"] == Value::Bool(true) {
        "ok"
    } else {
        "failed"
    };
    let mut report = json!({
        "socket": socket,
        "snippet": snippet,
        "plugin": plugin,
        "shell": { "present": shell_present, "running": shell_running },
        "panel": { "name": PANEL_BUS_NAME, "on_bus": panel_on_bus() },
    });
    report["snippet"]["state"] = json!(snippet_state);
    report["plugin"]["state"] = json!(plugin_state);
    // Always present, so the report has one shape with or without Omarchy.
    report["plugin"]["detail"] = if plugin_state == "skipped" {
        json!("the omarchy command is not on PATH")
    } else {
        Value::Null
    };
    report
}

/// The `omarchy` line of `dettivo doctor`.
pub fn doctor_line(report: &Value) -> String {
    if report["shell"]["present"] != Value::Bool(true) {
        return "omarchy   shell absent (plain Hyprland: dettivo-osd.service hosts the pill)\n"
            .into();
    }
    let plugin = &report["plugin"];
    let plugin_text = if plugin["installed"] == Value::Bool(true) {
        format!(
            "plugin {} {}",
            plugin["version"].as_str().unwrap_or("?"),
            match plugin["enabled"] {
                Value::Bool(true) => "enabled",
                Value::Bool(false) => "disabled (omarchy plugin enable gmickel.dettivo)",
                _ => "state unknown",
            }
        )
    } else {
        "plugin not installed (run `dettivo setup omarchy`)".to_string()
    };
    format!(
        "omarchy   shell {}, {plugin_text}, panel {} the bus\n",
        if report["shell"]["running"] == Value::Bool(true) {
            "running"
        } else {
            "not running"
        },
        if report["panel"]["on_bus"] == Value::Bool(true) {
            "on"
        } else {
            "not on"
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_doctor_line_names_what_is_missing() {
        let absent = json!({ "shell": { "present": false } });
        assert!(doctor_line(&absent).contains("shell absent"));
        let report = json!({
            "shell": { "present": true, "running": true },
            "plugin": { "installed": true, "version": "0.1.0", "enabled": false },
            "panel": { "on_bus": false },
        });
        let line = doctor_line(&report);
        assert!(line.contains("plugin 0.1.0 disabled (omarchy plugin enable"));
        assert!(line.contains("panel not on the bus"));
    }
}
