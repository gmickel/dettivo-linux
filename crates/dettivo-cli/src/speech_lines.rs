//! The human lines of `dettivo speech providers --language` (the
//! catalogue's ranking for a language, ADR 0085) and the engine states
//! `dettivo speech status` and `dettivo doctor` show (cold, loaded or
//! warm, ADR 0084).

use serde_json::Value;

/// Rates within this many points of the recommended model read as close.
const CLOSE_POINTS: f64 = 1.5;

/// The English name of a language code the ranking covers, else the code.
fn language_name(code: &str) -> &str {
    match code {
        "en" => "English",
        "de" => "German",
        "fr" => "French",
        "es" => "Spanish",
        "it" => "Italian",
        "nl" => "Dutch",
        "pt" => "Portuguese",
        "pl" => "Polish",
        other => other,
    }
}

/// The recommendation sentence and one line per ranked model.
pub fn ranking(ranking: &Value) -> String {
    let code = ranking["language"].as_str().unwrap_or_default();
    let name = language_name(code);
    let models = ranking["models"].as_array().cloned().unwrap_or_default();
    let Some(best) = models.first() else {
        return format!(
            "No recorded ranking for {name}; the catalogue ranks English, German, French, Spanish, Italian, Dutch, Portuguese and Polish.\n"
        );
    };
    let wer = |m: &Value| m["wer"].as_f64().unwrap_or(f64::NAN);
    let mut out = format!(
        "For {name}, {} is recommended (WER {:.1})",
        best["model_id"].as_str().unwrap_or_default(),
        wer(best)
    );
    let close: Vec<String> = models[1..]
        .iter()
        .filter(|m| wer(m) < wer(best) + CLOSE_POINTS)
        .map(|m| {
            format!(
                "{} is close ({:.1})",
                m["model_id"].as_str().unwrap_or_default(),
                wer(m)
            )
        })
        .collect();
    if !close.is_empty() {
        out.push_str(&format!(", {}", close.join(", ")));
    }
    out.push_str(".\n");
    for m in &models {
        out.push_str(&format!(
            "  {}  {}/{}  WER {:.1}{}\n",
            m["rank"],
            m["provider"].as_str().unwrap_or_default(),
            m["model_id"].as_str().unwrap_or_default(),
            wer(m),
            if m["is_downloaded"] == true {
                "  ready"
            } else {
                ""
            }
        ));
    }
    out
}

/// `cold`, `loaded` or `warm` for one `speech.engines` row: whether the
/// next take pays the start, the kernel compile, or neither.
pub fn warmth(engine: &Value) -> &'static str {
    if engine["running"] != true || engine["model"].is_null() {
        "cold"
    } else if engine["warm"] == true {
        "warm"
    } else {
        "loaded"
    }
}

/// One line per speech engine for `dettivo speech status`.
pub fn engine_states(engines: &Value) -> String {
    engines["engines"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| {
            e["binary"]
                .as_str()
                .is_some_and(|b| b == "dettivo-engine-whisper" || b == "dettivo-engine-parakeet")
        })
        .map(|e| {
            format!(
                "engine {} {}{}\n",
                e["binary"].as_str().unwrap_or_default(),
                warmth(e),
                e["backend"]
                    .as_str()
                    .map(|b| format!(" on {b}"))
                    .unwrap_or_default()
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_ranking_names_the_recommendation_and_the_close_runner_up() {
        let rows = json!({"language": "de", "models": [
            {"provider": "parakeet", "model_id": "parakeet-v3", "rank": 1, "wer": 4.1, "is_downloaded": true},
            {"provider": "whisper", "model_id": "large-v3-turbo", "rank": 2, "wer": 4.9, "is_downloaded": false},
            {"provider": "parakeet", "model_id": "parakeet-ultra", "rank": 3, "wer": 7.0, "is_downloaded": false},
        ]});
        let text = ranking(&rows);
        assert!(
            text.starts_with(
                "For German, parakeet-v3 is recommended (WER 4.1), large-v3-turbo is close (4.9).\n"
            ),
            "{text}"
        );
        assert!(
            text.contains("  1  parakeet/parakeet-v3  WER 4.1  ready\n"),
            "{text}"
        );
        assert!(
            text.contains("  3  parakeet/parakeet-ultra  WER 7.0\n"),
            "{text}"
        );
        let none = ranking(&json!({"language": "xx", "models": []}));
        assert!(none.starts_with("No recorded ranking for xx"), "{none}");
    }

    #[test]
    fn an_engine_reads_cold_loaded_or_warm() {
        let row = |running: bool, model: Value, warm: bool| json!({"running": running, "model": model, "warm": warm});
        assert_eq!(warmth(&row(false, Value::Null, false)), "cold");
        assert_eq!(warmth(&row(true, Value::Null, false)), "cold");
        assert_eq!(warmth(&row(true, json!("/m.bin"), false)), "loaded");
        assert_eq!(warmth(&row(true, json!("/m.bin"), true)), "warm");
    }
}
