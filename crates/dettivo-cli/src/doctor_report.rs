//! Concise readiness rows and probe failures for `dettivo doctor`.

use serde_json::Value;

/// Renders the machine snapshot without repeating the detailed JSON payloads.
pub fn human(report: &Value) -> String {
    let mut out = String::new();
    let mut row = |name: &str, text: String| {
        if name.starts_with("-- ") {
            out.push_str(&format!("{name} {text}\n"));
        } else {
            out.push_str(&format!("{name:<9} {text}\n"));
        }
    };
    row("-- desktop", "--".into());
    row(
        "socket",
        format!(
            "{} ({})",
            show(&report["socket"]["path"]),
            if report["socket"]["exists"] == true {
                "present"
            } else {
                "missing"
            }
        ),
    );
    row(
        "service",
        format!(
            "dettivod.socket {}, dettivod.service {}",
            show(&report["service"]["socket_unit"]["active"]),
            show(&report["service"]["service_unit"]["active"])
        ),
    );
    row(
        "session",
        format!(
            "{} on {}",
            show(&report["compositor"]["name"]),
            show(&report["session_type"])
        ),
    );
    row(
        "portals",
        if report["portals"]["available"] == true {
            "available".into()
        } else {
            format!("not available: {}", show(&report["portals"]["reason"]))
        },
    );
    row(
        "audio",
        if report["audio"]["pipewire"] == true {
            "PipeWire ready".into()
        } else {
            "capture unavailable".into()
        },
    );
    row("microphone ready", microphone_ready(&report["audio"]));
    row("-- daemon", "--".into());
    row(
        "daemon",
        if report["daemon"]["reachable"] == true {
            format!(
                "reachable, version {}, health {}",
                show(&report["daemon"]["version"]["app_version"]),
                show(&report["daemon"]["health"]["ok"])
            )
        } else {
            format!("unreachable: {}", show(&report["daemon"]["error"]))
        },
    );
    if let Some(probes) = report["daemon"]["probes"].as_object() {
        for (method, probe) in probes {
            row(
                "probe",
                format!(
                    "{method}: {}: {}",
                    show(&probe["state"]),
                    show(&probe["error"]["message"])
                ),
            );
        }
    }
    row(
        "config",
        match report["config"]["ok"] {
            Value::Bool(true) => "valid".into(),
            Value::Bool(false) => format!(
                "INVALID\n{}",
                crate::output::human("config.validate", &report["config"])
            ),
            _ => format!("unknown: {}", show(&report["config"]["note"])),
        },
    );
    row(
        "tier",
        format!(
            "{}: {}",
            show(&report["tier"]["tier"]),
            show(&report["tier"]["reason"])
        ),
    );
    row(
        "platform",
        format!(
            "{} on {}",
            show(&report["platform"]["os"]),
            show(&report["platform"]["compositor"])
        ),
    );
    row("-- insertion and hotkeys", "--".into());
    row("backend", show(&report["platform"]["insertion_backend"]));
    if let Some(backends) = report["insertion"]["backends"].as_array() {
        for backend in backends {
            row(
                "backend",
                format!(
                    "{}: {}{}",
                    show(&backend["name"]),
                    if backend["available"] == true {
                        "ready"
                    } else {
                        "unavailable"
                    },
                    backend["reason"]
                        .as_str()
                        .map(|reason| format!(": {reason}"))
                        .unwrap_or_default()
                ),
            );
        }
    }
    if report["insertion"]["compositor"].is_object() {
        row(
            "compositor",
            compositor_row(&report["insertion"]["compositor"]),
        );
    }
    if report["hotkeys"].is_object() {
        for line in crate::output::hotkeys_lines(&report["hotkeys"]).lines() {
            row("hotkeys", line.into());
        }
    }
    if report["snippet"]["main_config"].is_object() {
        let text = crate::setup::check_line(&report["snippet"]);
        row(
            "snippet",
            text.trim_start_matches("snippet   ").trim_end().into(),
        );
    }
    row("-- engines and models", "--".into());
    if let Some(engines) = report["engines"].as_array() {
        for engine in engines {
            row(
                "engine",
                format!("{} {}", show(&engine["binary"]), engine_state(engine)),
            );
        }
    } else {
        row("engine", "not reported".into());
    }
    row(
        "models",
        format!(
            "{} ready, {} quarantined; selected {}/{} {}",
            show(&report["models"]["ready"]),
            show(&report["models"]["quarantined"]),
            show(&report["models"]["selected"]["provider"]),
            show(&report["models"]["selected"]["id"]),
            show(&report["models"]["selected"]["readiness"])
        ),
    );
    let vad = &report["models"]["vad"];
    row(
        "voice gate",
        match (vad["action"].as_str(), vad["readiness"].as_str()) {
            (Some(action), _) => format!(
                "{} {}: dictation gates on level alone; run `{action}`",
                show(&vad["model"]),
                show(&vad["readiness"])
            ),
            (None, Some("unverified")) => format!(
                "{} unverified: on disk, checked before the voice gate first uses it",
                show(&vad["model"])
            ),
            (None, _) => format!("{} {}", show(&vad["model"]), show(&vad["readiness"])),
        },
    );
    row(
        "llm",
        format!(
            "provider {}; model {} {}; {}",
            show(&report["llm"]["selected_provider"]),
            show(&report["llm"]["model"]),
            show(&report["llm"]["model_readiness"]),
            engine_state(&report["llm"]["engine"])
        ),
    );
    row(
        "diarize",
        format!(
            "model {} {}; {}",
            show(&report["diarization"]["model"]),
            show(&report["diarization"]["model_readiness"]),
            engine_state(&report["diarization"]["engine"])
        ),
    );
    row("-- clients", "--".into());
    row(
        "history",
        format!(
            "{} items in {} ({})",
            show(&report["history"]["item_count"]),
            show(&report["history"]["path"]),
            show(&report["history"]["last_migration"])
        ),
    );
    row(
        "osd",
        if report["osd"]["host"] == "disabled" {
            format!("disabled: {}", show(&report["osd"]["notice"]))
        } else {
            show(&report["osd"]["host"])
        },
    );
    row(
        "app",
        if report["app"]["running"] == true {
            format!(
                "renderer {} ({})",
                show(&report["app"]["renderer"]),
                show(&report["app"]["renderer_reason"])
            )
        } else {
            "not running".into()
        },
    );
    row(
        "rest",
        format!(
            "enabled {}, {}:{}",
            show(&report["rest"]["enabled"]),
            show(&report["rest"]["bind"]),
            show(&report["rest"]["port"])
        ),
    );
    row(
        "mcp",
        format!(
            "{}: {} tools",
            show(&report["mcp"]["server"]["name"]),
            show(&report["mcp"]["tools"])
        ),
    );
    let env: Vec<_> = report["environment"]
        .as_object()
        .into_iter()
        .flat_map(|env| env.iter())
        .filter(|(_, value)| **value == true)
        .map(|(key, _)| key.as_str())
        .collect();
    row(
        "env",
        if env.is_empty() {
            "no DETTIVO_* overrides".into()
        } else {
            env.join(", ")
        },
    );
    out.push_str(&crate::omarchy_check::doctor_line(&report["omarchy"]));
    out
}

