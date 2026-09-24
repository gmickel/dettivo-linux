//! Machine-local checks and stable doctor JSON mapped from daemon diagnostics.
//! Required probes must succeed; unknown or failed probes make the report fail.

use std::cell::RefCell;
use std::process::Command;

use serde_json::{Map, Value, json};

use crate::client::{Client, SOCKET_UNIT};
use crate::exit::{Exit, Failure};
use crate::{Cli, output};

/// Runs the report.
pub fn run(cli: &Cli, client: &Client) -> Result<(), Failure> {
    let mut report = Map::new();
    let mut healthy = true;

    let diagnostics = client.call("system.diagnostics", json!({}));
    let failed_probes = RefCell::new(Map::new());
    let probe = |method: &str| -> Result<Value, Failure> {
        let probe = match &diagnostics {
            Ok(snapshot) => snapshot["probes"].get(method).cloned().unwrap_or_else(|| {
                json!({"state": "unknown", "result": null, "error": {"message": "probe missing from diagnostics"}})
            }),
            Err(failure) => json!({"state": "unknown", "result": null, "error": {
                "message": failure.message, "exit_code": failure.exit.number(), "rpc": failure.rpc,
            }}),
        };
        if probe["state"] == "success" && !probe["result"].is_null() {
            return Ok(probe["result"].clone());
        }
        failed_probes
            .borrow_mut()
            .insert(method.into(), probe.clone());
        let error = probe["error"].clone();
        Err(serde_json::from_value(error.clone())
            .map(Failure::rpc)
            .unwrap_or_else(|_| {
                Failure::new(
                    Exit::Failure,
                    error["message"].as_str().unwrap_or("probe outcome unknown"),
                )
            }))
    };

    let socket_exists = client.socket.exists();
    report.insert(
        "socket".into(),
        json!({
            "path": client.socket.to_string_lossy(),
            "exists": socket_exists,
            "override": std::env::var_os("DETTIVO_IPC_SOCKET").is_some() || cli.socket.is_some(),
        }),
    );
    report.insert("service".into(), service_facts());
    let session = session_facts(|k| std::env::var(k).ok());

    let daemon = match probe("system.version") {
        Ok(version) => {
            let health = probe("system.health").unwrap_or(Value::Null);
            let caps = probe("system.capabilities").unwrap_or(Value::Null);
            if health.get("ok") != Some(&Value::Bool(true)) {
                healthy = false;
            }
            let platform = caps.get("platform").cloned().unwrap_or(Value::Null);
            report.insert(
                "compositor".into(),
                json!({ "name": platform["compositor"], "source": "daemon" }),
            );
            report.insert("session_type".into(), platform["session"].clone());
            report.insert(
                "tier".into(),
                json!({ "tier": platform["tier"], "reason": platform["tier_reason"] }),
            );
            report.insert(
                "auth".into(),
                caps.get("auth").cloned().unwrap_or(Value::Null),
            );
            let mut rest = caps.get("rest").cloned().unwrap_or(Value::Null);
            if let Some(obj) = rest.as_object_mut() {
                let source = if cli.token.is_some() || cli.token_file.is_some() {
                    Some("flag".to_string())
                } else {
                    dettivo_rest::auth::resolve_token().map(|(_, o)| o.as_str().to_string())
                };
                obj.insert("token_source".into(), json!(source));
                obj.insert(
                    "token_required".into(),
                    caps["auth"]["rest_token_required"].clone(),
                );
            }
            report.insert("rest".into(), rest);
            report.insert(
                "insertion".into(),
                probe("insert.target").unwrap_or(Value::Null),
            );
            let hotkeys = probe("hotkeys.status").unwrap_or(Value::Null);
            let home = crate::setup::config_home(|k| std::env::var_os(k));
            let snippet = crate::setup::detect(platform["compositor"].as_str(), &home)
                .map(|c| crate::setup::check(c, &home))
                .unwrap_or(json!({ "compositor": null }));
            report.insert("hotkeys".into(), hotkeys);
            report.insert("snippet".into(), snippet);
            report.insert("platform".into(), platform);
            report.insert("mcp".into(), mcp_facts(&caps));
            json!({ "reachable": true, "version": version, "health": health })
        }
        Err(f) => {
            healthy = false;
            for (key, value) in [
                (
                    "compositor",
                    json!({ "name": session["compositor"], "source": "environment" }),
                ),
                ("session_type", session["session"].clone()),
                ("tier", json!({ "tier": null, "reason": null })),
                ("auth", Value::Null),
                ("rest", Value::Null),
                ("insertion", Value::Null),
                ("hotkeys", Value::Null),
                ("snippet", json!({ "compositor": null })),
                ("platform", Value::Null),
                ("mcp", Value::Null),
            ] {
                report.insert(key.into(), value);
            }
            json!({ "reachable": false, "error": f.message })
        }
    };
    report.insert("daemon".into(), daemon);
    report.insert("portals".into(), portal_facts());
    report.insert(
        "audio".into(),
        probe("audio.devices").unwrap_or(Value::Null),
    );

    let config = match probe("config.validate") {
        Ok(v) => {
            if v.get("ok") != Some(&Value::Bool(true)) {
                healthy = false;
            }
            v
        }
        Err(f) => json!({ "ok": Value::Null, "note": f.message }),
    };
    report.insert("config".into(), config);
    let engines = match probe("speech.engines") {
        Ok(v) => {
            let degraded = v["engines"]
                .as_array()
                .is_some_and(|rows| rows.iter().any(|r| r["degraded"] == Value::Bool(true)));
            if degraded {
                healthy = false;
            }
            v["engines"].clone()
        }
        Err(_) => Value::Null,
    };
    report.insert("engines".into(), engines);
    report.insert(
        "models".into(),
        probe("speech.models.status")
            .map(|v| models_facts(&v))
            .unwrap_or(Value::Null),
    );
    let llm = match probe("llm.providers.list") {
        Ok(providers) => {
            let engine = probe("llm.engine.status").unwrap_or(Value::Null);
            if engine["degraded"] == Value::Bool(true) {
                healthy = false;
            }
            let models = probe("llm.models.status").unwrap_or(Value::Null);
            llm_facts(&providers, engine, &models)
        }
        Err(_) => Value::Null,
    };
    report.insert("llm".into(), llm);
    let diarization = match probe("speech.models.status") {
        Ok(v) => {
            let model = v["models"]
                .as_array()
                .and_then(|rows| {
                    rows.iter()
                        .find(|row| row["provider"] == "diarize")
                        .cloned()
                })
                .unwrap_or(Value::Null);
            let engine = report["engines"]
                .as_array()
                .and_then(|rows| {
                    rows.iter()
                        .find(|r| r["binary"] == "dettivo-engine-diarize")
                        .cloned()
                })
                .unwrap_or(Value::Null);
            json!({
                "model": format!("{}/{}", model["provider"].as_str().unwrap_or("diarize"), model["id"].as_str().unwrap_or("diarization")),
                "model_readiness": model["readiness"],
                "engine": engine,
            })
        }
        Err(_) => Value::Null,
    };
    report.insert("diarization".into(), diarization);
    report.insert(
        "history".into(),
        probe("transcripts.stats").unwrap_or(Value::Null),
    );
    report.insert("osd".into(), crate::osd::doctor_facts(&client.socket));
    let home = crate::setup::config_home(|k| std::env::var_os(k));
    report.insert("omarchy".into(), crate::omarchy_check::check(&home));
    report.insert(
        "environment".into(),
        json!({
            "DETTIVO_CONFIG": std::env::var_os("DETTIVO_CONFIG").is_some(),
            "DETTIVO_IPC_SOCKET": std::env::var_os("DETTIVO_IPC_SOCKET").is_some(),
            "DETTIVO_IPC_TOKEN": std::env::var_os("DETTIVO_IPC_TOKEN").is_some(),
            "DETTIVO_DATA_DIR": std::env::var_os("DETTIVO_DATA_DIR").is_some(),
            "DETTIVO_QA": std::env::var_os("DETTIVO_QA").is_some(),
            "DETTIVO_FORCE_CPU": std::env::var_os("DETTIVO_FORCE_CPU").is_some(),
        }),
    );
    let failed_probes = failed_probes.into_inner();
    if !failed_probes.is_empty() {
        healthy = false;
        report.get_mut("daemon").unwrap()["probes"] = Value::Object(failed_probes);
    }
    let report = Value::Object(report);
    if cli.json {
        output::result(cli, "doctor", &report);
    } else if !cli.quiet {
        print!("{}", crate::doctor_report::human(&report));
    }
    if healthy {
        Ok(())
    } else {
        Err(Failure::reported(
            Exit::Failure,
            "doctor found problems (see report)",
        ))
    }
}

