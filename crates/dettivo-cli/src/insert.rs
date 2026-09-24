//! `dettivo insert`: `insert.perform` with flags, plus the Linux additions
//! `insert undo` and `insert target`. The text comes from `--text` (or
//! standard input with `--text -`, for a compositor binding that pipes)
//! or from a history reference with `--kind` and `--id`.

use std::io::Read;

use clap::{Args, Subcommand};
use serde_json::{Map, Value, json};

use crate::client::Client;
use crate::exit::{Exit, Failure};
use crate::{Cli, output};

/// `dettivo insert undo | target`.
#[derive(Debug, Subcommand)]
pub enum InsertCmd {
    /// Take the last insertion back (insert.undo, within the undo window).
    Undo,
    /// Show the focused window and every backend's availability (insert.target).
    Target,
}

/// The `insert.perform` flags.
#[derive(Debug, Args)]
pub struct InsertArgs {
    /// Insertion mode: raw, polish or clipboard_only.
    #[arg(long, default_value = "raw", value_name = "MODE")]
    pub mode: String,
    /// The text to insert; `-` reads standard input.
    #[arg(long, value_name = "TEXT")]
    pub text: Option<String>,
    /// Insert a history item instead: its kind (dictation or meeting).
    #[arg(long, value_name = "KIND", requires = "id")]
    pub kind: Option<String>,
    /// The history item's id (with --kind).
    #[arg(long, value_name = "UUID", requires = "kind")]
    pub id: Option<String>,
    /// Refuse unless the focused app id matches (captured at hotkey time).
    #[arg(long, value_name = "APP_ID")]
    pub expected_target_bundle_id: Option<String>,
    /// Refuse unless the focused window's pid matches.
    #[arg(long, value_name = "PID")]
    pub expected_target_pid: Option<String>,
}

/// Runs `dettivo insert ...`.
pub fn run(
    cli: &Cli,
    client: &Client,
    what: Option<&InsertCmd>,
    args: &InsertArgs,
) -> Result<(), Failure> {
    let (method, params) = match what {
        Some(InsertCmd::Undo) => ("insert.undo", json!({})),
        Some(InsertCmd::Target) => ("insert.target", json!({})),
        None => ("insert.perform", perform_params(args)?),
    };
    let result = client.call(method, params)?;
    output::result(cli, method, &result);
    if method == "insert.perform" && result.get("outcome") == Some(&Value::String("failed".into()))
    {
        return Err(Failure::new(
            Exit::Failure,
            format!(
                "insertion failed: {}",
                result
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("no reason")
            ),
        ));
    }
    Ok(())
}

fn perform_params(args: &InsertArgs) -> Result<Value, Failure> {
    if !["raw", "polish", "clipboard_only"].contains(&args.mode.as_str()) {
        return Err(Failure::new(
            Exit::InvalidArgs,
            "--mode must be raw, polish or clipboard_only",
        ));
    }
    let mut params = Map::new();
    params.insert("mode".into(), Value::String(args.mode.clone()));
    match (&args.text, &args.kind, &args.id) {
        (Some(text), _, _) => {
            let text = if text == "-" {
                let mut buf = String::new();
                std::io::stdin()
                    .read_to_string(&mut buf)
                    .map_err(|e| Failure::new(Exit::Failure, format!("cannot read stdin: {e}")))?;
                buf
            } else {
                text.clone()
            };
            params.insert("text".into(), Value::String(text));
        }
        (None, Some(kind), Some(id)) => {
            params.insert("source_ref".into(), json!({"kind": kind, "id": id}));
        }
        _ => {
            return Err(Failure::new(
                Exit::InvalidArgs,
                "give --text <TEXT> (or --text -) or --kind <KIND> --id <UUID>",
            ));
        }
    }
    if let Some(app) = &args.expected_target_bundle_id {
        params.insert(
            "expected_target_bundle_id".into(),
            Value::String(app.clone()),
        );
    }
    if let Some(pid) = &args.expected_target_pid {
        params.insert("expected_target_pid".into(), Value::String(pid.clone()));
    }
    Ok(Value::Object(params))
}

