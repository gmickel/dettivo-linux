//! `dettivo polish` and `dettivo llm`: the `polish.*` methods and the
//! Linux `llm.*` additions (ADR 0023, ADR 0026, `docs/polish.md`):
//! providers, endpoints, the language model catalogue and the local
//! engine. `polish test` is the one to reach for first: it shows what
//! each layer would do to a sample without dictating anything.

use clap::Subcommand;
use serde_json::{Value, json};

use crate::client::Client;
use crate::commands::strip_nulls;
use crate::exit::Failure;
use crate::{Cli, output};

/// `dettivo polish <what>`.
#[derive(Debug, Subcommand)]
pub enum PolishCmd {
    /// Run a sample through the layers and print what each one made of it.
    Test {
        /// The dictated text to try.
        text: String,
        /// The preset (email, code, chat, notes, generic).
        #[arg(long, default_value = "generic")]
        preset: String,
        /// The style (asDictated, formal, casual, veryCasual).
        #[arg(long)]
        style: Option<String>,
        /// The app id the policy resolves for.
        #[arg(long, value_name = "APP_ID")]
        bundle_id: Option<String>,
        /// raw, deterministic_polish (the default) or enhanced.
        #[arg(long)]
        mode: Option<String>,
        /// A custom rule to apply, by name; repeatable.
        #[arg(long = "rule", value_name = "NAME")]
        rules: Vec<String>,
    },
    /// The custom rules the Enhanced rewrite carries.
    Rules {
        /// Which rules verb.
        #[command(subcommand)]
        what: RulesCmd,
    },
    /// polish.presets.list: the presets a profile or a test may name.
    Presets,
    /// The per-app presets.
    Apps {
        /// Which apps verb.
        #[command(subcommand)]
        what: AppsCmd,
    },
}

/// `dettivo polish rules <what>`.
#[derive(Debug, Subcommand)]
pub enum RulesCmd {
    /// polish.rules.list
    List,
    /// polish.rules.create
    Add {
        /// The rule's name.
        name: String,
        /// The instruction, at most 500 characters.
        content: String,
        /// Create it switched off.
        #[arg(long)]
        disabled: bool,
    },
    /// polish.rules.update
    Set {
        /// The rule's id (polish rules list prints it).
        rule_id: String,
        /// The new instruction.
        content: String,
        /// Switch the rule off.
        #[arg(long)]
        disabled: bool,
    },
    /// polish.rules.delete
    Remove {
        /// The rule's id.
        rule_id: String,
    },
}

/// `dettivo polish apps <what>`.
#[derive(Debug, Subcommand)]
pub enum AppsCmd {
    /// polish.apps.list
    List,
    /// polish.apps.set
    Set {
        /// The Wayland app id or X11 class ("prefix*" matches a family).
        bundle_id: String,
        /// The preset that app takes.
        preset: String,
    },
}

/// `dettivo llm <what>`.
#[derive(Debug, Subcommand)]
pub enum LlmCmd {
    /// llm.providers.list: which providers answer right now.
    Providers,
    /// llm.endpoints.trust: allow an endpoint that is not on this machine.
    Trust {
        /// The endpoint URL.
        url: String,
    },
    /// llm.endpoints.list: the endpoints in force and which are local.
    Endpoints,
    /// Run a sample through the Enhanced pass (polish.test with mode=enhanced).
    Test {
        /// The dictated text to try.
        text: String,
        /// The preset (email, code, chat, notes, generic).
        #[arg(long, default_value = "generic")]
        preset: String,
    },
    /// llm.models.download: fetch and verify a catalogue language model.
    Download {
        /// Model id, e.g. qwen3-4b-instruct-2507.
        #[arg(long)]
        model: String,
        /// Print progress until the download ends.
        #[arg(long)]
        wait: bool,
    },
    /// llm.models.cancel: stop a download, keeping the partial file.
    Cancel {
        /// Model id.
        #[arg(long)]
        model: String,
    },
    /// llm.models.delete: remove a language model from disk.
    Delete {
        /// Model id.
        #[arg(long)]
        model: String,
        /// Delete even the selected model.
        #[arg(long)]
        force: bool,
    },
    /// llm.models.status: every language model's readiness.
    Status,
    /// llm.engine.status: the local engine process, its backend and memory.
    Engine,
    /// The sideloaded polish fine-tune: status, use, clear.
    Experiment {
        #[command(subcommand)]
        what: crate::experiment::ExperimentCmd,
    },
}