/// `systemctl --user` facts about the two units; the unit blocks are
/// present with `unknown` values when systemd is absent, so the shape
/// holds on every machine.
fn service_facts() -> Value {
    let available = Command::new("systemctl")
        .args(["--user", "--version"])
        .output()
        .is_ok_and(|o| o.status.success());
    let probe = |unit: &str, verb: &str| -> String {
        if !available {
            return "unknown".to_string();
        }
        Command::new("systemctl")
            .args(["--user", verb, unit])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown".to_string())
    };
    json!({
        "systemd": available,
        "socket_unit": { "name": SOCKET_UNIT, "enabled": probe(SOCKET_UNIT, "is-enabled"), "active": probe(SOCKET_UNIT, "is-active") },
        "service_unit": { "name": "dettivod.service", "active": probe("dettivod.service", "is-active") },
    })
}

/// The compositor and session type from this process's environment, for
/// the machine rows when the daemon is unreachable (the daemon's own
/// detection answers otherwise).
pub fn session_facts(env: impl Fn(&str) -> Option<String>) -> Value {
    let session = match env("XDG_SESSION_TYPE").as_deref() {
        Some("x11") => "x11",
        Some("wayland") => "wayland",
        _ if env("WAYLAND_DISPLAY").is_some() => "wayland",
        _ if env("DISPLAY").is_some() => "x11",
        _ => "wayland",
    };
    let compositor = if env("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        Some("Hyprland".to_string())
    } else {
        env("XDG_CURRENT_DESKTOP")
            .filter(|s| !s.is_empty())
            .map(|s| s.split(':').next().unwrap_or(&s).to_string())
    };
    json!({ "compositor": compositor, "session": session })
}

