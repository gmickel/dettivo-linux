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
}
