//! `status`, `config`, `dictation`, `events`, `hotkeys` and `call`: each
//! subcommand is one contract method (`speech` lives in its own module).

use clap::Subcommand;
use serde_json::{Value, json};

use crate::client::Client;
use crate::exit::{Exit, Failure};
use crate::{Cli, output};

/// `dettivo status <what>`: the `system.*` methods.
#[derive(Debug, Subcommand)]
pub enum StatusCmd {
    /// system.ping
    Ping,
    /// system.health
    Health,
    /// system.version
    Version,
    /// system.capabilities
    Capabilities,
}

/// `dettivo config <what>`: the `config.*` methods plus `edit`.
#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Print one key, or every key, with its effective value and source.
    Get {
        /// Dotted key, e.g. daemon.log_level.
        key: Option<String>,
    },
    /// Write one key; the file keeps its comments and ordering.
    Set {
        /// Dotted key.
        key: String,
        /// The value, as you would type it in the file (quotes optional).
        value: String,
    },
    /// Remove one key so its default applies.
    Unset {
        /// Dotted key.
        key: String,
    },
    /// Print every path the daemon resolved.
    Path,
    /// Open config.toml in $VISUAL or $EDITOR, then validate it.
    Edit,
    /// Validate the file on disk, naming the key and line of a problem.
    Validate,
    /// Print the fully commented default file.
    PrintDefault,
    /// List every key with its section, type, default and meaning.
    Keys,
}

/// Runs a `status` subcommand.
pub fn status(cli: &Cli, client: &Client, what: &StatusCmd) -> Result<(), Failure> {
    let method = match what {
        StatusCmd::Ping => "system.ping",
        StatusCmd::Health => "system.health",
        StatusCmd::Version => "system.version",
        StatusCmd::Capabilities => "system.capabilities",
    };
    let result = client.call(method, json!({}))?;
    output::result(cli, method, &result);
    Ok(())
}

/// Runs a `config` subcommand.
pub fn config(cli: &Cli, client: &Client, what: &ConfigCmd) -> Result<(), Failure> {
    let (method, params) = match what {
        ConfigCmd::Get { key } => ("config.get", json!({ "key": key })),
        ConfigCmd::Set { key, value } => ("config.set", json!({ "key": key, "value": value })),
        ConfigCmd::Unset { key } => ("config.unset", json!({ "key": key })),
        ConfigCmd::Path => ("config.path", json!({})),
        ConfigCmd::Validate => ("config.validate", json!({})),
        ConfigCmd::PrintDefault => ("config.print_default", json!({})),
        ConfigCmd::Keys => ("config.keys", json!({})),
        ConfigCmd::Edit => return edit(cli, client),
    };
    let params = strip_null_key(params);
    let result = client.call(method, params)?;
    if method == "config.validate" && result.get("ok") != Some(&Value::Bool(true)) {
        output::result(cli, method, &result);
        return Err(Failure::reported(Exit::Failure, "configuration is invalid"));
    }
    output::result(cli, method, &result);
    Ok(())
}

/// Drops every null member: an absent optional field, not a null one.
pub fn strip_nulls(mut params: Value) -> Value {
    if let Some(obj) = params.as_object_mut() {
        obj.retain(|_, v| !v.is_null());
    }
    params
}

/// `dettivo hotkeys <what>`.
#[derive(Debug, Subcommand)]
pub enum HotkeysCmd {
    /// hotkeys.status: the backend in force, portal and evdev availability, bound actions.
    Status,
}

/// Runs a `hotkeys` subcommand.
pub fn hotkeys(cli: &Cli, client: &Client, what: &HotkeysCmd) -> Result<(), Failure> {
    let method = match what {
        HotkeysCmd::Status => "hotkeys.status",
    };
    let result = client.call(method, json!({}))?;
    output::result(cli, method, &result);
    Ok(())
}