/// Desktop portal interfaces and the two required for insertion and hotkeys.
pub fn portal_facts() -> Value {
    let output = Command::new("busctl")
        .args([
            "--user",
            "introspect",
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
        ])
        .output();
    let (available, interfaces, reason): (bool, Vec<String>, Option<String>) = match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            (true, portal_interfaces(&text), None)
        }
        Ok(o) => (
            false,
            Vec::new(),
            Some(
                String::from_utf8_lossy(&o.stderr)
                    .trim()
                    .lines()
                    .next()
                    .unwrap_or("org.freedesktop.portal.Desktop does not answer")
                    .to_string(),
            ),
        ),
        Err(e) => (false, Vec::new(), Some(format!("busctl: {e}"))),
    };
    let has = |name: &str| interfaces.iter().any(|i| i == name);
    json!({
        "available": available,
        "reason": reason,
        "global_shortcuts": has("org.freedesktop.portal.GlobalShortcuts"),
        "remote_desktop": has("org.freedesktop.portal.RemoteDesktop"),
        "interfaces": interfaces,
    })
}

/// The portal interface names in `busctl introspect` output.
pub fn portal_interfaces(introspection: &str) -> Vec<String> {
    introspection
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .filter(|name| name.starts_with("org.freedesktop.portal."))
        .map(str::to_string)
        .collect()
}

/// Model selection and readiness per provider.
pub fn models_facts(status: &Value) -> Value {
    let rows = status["models"].as_array().cloned().unwrap_or_default();
    let count = |state: &str| rows.iter().filter(|r| r["readiness"] == state).count();
    let selected = rows
        .iter()
        .find(|r| r["is_selected"] == Value::Bool(true))
        .cloned()
        .unwrap_or(Value::Null);
    let mut providers: Vec<String> = rows
        .iter()
        .filter_map(|r| r["provider"].as_str().map(str::to_string))
        .collect();
    providers.sort();
    providers.dedup();
    let per_provider: Vec<Value> = providers
        .iter()
        .map(|p| {
            let mine: Vec<&Value> = rows.iter().filter(|r| r["provider"] == *p).collect();
            let of = |state: &str| mine.iter().filter(|r| r["readiness"] == state).count();
            json!({
                "provider": p,
                "total": mine.len(),
                "ready": of("ready"),
                "quarantined": of("quarantined"),
                "selected": mine.iter().find(|r| r["is_selected"] == Value::Bool(true)).map(|r| r["id"].clone()),
            })
        })
        .collect();
    json!({
        "dir": status["models_dir"],
        "catalogue_version": status["catalogue_version"],
        "selected": selected,
        "quarantined": count("quarantined"),
        "ready": count("ready"),
        "providers": per_provider,
    })
}

