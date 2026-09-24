//! `dettivo llm experiment`: the sideloaded polish fine-tune (ADR 0032,
//! docs/polish-models.md). `status` reads the sideload row of
//! `llm.models.status`, `use` sets `[llm] polish_experiment` to a
//! manifest name through `config.set`, and `clear` unsets it; every
//! write comes back with the row the daemon now serves, so a manifest
//! that does not resolve is visible the moment it is chosen.

use clap::Subcommand;
use serde_json::{Value, json};

use crate::client::Client;
use crate::exit::Failure;
use crate::{Cli, output};

/// `dettivo llm experiment <what>`.
#[derive(Debug, Subcommand)]
pub enum ExperimentCmd {
    /// The sideload in force: its manifest, readiness and form.
    Status,
    /// Run Enhanced on the fine-tune a manifest names (sets `[llm] polish_experiment`).
    Use {
        /// The manifest name under the experiments directory (`current` reads current.json).
        #[arg(default_value = "current")]
        name: String,
    },
    /// Back to the catalogue model (clears `[llm] polish_experiment`).
    Clear,
}

/// The pseudo method the status rendering is keyed on.
pub const STATUS_METHOD: &str = "llm.experiment.status";

/// The sideload rows of an `llm.models.status` result, with the key in
/// force, as one value for the rendering.
fn sideload_view(status: &Value) -> Value {
    let rows: Vec<Value> = status["models"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter(|m| m["source"] == "sideload")
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    json!({
        "polish_experiment": status["polish_experiment"],
        "selected": status["selected"],
        "models": rows,
    })
}

/// Runs an `experiment` subcommand.
pub fn run(cli: &Cli, client: &Client, what: &ExperimentCmd) -> Result<(), Failure> {
    match what {
        ExperimentCmd::Status => {}
        ExperimentCmd::Use { name } => {
            client.call(
                "config.set",
                json!({"key": "llm.polish_experiment", "value": name}),
            )?;
        }
        ExperimentCmd::Clear => {
            client.call("config.unset", json!({"key": "llm.polish_experiment"}))?;
        }
    }
    let status = client.call("llm.models.status", json!({}))?;
    output::result(cli, STATUS_METHOD, &sideload_view(&status));
    Ok(())
}

/// The human rendering of the status view.
pub fn render(view: &Value) -> String {
    let key = view["polish_experiment"].as_str().unwrap_or_default();
    if key.is_empty() {
        return format!(
            "experiment  none; Enhanced runs on llm/{}\n",
            view["selected"].as_str().unwrap_or_default()
        );
    }
    let mut out = String::new();
    for m in view["models"].as_array().into_iter().flatten() {
        match m["manifest_error"].as_str() {
            Some(error) => out.push_str(&format!(
                "experiment  {key}: does not resolve ({error}); Enhanced runs on llm/{}\n",
                view["selected"].as_str().unwrap_or_default()
            )),
            None => {
                out.push_str(&format!(
                    "experiment  {key}: {} ({}, {}) {}{}\n",
                    m["id"].as_str().unwrap_or_default(),
                    m["display_name"].as_str().unwrap_or_default(),
                    m["format"].as_str().unwrap_or_default(),
                    m["readiness"].as_str().unwrap_or_default(),
                    m["base_model"]
                        .as_str()
                        .map(|b| format!(" over llm/{b}"))
                        .unwrap_or_default(),
                ));
                if let Some(path) = m["path"].as_str() {
                    out.push_str(&format!("file        {path}\n"));
                }
                if let Some(error) = m["error"].as_str() {
                    out.push_str(&format!("error       {error}\n"));
                }
            }
        }
    }
    if out.is_empty() {
        out.push_str(&format!("experiment  {key}: not reported by the daemon\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_view_keeps_the_sideload_rows_only() {
        let status = json!({
            "polish_experiment": "current",
            "selected": "qwen3-4b-instruct-2507",
            "models": [
                {"id": "qwen3-4b-instruct-2507", "source": "catalogue"},
                {"id": "tuned", "source": "sideload", "display_name": "Tuned", "format": "gguf-lora",
                 "readiness": "missing", "base_model": "qwen3-1.7b", "path": null,
                 "error": "the base llm/qwen3-1.7b is not downloaded; run `dettivo llm download --model qwen3-1.7b`"}
            ]
        });
        let view = sideload_view(&status);
        assert_eq!(view["models"].as_array().unwrap().len(), 1);
        let text = render(&view);
        assert_eq!(
            text,
            "experiment  current: tuned (Tuned, gguf-lora) missing over llm/qwen3-1.7b\nerror       the base llm/qwen3-1.7b is not downloaded; run `dettivo llm download --model qwen3-1.7b`\n"
        );
    }

    #[test]
    fn no_key_and_a_broken_manifest_say_what_enhanced_runs_on() {
        let none = render(
            &json!({"polish_experiment": "", "selected": "qwen3-4b-instruct-2507", "models": []}),
        );
        assert_eq!(
            none,
            "experiment  none; Enhanced runs on llm/qwen3-4b-instruct-2507\n"
        );
        let broken = render(&json!({
            "polish_experiment": "current",
            "selected": "qwen3-4b-instruct-2507",
            "models": [{"id": "current", "source": "sideload", "manifest_error": "/x/current.json cannot be read"}]
        }));
        assert!(broken.starts_with("experiment  current: does not resolve (/x/current.json cannot be read); Enhanced runs on llm/qwen3-4b-instruct-2507"), "{broken}");
    }
}