/// The always-ready microphone's row (ADR 0092): a capture that could
/// not open is named here, never as a failed dictation. A press opens the
/// microphone itself only while PipeWire is there. An open or waiting ring
/// says whether a screen lock closes it.
fn microphone_ready(audio: &Value) -> String {
    let ready = &audio["microphone_ready"];
    let lock = match ready["lock_signal"].as_str() {
        Some("compositor") => "closes when the screen locks",
        Some("logind") => "closes on a lock only where the locker sets logind's LockedHint",
        _ => "no lock signal: stays open while the screen is locked",
    };
    match ready["state"].as_str() {
        Some("off") | None => "off ([audio] keep_microphone_ready)".into(),
        Some("open") => format!("open: takes start 450 ms before the press; {lock}"),
        Some("closed") => format!("closed: {}; {lock}", show(&ready["reason"])),
        Some(_) if audio["pipewire"] != true => format!("failed: {}", show(&ready["reason"])),
        Some(_) => format!(
            "failed: {}; dictation opens the microphone on press",
            show(&ready["reason"])
        ),
    }
}

/// The Hyprland instance the daemon talks to, and whether the signature
/// in its environment is stale (fn-91).
fn compositor_row(instance: &Value) -> String {
    let environment = show(&instance["environment_signature"]);
    match instance["signature"].as_str() {
        None => format!(
            "no Hyprland instance answers for this display (the environment names {environment})"
        ),
        Some(live) if instance["environment_stale"] == true => format!(
            "Hyprland instance {live}; the environment's {environment} is stale (the compositor restarted)"
        ),
        Some(live) => format!("Hyprland instance {live}"),
    }
}

fn show(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "unknown".into(),
        other => other.to_string(),
    }
}

fn engine_state(engine: &Value) -> String {
    if engine.is_null() {
        "unknown".into()
    } else if engine["degraded"] == true {
        format!("DEGRADED after {} crashes", show(&engine["crashes"]))
    } else if engine["running"] == true {
        format!(
            "running on {}, {}",
            show(&engine["backend"]),
            crate::speech_lines::warmth(engine)
        )
    } else if engine["path"].is_string() {
        "ready, not running".into()
    } else {
        "not found".into()
    }
}

#[cfg(test)]
mod tests {
    use super::human;
    use crate::doctor::models_facts;
    use serde_json::json;