/// Runs a `polish` subcommand.
pub fn polish(cli: &Cli, client: &Client, what: &PolishCmd) -> Result<(), Failure> {
    let (method, params) = match what {
        PolishCmd::Test {
            text,
            preset,
            style,
            bundle_id,
            mode,
            rules,
        } => (
            "polish.test",
            strip_nulls(json!({
                "input": text,
                "preset": preset,
                "rules": rules,
                "style": style,
                "bundle_id": bundle_id,
                "mode": mode,
            })),
        ),
        PolishCmd::Presets => ("polish.presets.list", json!({})),
        PolishCmd::Rules { what } => match what {
            RulesCmd::List => ("polish.rules.list", json!({})),
            RulesCmd::Add {
                name,
                content,
                disabled,
            } => (
                "polish.rules.create",
                json!({"name": name, "content": content, "enabled": !disabled}),
            ),
            RulesCmd::Set {
                rule_id,
                content,
                disabled,
            } => (
                "polish.rules.update",
                json!({"rule_id": rule_id, "content": content, "enabled": !disabled}),
            ),
            RulesCmd::Remove { rule_id } => ("polish.rules.delete", json!({ "rule_id": rule_id })),
        },
        PolishCmd::Apps { what } => match what {
            AppsCmd::List => ("polish.apps.list", json!({})),
            AppsCmd::Set { bundle_id, preset } => (
                "polish.apps.set",
                json!({"bundle_id": bundle_id, "preset": preset}),
            ),
        },
    };
    let result = client.call(method, params)?;
    output::result(cli, method, &result);
    Ok(())
}

/// Runs an `llm` subcommand.
pub fn llm(cli: &Cli, client: &Client, what: &LlmCmd) -> Result<(), Failure> {
    let (method, params) = match what {
        LlmCmd::Providers => ("llm.providers.list", json!({})),
        LlmCmd::Trust { url } => ("llm.endpoints.trust", json!({ "url": url })),
        LlmCmd::Endpoints => ("llm.endpoints.list", json!({})),
        LlmCmd::Test { text, preset } => (
            "polish.test",
            json!({"input": text, "preset": preset, "rules": [], "mode": "enhanced"}),
        ),
        LlmCmd::Download { model, wait } => {
            let params = json!({ "model": model });
            let result = client.call("llm.models.download", params.clone())?;
            if !*wait {
                output::result(cli, "llm.models.download", &result);
                return Ok(());
            }
            return crate::speech::follow_download(cli, client, "llm.models.status", params);
        }
        LlmCmd::Cancel { model } => ("llm.models.cancel", json!({ "model": model })),
        LlmCmd::Delete { model, force } => (
            "llm.models.delete",
            json!({ "model": model, "force": force }),
        ),
        LlmCmd::Status => ("llm.models.status", json!({})),
        LlmCmd::Engine => ("llm.engine.status", json!({})),
        LlmCmd::Experiment { what } => return crate::experiment::run(cli, client, what),
    };
    let result = client.call(method, params)?;
    output::result(cli, method, &result);
    Ok(())
}