/// Human rendering of the three results.
pub fn human(method: &str, value: &Value) -> String {
    let text = |v: &Value| v.as_str().unwrap_or("-").to_string();
    match method {
        "insert.perform" => {
            let backend = &value["backend"];
            let via = if backend.is_object() {
                format!(
                    " via {} ({} ms{})",
                    text(&backend["name"]),
                    backend["latency_ms"].as_u64().unwrap_or(0),
                    if backend["undo_supported"] == Value::Bool(true) {
                        ", undo available"
                    } else {
                        ""
                    }
                )
            } else {
                String::new()
            };
            let target = text(&value["target_app"]["bundle_id"]);
            match value["outcome"].as_str() {
                Some("inserted") => format!("inserted into {target}{via}\n"),
                Some("copied_to_clipboard") => format!(
                    "copied to clipboard{via}{}\n",
                    value["reason"]
                        .as_str()
                        .map(|r| format!(" ({r})"))
                        .unwrap_or_default()
                ),
                _ => format!("failed: {}\n", text(&value["reason"])),
            }
        }
        "insert.undo" => {
            if value["undone"] == Value::Bool(true) {
                "undone\n".to_string()
            } else {
                format!("not undone: {}\n", text(&value["reason"]))
            }
        }
        "insert.target" => {
            let mut out = String::new();
            match value["target"].as_object() {
                Some(t) => out.push_str(&format!(
                    "target    {} pid {}{} (probe {})\n",
                    text(&t["app_id"]),
                    t["pid"]
                        .as_u64()
                        .map(|p| p.to_string())
                        .unwrap_or("-".into()),
                    if t["is_dettivo"] == Value::Bool(true) {
                        ", a Dettivo window"
                    } else {
                        ""
                    },
                    text(&value["probe"])
                )),
                None => out.push_str(&format!(
                    "target    none (probe {})\n",
                    text(&value["probe"])
                )),
            }
            out.push_str(&backend_lines(&value["backends"]));
            out.push_str(&format!("chosen    {}\n", text(&value["chosen"])));
            out
        }
        _ => output::flat_lines(value),
    }
}

/// One line per backend: name, availability, reason.
pub fn backend_lines(backends: &Value) -> String {
    backends
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|b| {
                    format!(
                        "backend   {:<17} {}{}\n",
                        b["name"].as_str().unwrap_or("?"),
                        if b["available"] == Value::Bool(true) {
                            "available"
                        } else {
                            "unavailable"
                        },
                        b["reason"]
                            .as_str()
                            .map(|r| format!(": {r}"))
                            .unwrap_or_default()
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_perform_undo_and_target() {
        let performed = json!({"outcome": "inserted", "target_app": {"bundle_id": "foot"}, "backend": {"name": "virtual_keyboard", "latency_ms": 12, "undo_supported": true}});
        assert_eq!(
            human("insert.perform", &performed),
            "inserted into foot via virtual_keyboard (12 ms, undo available)\n"
        );
        let failed = json!({"outcome": "failed", "reason": "target_is_self"});
        assert_eq!(human("insert.perform", &failed), "failed: target_is_self\n");
        assert_eq!(
            human(
                "insert.undo",
                &json!({"undone": false, "reason": "expired"})
            ),
            "not undone: expired\n"
        );
        let target = json!({"target": {"app_id": "foot", "pid": 7, "is_dettivo": false}, "probe": "hyprland", "backends": [{"name": "libei", "available": false, "reason": "no portal"}], "chosen": "clipboard"});
        let text = human("insert.target", &target);
        assert!(
            text.starts_with("target    foot pid 7 (probe hyprland)\n"),
            "{text}"
        );
        assert!(
            text.contains("backend   libei             unavailable: no portal\n"),
            "{text}"
        );
        assert!(text.ends_with("chosen    clipboard\n"), "{text}");
    }

    #[test]
    fn perform_params_need_text_or_a_reference() {
        let args = |text: Option<&str>, kind: Option<&str>, id: Option<&str>| InsertArgs {
            mode: "raw".into(),
            text: text.map(str::to_string),
            kind: kind.map(str::to_string),
            id: id.map(str::to_string),
            expected_target_bundle_id: Some("foot".into()),
            expected_target_pid: None,
        };
        let p = perform_params(&args(Some("hi"), None, None)).unwrap();
        assert_eq!(p["text"], "hi");
        assert_eq!(p["expected_target_bundle_id"], "foot");
        let p = perform_params(&args(None, Some("dictation"), Some("x"))).unwrap();
        assert_eq!(p["source_ref"]["kind"], "dictation");
        assert_eq!(
            perform_params(&args(None, None, None)).unwrap_err().exit,
            Exit::InvalidArgs
        );
        let mut bad = args(Some("hi"), None, None);
        bad.mode = "loud".into();
        assert_eq!(perform_params(&bad).unwrap_err().exit, Exit::InvalidArgs);
    }
}