/// `config.get` without a key sends `{}`; a JSON null would be a type error.
pub fn strip_null_key(mut params: Value) -> Value {
    if let Some(obj) = params.as_object_mut() {
        if obj.get("key").is_some_and(Value::is_null) {
            obj.remove("key");
        }
    }
    params
}

/// `config edit`: open the file the daemon names (or the default location
/// when no daemon runs), then validate through the daemon.
fn edit(cli: &Cli, client: &Client) -> Result<(), Failure> {
    let path = match client.call("config.path", json!({})) {
        Ok(v) => v
            .get("config")
            .and_then(Value::as_str)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| crate::paths::config_file(|k| std::env::var_os(k))),
        Err(f) if f.exit == Exit::Unavailable => crate::paths::config_file(|k| std::env::var_os(k)),
        Err(f) => return Err(f),
    };
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                Failure::new(
                    Exit::Failure,
                    format!("cannot create {}: {e}", parent.display()),
                )
            })?;
        }
        std::fs::write(&path, dettivo_default_text(client)).map_err(|e| {
            Failure::new(
                Exit::Failure,
                format!("cannot create {}: {e}", path.display()),
            )
        })?;
    }
    let editor = ["VISUAL", "EDITOR"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|e| !e.trim().is_empty())
        .unwrap_or_else(|| "vi".to_string());
    // The editor value is split on whitespace and executed directly, never
    // through a shell: `code --wait` works, and no character in the
    // variable can become shell syntax.
    let mut words = editor.split_whitespace();
    let program = words
        .next()
        .ok_or_else(|| Failure::new(Exit::Failure, "VISUAL/EDITOR is empty"))?;
    let status = std::process::Command::new(program)
        .args(words)
        .arg(&path)
        .status()
        .map_err(|e| Failure::new(Exit::Failure, format!("cannot run {editor}: {e}")))?;
    if !status.success() {
        return Err(Failure::new(
            Exit::Failure,
            format!("{editor} exited with {status}"),
        ));
    }
    match client.call("config.validate", json!({})) {
        Ok(result) => {
            output::result(cli, "config.validate", &result);
            if result.get("ok") == Some(&Value::Bool(true)) {
                Ok(())
            } else {
                Err(Failure::reported(Exit::Failure, "configuration is invalid"))
            }
        }
        Err(f) if f.exit == Exit::Unavailable => {
            if !cli.quiet {
                println!(
                    "edited {} (no daemon running to validate it)",
                    path.display()
                );
            }
            Ok(())
        }
        Err(f) => Err(f),
    }
}

/// The default file text from the daemon, or a one-line stub without one.
fn dettivo_default_text(client: &Client) -> String {
    client
        .call("config.print_default", json!({}))
        .ok()
        .and_then(|v| v.get("text").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| "# Dettivo configuration (see docs/config.md)\n".to_string())
}

/// `dettivo call <method> [params]`: any contract method, 1:1.
pub fn call(cli: &Cli, client: &Client, method: &str, params: Option<&str>) -> Result<(), Failure> {
    let params: Value = match params {
        None => json!({}),
        Some(text) => serde_json::from_str(text)
            .map_err(|e| Failure::new(Exit::InvalidArgs, format!("params are not JSON: {e}")))?,
    };
    if !params.is_object() {
        return Err(Failure::new(
            Exit::InvalidArgs,
            "params must be a JSON object",
        ));
    }
    let result = client.call(method, params)?;
    output::result(cli, method, &result);
    Ok(())
}

/// `dettivo dictation <verb>`: the `dictation.*` methods plus `toggle`.
#[derive(Debug, Subcommand)]
pub enum DictationCmd {
    /// dictation.start
    Start {
        /// Language code or auto (default: [dictation] language).
        #[arg(long)]
        language: Option<String>,
        /// raw, deterministic_polish (alias polish) or enhanced.
        #[arg(long)]
        mode: Option<String>,
        /// The app id the text must land in (captured at hotkey time); the
        /// daemon probes the focused window itself without it.
        #[arg(long, value_name = "APP_ID")]
        expected_target_bundle_id: Option<String>,
        /// The pid of the window the text must land in.
        #[arg(long, value_name = "PID")]
        expected_target_pid: Option<String>,
    },
    /// dictation.stop: finish the take, transcribe and insert.
    Stop,
    /// dictation.cancel: drop the session.
    Cancel,
    /// dictation.status
    Status,
    /// Start when idle, stop when a session runs (Linux addition).
    Toggle,
    /// dictation.reinsert_last: insert the last transcript again (Linux addition).
    ReinsertLast,
}