/// The human rendering of a `polish.*` or `llm.*` result: one line per
/// row, and for `polish.test` the layers in the order they ran.
pub fn render(method: &str, result: &Value) -> Option<String> {
    let mut out = String::new();
    match method {
        "polish.test" => {
            for (label, key) in [
                ("raw     ", "raw"),
                ("polished", "polished"),
                ("enhanced", "enhanced"),
            ] {
                if let Some(text) = result[key].as_str() {
                    out.push_str(&format!("{label}  {text}\n"));
                }
            }
            out.push_str(&format!(
                "inserted  {}\n",
                result["output"].as_str().unwrap_or_default()
            ));
            if let Some(kind) = result["notice"]["kind"].as_str() {
                out.push_str(&format!(
                    "notice    {kind}: {}\n",
                    result["notice"]["reason"].as_str().unwrap_or_default()
                ));
            }
            out.push_str(&format!(
                "model     {}  policy {}\n",
                result["model"].as_str().unwrap_or_default(),
                result["policy_hash"].as_str().unwrap_or("-")
            ));
        }
        "polish.rules.list" => {
            for rule in result["rules"].as_array()? {
                out.push_str(&format!(
                    "{}  {:<24} {}  {}\n",
                    if rule["enabled"] == Value::Bool(true) {
                        "on "
                    } else {
                        "off"
                    },
                    rule["name"].as_str().unwrap_or_default(),
                    rule["rule_id"].as_str().unwrap_or_default(),
                    rule["content"].as_str().unwrap_or_default(),
                ));
            }
        }
        "polish.presets.list" => {
            for preset in result["presets"].as_array()? {
                out.push_str(&format!(
                    "{:<10} {}\n",
                    preset["id"].as_str().unwrap_or_default(),
                    preset["display_name"].as_str().unwrap_or_default(),
                ));
            }
        }
        "polish.apps.list" => {
            for mapping in result["mappings"].as_array()? {
                out.push_str(&format!(
                    "{:<32} {}\n",
                    mapping["bundle_id"].as_str().unwrap_or_default(),
                    mapping["preset"].as_str().unwrap_or_default(),
                ));
            }
        }
        "llm.providers.list" => {
            for provider in result["providers"].as_array()? {
                out.push_str(&format!(
                    "{}  {:<18} {:<24} {}\n",
                    if provider["available"] == Value::Bool(true) {
                        "up  "
                    } else {
                        "down"
                    },
                    provider["id"].as_str().unwrap_or_default(),
                    provider["model"].as_str().unwrap_or_default(),
                    provider["hint"]
                        .as_str()
                        .unwrap_or_else(|| provider["detail"].as_str().unwrap_or_default()),
                ));
            }
            if let Some(selected) = result["selected"].as_str() {
                out.push_str(&format!("selected  {selected}\n"));
            }
        }
        "llm.models.status" => {
            for m in result["models"].as_array()? {
                out.push_str(&format!(
                    "{:<12} {:<26} {}{}{}\n",
                    m["readiness"].as_str().unwrap_or_default(),
                    m["id"].as_str().unwrap_or_default(),
                    m["display_name"].as_str().unwrap_or_default(),
                    if m["source"] == "sideload" {
                        "  (sideload)"
                    } else {
                        ""
                    },
                    if m["is_selected"] == Value::Bool(true) {
                        "  (selected)"
                    } else {
                        ""
                    },
                ));
                if let Some(error) = m["manifest_error"].as_str() {
                    out.push_str(&format!("             manifest: {error}\n"));
                }
            }
            out.push_str(&format!(
                "selected  {}  analysis {}  under {}\n",
                result["selected"].as_str().unwrap_or_default(),
                result["analysis_model"].as_str().unwrap_or_default(),
                result["models_dir"].as_str().unwrap_or_default(),
            ));
            if let Some(key) = result["polish_experiment"]
                .as_str()
                .filter(|k| !k.is_empty())
            {
                out.push_str(&format!(
                    "experiment  {key} (dettivo llm experiment status)\n"
                ));
            }
        }
        crate::experiment::STATUS_METHOD => out.push_str(&crate::experiment::render(result)),
        "llm.engine.status" => {
            let state = if result["degraded"] == Value::Bool(true) {
                format!("DEGRADED after {} crashes", result["crashes"])
            } else if result["running"] == Value::Bool(true) {
                format!(
                    "running{} on {}{}",
                    if result["busy"] == Value::Bool(true) {
                        " (busy)"
                    } else {
                        ""
                    },
                    result["backend"].as_str().unwrap_or("-"),
                    result["memory_bytes"]
                        .as_u64()
                        .map(|b| format!(", {} MiB on the device", b >> 20))
                        .unwrap_or_default()
                )
            } else if result["path"].is_string() {
                "found, not running".to_string()
            } else {
                "NOT FOUND".to_string()
            };
            out.push_str(&format!(
                "engine    {} {state}\nidle      unloaded after {} s\n",
                result["binary"].as_str().unwrap_or_default(),
                result["idle_seconds"]
            ));
            if let Some(reason) = result["reason"].as_str() {
                out.push_str(&format!("reason    {reason}\n"));
            }
            if let Some(lora) = result["lora"].as_str() {
                out.push_str(&format!("lora      {lora}\n"));
            }
        }
        "llm.endpoints.list" => {
            for endpoint in result["endpoints"].as_array()? {
                out.push_str(&format!(
                    "{}  {}\n",
                    if endpoint["loopback"] == Value::Bool(true) {
                        "local "
                    } else {
                        "remote"
                    },
                    endpoint["url"].as_str().unwrap_or_default(),
                ));
            }
        }
        _ => return None,
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_test_verb_prints_every_layer_it_ran() {
        let text = render(
            "polish.test",
            &json!({
                "raw": "Um update readme",
                "polished": "Update readme.",
                "output": "Update readme.",
                "model": "deterministic",
                "policy_hash": "7df425f2",
                "applied_rules": []
            }),
        )
        .unwrap();
        assert!(text.contains("raw       Um update readme"), "{text}");
        assert!(text.contains("inserted  Update readme."), "{text}");
        assert!(text.contains("policy 7df425f2"), "{text}");
        assert!(!text.contains("enhanced"), "{text}");
    }

    #[test]
    fn a_fallback_prints_the_notice_and_a_provider_prints_its_hint() {
        let text = render(
            "polish.test",
            &json!({
                "raw": "a", "polished": "A.", "output": "A.", "model": "deterministic",
                "notice": {"kind": "provider_unavailable", "reason": "nothing answered"},
                "applied_rules": []
            }),
        )
        .unwrap();
        assert!(
            text.contains("notice    provider_unavailable: nothing answered"),
            "{text}"
        );
        let text = render(
            "llm.providers.list",
            &json!({"providers": [
                {"id": "ollama", "model": "qwen3:4b", "available": false,
                 "detail": "nothing answered", "hint": "ollama pull qwen3:4b"}
            ]}),
        )
        .unwrap();
        assert!(text.contains("down  ollama"), "{text}");
        assert!(text.contains("ollama pull qwen3:4b"), "{text}");
    }

    #[test]
    fn a_method_without_a_rendering_falls_back_to_json() {
        assert_eq!(render("system.ping", &json!({})), None);
    }
}
