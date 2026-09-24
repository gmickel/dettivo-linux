//! Human and `--json` rendering. Human output is short lines a person
//! scans; `--json` preserves daemon results/errors and adds structured
//! adapter errors so a script can pipe either origin to `jq`.

use serde_json::Value;

use crate::Cli;
use crate::exit::Failure;

/// Prints a method result according to the flags.
pub fn result(cli: &Cli, method: &str, value: &Value) {
    if cli.quiet {
        return;
    }
    if cli.json {
        println!("{value}");
        return;
    }
    print!("{}", human(method, value));
}

/// The human rendering of a result, ending in a newline.
pub fn human(method: &str, value: &Value) -> String {
    match method {
        "system.ping" => "ok\n".to_string(),
        "config.print_default" => value
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        "config.get" => {
            let mut out = String::new();
            if let Some(entries) = value.get("entries").and_then(Value::as_array) {
                for e in entries {
                    out.push_str(&entry_line(e));
                }
            }
            out
        }
        "config.set" | "config.unset" => entry_line(value),
        "config.keys" => keys_lines(value),
        "config.validate" => validate_lines(value),
        "insert.perform" | "insert.undo" | "insert.target" => crate::insert::human(method, value),
        "hotkeys.status" => hotkeys_lines(value),
        _ => crate::polish::render(method, value).unwrap_or_else(|| flat_lines(value)),
    }
}

fn entry_line(e: &Value) -> String {
    format!(
        "{} = {}  ({})\n",
        e.get("key").and_then(Value::as_str).unwrap_or("?"),
        e.get("value").unwrap_or(&Value::Null),
        e.get("source").and_then(Value::as_str).unwrap_or("?")
    )
}

/// One line per key: the key, its type, its default and its sentence.
fn keys_lines(v: &Value) -> String {
    let mut out = String::new();
    if let Some(keys) = v.get("keys").and_then(Value::as_array) {
        for k in keys {
            out.push_str(&format!(
                "{}  {}  default {}  {}\n",
                k.get("key").and_then(Value::as_str).unwrap_or("?"),
                k.get("kind").and_then(Value::as_str).unwrap_or("?"),
                k.get("default").unwrap_or(&Value::Null),
                k.get("doc").and_then(Value::as_str).unwrap_or("")
            ));
        }
    }
    out
}

fn validate_lines(v: &Value) -> String {
    let path = v.get("path").and_then(Value::as_str).unwrap_or("?");
    let errors = v.get("errors").and_then(Value::as_array);
    match errors {
        Some(errors) if !errors.is_empty() => errors
            .iter()
            .map(|e| {
                let line = e
                    .get("line")
                    .and_then(Value::as_u64)
                    .map(|l| format!(":{l}"))
                    .unwrap_or_default();
                let key = e
                    .get("key")
                    .and_then(Value::as_str)
                    .map(|k| format!(" {k}:"))
                    .unwrap_or_default();
                format!(
                    "{path}{line}:{key} {}\n",
                    e.get("message").and_then(Value::as_str).unwrap_or("")
                )
            })
            .collect(),
        _ => format!("ok: {path}\n"),
    }
}