    fn voice_gate_row(readiness: &str) -> String {
        let status = json!({ "models": [
            { "provider": "whisper", "id": "silero-vad", "readiness": readiness },
        ]});
        let report = json!({ "models": models_facts(&status) });
        human(&report)
            .lines()
            .find(|l| l.starts_with("voice gate"))
            .expect("a voice gate row")
            .to_string()
    }

    /// fn-79 R3: only an absent or refused model asks for a download; a model
    /// on disk that start-up has not hashed yet says it is being checked.
    #[test]
    fn the_voice_gate_row_asks_for_a_download_only_when_the_model_is_absent() {
        let fetch = "run `dettivo speech download --model silero-vad`";
        for readiness in ["missing", "partial", "quarantined"] {
            let row = voice_gate_row(readiness);
            assert!(row.contains(fetch), "{readiness}: {row}");
            assert!(row.contains("gates on level alone"), "{readiness}: {row}");
        }
        assert_eq!(
            voice_gate_row("unverified"),
            "voice gate whisper/silero-vad unverified: on disk, checked before the voice gate first uses it"
        );
        assert_eq!(
            voice_gate_row("ready"),
            "voice gate whisper/silero-vad ready"
        );
        assert_eq!(
            voice_gate_row("downloading"),
            "voice gate whisper/silero-vad downloading"
        );
    }

    /// fn-81 R6: an always-ready microphone that cannot open is a doctor
    /// row naming the failure.
    #[test]
    fn the_microphone_ready_row_names_its_state() {
        let row_on = |pipewire: bool, ready: serde_json::Value| {
            human(&json!({ "audio": { "pipewire": pipewire, "microphone_ready": ready } }))
                .lines()
                .find(|l| l.starts_with("microphone ready"))
                .expect("a microphone ready row")
                .to_string()
        };
        let row = |ready: serde_json::Value| row_on(true, ready);
        let failed =
            row(json!({"state": "failed", "reason": "audio device \"usb\" is not present"}));
        assert!(
            failed.contains("failed: audio device \"usb\" is not present; dictation opens the microphone on press"),
            "{failed}"
        );
        assert!(
            row(json!({"state": "off", "reason": null}))
                .contains("off ([audio] keep_microphone_ready)")
        );
        assert!(
            row(json!({"state": "closed", "reason": "the session is locked"}))
                .contains("closed: the session is locked")
        );
        // QA81-3: with PipeWire gone a press cannot open the microphone
        // either, so the row names PipeWire and promises nothing.
        let gone = row_on(
            false,
            json!({"state": "failed", "reason": "PipeWire is not reachable: PipeWire went away"}),
        );
        assert!(
            gone.ends_with("failed: PipeWire is not reachable: PipeWire went away"),
            "{gone}"
        );
    }

    /// QA81-1: an open ring says whether a screen lock closes it, and
    /// warns when nothing would.
    #[test]
    fn the_microphone_ready_row_names_the_lock_signal() {
        for (signal, says) in [
            ("compositor", "closes when the screen locks"),
            (
                "logind",
                "closes on a lock only where the locker sets logind's LockedHint",
            ),
            (
                "none",
                "no lock signal: stays open while the screen is locked",
            ),
        ] {
            let ready = json!({"state": "open", "reason": null, "lock_signal": signal});
            let report =
                human(&json!({ "audio": { "pipewire": true, "microphone_ready": ready } }));
            let row = report
                .lines()
                .find(|l| l.starts_with("microphone ready"))
                .unwrap();
            assert!(
                row.ends_with(&format!(
                    "open: takes start 450 ms before the press; {says}"
                )),
                "{row}"
            );
        }
    }

    /// fn-91 R4: the doctor names the Hyprland instance in use and a stale
    /// signature in the daemon's environment.
    #[test]
    fn the_compositor_row_names_the_instance_and_a_stale_environment() {
        let row = |instance: serde_json::Value| {
            human(&json!({ "insertion": { "compositor": instance } }))
                .lines()
                .find(|l| l.starts_with("compositor"))
                .expect("a compositor row")
                .to_string()
        };
        let current =
            json!({"signature": "abc", "environment_signature": "abc", "environment_stale": false});
        assert!(row(current).ends_with("Hyprland instance abc"));
        let stale =
            json!({"signature": "new", "environment_signature": "old", "environment_stale": true});
        assert!(row(stale).ends_with(
            "Hyprland instance new; the environment's old is stale (the compositor restarted)"
        ));
        let none =
            json!({"signature": null, "environment_signature": "old", "environment_stale": true});
        assert!(row(none).ends_with(
            "no Hyprland instance answers for this display (the environment names old)"
        ));
        assert!(!human(&json!({})).contains("\ncompositor"));
    }
}