/// Language model selection, provider order and engine readiness.
pub fn llm_facts(providers: &Value, engine: Value, models: &Value) -> Value {
    let rows = providers["providers"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let local = rows
        .iter()
        .find(|p| p["id"] == "local")
        .cloned()
        .unwrap_or(Value::Null);
    let order: Vec<Value> = rows
        .iter()
        .map(|p| json!({ "id": p["id"], "available": p["available"], "model": p["model"] }))
        .collect();
    let selected = models["models"]
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .find(|m| m["is_selected"] == Value::Bool(true))
                .cloned()
        })
        .unwrap_or(Value::Null);
    // The sideloaded fine-tune's row (ADR 0032) while a manifest
    // is in force, so one that does not resolve is named here.
    let experiment = models["models"]
        .as_array()
        .and_then(|rows| rows.iter().find(|m| m["source"] == "sideload").cloned())
        .unwrap_or(Value::Null);
    json!({
        "selected_provider": providers["selected"],
        "provider_order": order,
        "local_available": local["available"],
        "local_detail": local["detail"],
        "local_hint": local["hint"],
        "model": selected["id"],
        "model_readiness": selected["readiness"],
        "experiment": experiment,
        "engine": engine,
    })
}

/// The MCP surface: the server this CLI carries and the tools it would
/// offer against these capabilities.
fn mcp_facts(caps: &Value) -> Value {
    let shown = dettivo_mcp::tools::available(Some(caps));
    json!({
        "server": { "name": dettivo_mcp::SERVER_NAME, "version": dettivo_mcp::SERVER_VERSION },
        "tools": shown.len(),
        "resource_templates": dettivo_mcp::resources::templates().len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_facts_follow_the_environment() {
        let hypr = |k: &str| match k {
            "HYPRLAND_INSTANCE_SIGNATURE" => Some("sig".to_string()),
            "XDG_SESSION_TYPE" => Some("wayland".to_string()),
            _ => None,
        };
        assert_eq!(
            session_facts(hypr),
            json!({ "compositor": "Hyprland", "session": "wayland" })
        );
        let x11 = |k: &str| match k {
            "XDG_CURRENT_DESKTOP" => Some("GNOME:ubuntu".to_string()),
            "DISPLAY" => Some(":0".to_string()),
            _ => None,
        };
        assert_eq!(
            session_facts(x11),
            json!({ "compositor": "GNOME", "session": "x11" })
        );
    }

    #[test]
    fn portal_interfaces_are_read_from_the_introspection() {
        let text = "NAME                                TYPE      SIGNATURE\norg.freedesktop.DBus.Peer           interface -\norg.freedesktop.portal.GlobalShortcuts interface -\n.BindShortcuts                      method    osa(sa{sv})sa{sv} o\norg.freedesktop.portal.Settings     interface -\n";
        assert_eq!(
            portal_interfaces(text),
            [
                "org.freedesktop.portal.GlobalShortcuts",
                "org.freedesktop.portal.Settings"
            ]
        );
    }

    #[test]
    fn models_are_counted_per_provider_and_the_provider_order_is_kept() {
        let status = json!({ "models_dir": "/m", "catalogue_version": 1, "models": [
            { "provider": "whisper", "id": "tiny.en", "readiness": "ready", "is_selected": false },
            { "provider": "whisper", "id": "large-v3-turbo", "readiness": "missing", "is_selected": true },
            { "provider": "parakeet", "id": "parakeet-v3", "readiness": "quarantined", "is_selected": false },
        ]});
        let facts = models_facts(&status);
        assert_eq!(facts["selected"]["id"], "large-v3-turbo");
        assert_eq!(facts["providers"][0]["provider"], "parakeet");
        assert_eq!(facts["providers"][0]["quarantined"], 1);
        assert_eq!(facts["providers"][1]["ready"], 1);
        assert_eq!(facts["providers"][1]["selected"], "large-v3-turbo");
        let providers = json!({ "selected": "auto", "providers": [
            { "id": "local", "available": false, "detail": "not downloaded", "hint": "dettivo llm download", "model": "qwen3-4b" },
            { "id": "ollama", "available": true, "model": "qwen3:4b" },
        ]});
        let llm = llm_facts(
            &providers,
            json!({ "running": false }),
            &json!({ "models": [] }),
        );
        assert_eq!(llm["provider_order"][0]["id"], "local");
        assert_eq!(llm["provider_order"][1]["available"], true);
        assert_eq!(llm["local_hint"], "dettivo llm download");
        assert_eq!(llm["engine"]["running"], false);
    }
}
