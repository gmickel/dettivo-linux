//! The doctor's diarization row: the configured model set with its
//! readiness, and the engine that loads it with the backend and reason of
//! its last load (ADR 0035, ADR 0073).

use serde_json::{Value, json};

use crate::client::Client;

/// The Nemotron model set and the engine that loads it. A client depends
/// on dettivo-proto alone, so the pair is named here as the daemon's
/// `dettivo_speech::diarize::binary_for` names it.
const NEMOTRON: (&str, &str) = ("nemotron-3-diarization", "dettivo-engine-nemotron");
/// Every other model set's engine.
const SHERPA_ENGINE: &str = "dettivo-engine-diarize";
/// The configured set when the daemon does not say.
const DEFAULT_MODEL: &str = "diarization-en";

/// The engine binary for a model set.
fn engine_for(model: &str) -> &'static str {
    if model == NEMOTRON.0 {
        NEMOTRON.1
    } else {
        SHERPA_ENGINE
    }
}

/// The row from `speech.models.status` (`models`) and the engine rows of
/// the report (`engines`), for the model `[meetings.diarization] model`
/// names.
pub fn facts(models: &Value, engines: &Value, client: &Client) -> Value {
    let configured = client
        .call("config.get", json!({ "key": "meetings.diarization.model" }))
        .ok()
        .and_then(|r| r["entries"][0]["value"].as_str().map(str::to_string))
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    row(models, engines, &configured)
}

fn row(models: &Value, engines: &Value, configured: &str) -> Value {
    let model = models["models"]
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .find(|r| r["provider"] == "diarize" && r["id"] == configured)
                .cloned()
        })
        .unwrap_or(Value::Null);
    let binary = engine_for(configured);
    let engine = engines
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["binary"] == binary).cloned())
        .unwrap_or(Value::Null);
    json!({
        "model": format!("diarize/{configured}"),
        "model_readiness": model["readiness"],
        "engine": engine,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_row_follows_the_configured_model_to_its_engine() {
        let models = json!({"models": [
            {"provider": "diarize", "id": "diarization-en", "readiness": "ready"},
            {"provider": "diarize", "id": "nemotron-3-diarization", "readiness": "missing"},
        ]});
        let engines = json!([
            {"binary": "dettivo-engine-diarize", "backend": null},
            {"binary": "dettivo-engine-nemotron", "backend": "vulkan", "reason": "ggml Vulkan device 0 loaded the model"},
        ]);
        let nemotron = row(&models, &engines, "nemotron-3-diarization");
        assert_eq!(nemotron["model"], "diarize/nemotron-3-diarization");
        assert_eq!(nemotron["model_readiness"], "missing");
        assert_eq!(nemotron["engine"]["backend"], "vulkan");
        let sherpa = row(&models, &engines, "diarization-en");
        assert_eq!(sherpa["model_readiness"], "ready");
        assert_eq!(sherpa["engine"]["binary"], "dettivo-engine-diarize");
    }
}