/// The `dictation.start` params. A mode the binding did not name stays
/// off the wire, so the daemon applies its configured `[dictation] mode`
/// exactly as it does for a portal or evdev hotkey; `--mode raw` is still
/// an explicit override.
pub fn start_params(
    language: Option<&str>,
    mode: Option<&str>,
    expected_target_bundle_id: Option<&str>,
    expected_target_pid: Option<&str>,
) -> Value {
    strip_nulls(json!({
        "language": language.unwrap_or_default(),
        "mode": mode,
        "expected_target_bundle_id": expected_target_bundle_id,
        "expected_target_pid": expected_target_pid,
    }))
}

/// Runs a `dictation` subcommand.
pub fn dictation(cli: &Cli, client: &Client, what: &DictationCmd) -> Result<(), Failure> {
    let (method, params) = match what {
        DictationCmd::Start {
            language,
            mode,
            expected_target_bundle_id,
            expected_target_pid,
        } => (
            "dictation.start",
            start_params(
                language.as_deref(),
                mode.as_deref(),
                expected_target_bundle_id.as_deref(),
                expected_target_pid.as_deref(),
            ),
        ),
        DictationCmd::Stop => ("dictation.stop", json!({})),
        DictationCmd::Cancel => ("dictation.cancel", json!({})),
        DictationCmd::Status => ("dictation.status", json!({})),
        DictationCmd::ReinsertLast => ("dictation.reinsert_last", json!({})),
        DictationCmd::Toggle => ("dictation.toggle", json!({})),
    };
    let result = client.call(method, params)?;
    output::result(cli, method, &result);
    Ok(())
}

/// `dettivo events --follow`: subscribes and prints each notification as
/// one JSON line (the `params` object) until the daemon closes the
/// connection or `count` events arrived.
pub fn events(cli: &Cli, client: &Client, topics: &[String], count: u64) -> Result<(), Failure> {
    let topics: Vec<Value> = if topics.is_empty() {
        [
            "dictation.state",
            "meeting.state",
            "job.progress",
            "audio.level",
            "model.download",
            "engine.state",
            "meeting.segment",
        ]
        .iter()
        .map(|t| Value::String((*t).to_string()))
        .collect()
    } else {
        topics.iter().map(|t| Value::String(t.clone())).collect()
    };
    let mut seen = 0u64;
    client.stream(
        "events.subscribe",
        json!({ "topics": topics, "buffer": 256 }),
        |line| {
            let parsed: Value = serde_json::from_str(line).unwrap_or(Value::Null);
            if parsed["method"] != "events.notify" {
                return true;
            }
            if cli.json {
                println!("{}", parsed["params"]);
            } else if !cli.quiet {
                let p = &parsed["params"];
                println!(
                    "{} {} {}",
                    p["timestamp"].as_str().unwrap_or(""),
                    p["topic"].as_str().unwrap_or(""),
                    p["payload"]
                );
            }
            seen += 1;
            count == 0 || seen < count
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_start_without_a_mode_leaves_the_mode_to_the_daemon() {
        let bare = start_params(None, None, None, None);
        assert_eq!(bare, json!({"language": ""}));
        let explicit = start_params(Some("en"), Some("raw"), Some("foot"), Some("42"));
        assert_eq!(
            explicit,
            json!({"language": "en", "mode": "raw", "expected_target_bundle_id": "foot", "expected_target_pid": "42"})
        );
    }
}