/// The `hotkeys.status` lines: the backend, each path with its reason,
/// the bound actions.
pub fn hotkeys_lines(v: &Value) -> String {
    let avail = |block: &Value| -> String {
        if block["available"] == Value::Bool(true) {
            "available".to_string()
        } else {
            format!(
                "unavailable: {}",
                block["reason"].as_str().unwrap_or("no reason given")
            )
        }
    };
    let mut out = format!(
        "backend   {} (requested {})\nportal    {}\nevdev     {}\n",
        v["backend"].as_str().unwrap_or("?"),
        v["requested"].as_str().unwrap_or("?"),
        avail(&v["portal"]),
        avail(&v["evdev"])
    );
    let bound: Vec<&str> = v["bound"]
        .as_array()
        .map(|b| b.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    out.push_str(&format!(
        "bound     {}\n",
        if bound.is_empty() {
            "nothing (compositor bindings call the CLI)".to_string()
        } else {
            bound.join(", ")
        }
    ));
    if let Some(error) = v["error"].as_str() {
        out.push_str(&format!("error     {error}\n"));
    }
    out
}

/// `key: value` per field for a flat object, `key.sub: value` for one
/// block of scalars inside it (the `theme` block of a status); pretty JSON
/// for anything deeper, so `system.capabilities` stays readable.
pub fn flat_lines(v: &Value) -> String {
    match v.as_object() {
        Some(obj) if obj.values().all(is_flat) => obj
            .iter()
            .flat_map(|(k, x)| match x.as_object() {
                Some(inner) => inner
                    .iter()
                    .map(|(ik, ix)| format!("{k}.{ik}: {}\n", scalar(ix)))
                    .collect::<Vec<_>>(),
                None => vec![format!("{k}: {}\n", scalar(x))],
            })
            .collect(),
        _ => format!(
            "{}\n",
            serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
        ),
    }
}

fn is_flat(v: &Value) -> bool {
    match v {
        Value::Object(inner) => inner.values().all(|x| !x.is_object() && !x.is_array()),
        Value::Array(_) => false,
        _ => true,
    }
}

fn scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "-".to_string(),
        other => other.to_string(),
    }
}

/// Reports a failure on stderr and supplies a JSON error unless the command
/// already rendered its structured diagnostic report.
pub fn failure(cli: &Cli, failure: &Failure) {
    eprintln!("dettivo: {}", failure.message);
    failure_json(cli.json, cli.quiet, failure);
}

/// The JSON error body, shared with command parsing before a `Cli` exists.
pub fn failure_json(json: bool, quiet: bool, failure: &Failure) {
    if json && !quiet && !failure.reported {
        let error = match &failure.rpc {
            Some(rpc) => serde_json::to_value(rpc).expect("JSON-RPC errors serialize"),
            None => serde_json::json!({"code": -32603, "message": failure.message,
                "data": {"origin": "cli", "exit_code": failure.exit.number()}}),
        };
        println!("{}", serde_json::json!({"error": error}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_entries_validation_and_flat_objects() {
        assert_eq!(human("system.ping", &json!({"ok": true})), "ok\n");
        assert_eq!(
            human(
                "config.get",
                &json!({"entries":[{"key":"qa.mode","value":false,"source":"default"}]})
            ),
            "qa.mode = false  (default)\n"
        );
        assert_eq!(
            human(
                "config.validate",
                &json!({"ok":false,"path":"/c.toml","errors":[{"key":"daemon.log_level","line":2,"message":"unknown variant"}]})
            ),
            "/c.toml:2: daemon.log_level: unknown variant\n"
        );
        assert_eq!(
            human(
                "config.validate",
                &json!({"ok":true,"path":"/c.toml","errors":[]})
            ),
            "ok: /c.toml\n"
        );
        assert_eq!(
            human(
                "system.health",
                &json!({"ok":true,"recording_state":"idle"})
            ),
            "ok: true\nrecording_state: idle\n"
        );
        assert_eq!(
            human("system.capabilities", &json!({"auth":{"ipc_mode":"peer"}})),
            "auth.ipc_mode: peer\n"
        );
        assert!(
            human(
                "system.capabilities",
                &json!({"speech":{"providers":{"whisper":{"meeting_capable":true}}}})
            )
            .contains("\"meeting_capable\": true")
        );
        let status = human(
            "hotkeys.status",
            &json!({"backend":"none","requested":"auto","portal":{"available":false,"reason":"no portal"},"evdev":{"available":false,"reason":"needs the input group"},"bound":[],"error":null}),
        );
        assert_eq!(
            status,
            "backend   none (requested auto)\nportal    unavailable: no portal\nevdev     unavailable: needs the input group\nbound     nothing (compositor bindings call the CLI)\n"
        );
    }
}
