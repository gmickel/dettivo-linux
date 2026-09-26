//! One tool, one translation: each `tools/call` becomes the daemon
//! method the spec maps it to, with the argument defaults and clamps of
//! the macOS server. A daemon failure becomes the actionable tool text;
//! the daemon's result is the tool's payload.

use serde_json::{Map, Value, json};

use crate::bounds::MAX_ITEMS;
use crate::client::{Client, ClientError};
use crate::messages;
use crate::tools;

/// Why a tool could not answer.
#[derive(Debug)]
pub enum ToolError {
    /// The arguments do not fit the tool.
    InvalidParams(String),
    /// The daemon call failed.
    Daemon(ClientError),
    /// The server itself failed (a file, an encoding).
    Runtime(String),
}

impl From<ClientError> for ToolError {
    fn from(e: ClientError) -> Self {
        Self::Daemon(e)
    }
}

impl ToolError {
    /// The text an agent reads for `tool`.
    pub fn text(&self, tool: &str) -> String {
        match self {
            Self::InvalidParams(m) | Self::Runtime(m) => m.clone(),
            Self::Daemon(e) => messages::actionable(tool, e),
        }
    }
}

/// A non-empty string argument.
pub fn str_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// `str_arg` that tells absent from invalid: a value that is present
/// and not a string is an argument error, never a default.
pub fn typed_str_arg<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>, ToolError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(_)) => Ok(str_arg(args, key)),
        Some(other) => Err(ToolError::InvalidParams(format!(
            "{key} must be a string, not {}",
            kind_name(other)
        ))),
    }
}

fn kind_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// A limit clamped to `1..=MAX_ITEMS`, `default` when absent.
pub fn clamped_limit(args: &Value, default: u64) -> u64 {
    let limit = args
        .get("limit")
        .and_then(Value::as_i64)
        .unwrap_or(default as i64);
    limit.clamp(1, MAX_ITEMS as i64) as u64
}

fn kinds(args: &Value) -> Value {
    args.get("kinds")
        .filter(|k| k.is_array())
        .cloned()
        .unwrap_or_else(|| json!(["dictation", "meeting"]))
}

fn valid_query(query: &str) -> bool {
    !query.chars().any(|c| (c as u32) < 0x20 || c == '\u{7f}')
}

/// Calls `name` with `args` and returns the payload to bound.
pub fn call(client: &Client, name: &str, args: &Value) -> Result<Value, ToolError> {
    match name {
        "get_status" => {
            let mut health = client.call("system.health", json!({}))?;
            if let Ok(caps) = client.call("system.capabilities", json!({})) {
                health["capabilities"] = caps;
            }
            Ok(health)
        }
        "list_transcripts" => {
            let mut params =
                json!({"kinds": kinds(args), "limit": clamped_limit(args, 20), "cursor": null});
            if let Some(cursor) = str_arg(args, "cursor") {
                params["cursor"] = json!(cursor);
            }
            Ok(client.call("transcripts.list", params)?)
        }
        "get_transcript" => transcript_get(client, args),
        "search_transcripts" => {
            let query = str_arg(args, "query")
                .ok_or_else(|| ToolError::InvalidParams("query is required".into()))?;
            if !valid_query(query) {
                return Err(ToolError::InvalidParams(
                    "query contains unsupported control characters".into(),
                ));
            }
            Ok(client.call(
                "transcripts.search",
                json!({"query": query, "kinds": kinds(args), "limit": clamped_limit(args, 10)}),
            )?)
        }
        "get_latest_transcript" => {
            let kind = str_arg(args, "kind").unwrap_or("any");
            Ok(client.call("transcripts.latest", json!({"kind": kind}))?)
        }
        "start_dictation" => {
            let mut params = Map::new();
            params.insert(
                "language".into(),
                json!(str_arg(args, "language").unwrap_or("")),
            );
            params.insert("mode".into(), json!(str_arg(args, "mode").unwrap_or("raw")));
            Ok(client.call("dictation.start", Value::Object(params))?)
        }
        "stop_session" => session(client, args, "stop"),
        "cancel_session" => session(client, args, "cancel"),
        "start_meeting" => {
            let mut params = Map::new();
            params.insert(
                "capture".into(),
                args.get("capture")
                    .filter(|c| c.is_object())
                    .cloned()
                    .unwrap_or_else(|| json!({"system_audio": true, "microphone": true})),
            );
            if let Some(title) = str_arg(args, "title") {
                params.insert("title".into(), json!(title));
            }
            if let Some(ack) = args
                .get("acknowledge_meeting_disclosure")
                .and_then(Value::as_bool)
            {
                params.insert("acknowledge_meeting_disclosure".into(), json!(ack));
            }
            Ok(client.call("meetings.start", Value::Object(params))?)
        }
        "list_meetings" => {
            let mut params = json!({"limit": clamped_limit(args, 20)});
            if let Some(cursor) = str_arg(args, "cursor") {
                params["cursor"] = json!(cursor);
            }
            Ok(client.call("meetings.list", params)?)
        }
        "get_meeting" => {
            let id = str_arg(args, "meeting_id")
                .ok_or_else(|| ToolError::InvalidParams("meeting_id is required".into()))?;
            Ok(client.call("meetings.get", json!({"meeting_id": id}))?)
        }
        "search_meetings" => {
            let query = str_arg(args, "query")
                .ok_or_else(|| ToolError::InvalidParams("query is required".into()))?;
            Ok(client.call(
                "meetings.search",
                json!({"query": query, "limit": clamped_limit(args, 10)}),
            )?)
        }
        "get_meeting_segments" => {
            let id = str_arg(args, "meeting_id")
                .ok_or_else(|| ToolError::InvalidParams("meeting_id is required".into()))?;
            let mut params = json!({"meeting_id": id});
            if let Some(since) = typed_str_arg(args, "since")? {
                params["since"] = json!(since);
            }
            Ok(client.call("meetings.segments", params)?)
        }
        "import_audio" => tools::transfer::import_audio(client, args),
        "export_transcript" => tools::transfer::export_transcript(client, args),
        "insert_transcript" => insert(client, args),
        "list_polish_rules" => Ok(client.call("polish.rules.list", json!({}))?),
        "set_polish_app" => {
            let bundle_id = str_arg(args, "bundle_id")
                .ok_or_else(|| ToolError::InvalidParams("bundle_id is required".into()))?;
            let preset = str_arg(args, "preset")
                .ok_or_else(|| ToolError::InvalidParams("preset is required".into()))?;
            Ok(client.call(
                "polish.apps.set",
                json!({"bundle_id": bundle_id, "preset": preset}),
            )?)
        }
        "create_automation_job" => {
            let trigger = str_arg(args, "trigger")
                .ok_or_else(|| ToolError::InvalidParams("trigger is required".into()))?;
            let mut params = json!({"trigger": trigger});
            if let Some(id) = str_arg(args, "meeting_id") {
                params["meeting_id"] = json!(id);
            }
            if let Some(force) = args.get("force").and_then(Value::as_bool) {
                params["force"] = json!(force);
            }
            Ok(client.call("automation.jobs.create", params)?)
        }
        "list_automation_jobs" => Ok(client.call(
            "automation.jobs.list",
            json!({"limit": clamped_limit(args, 20)}),
        )?),
        other => Err(ToolError::InvalidParams(messages::unknown_tool(
            other,
            &tools::names(&tools::catalog()),
        ))),
    }
}

fn session(client: &Client, args: &Value, action: &str) -> Result<Value, ToolError> {
    // Only a non-empty meeting id routes to the meeting session; an empty
    // or absent one means the dictation session, and one of another type
    // is refused rather than read as absent.
    if let Some(id) = typed_str_arg(args, "meeting_id")? {
        let mut params = Map::new();
        params.insert("meeting_id".into(), json!(id));
        return Ok(client.call(&format!("meetings.{action}"), Value::Object(params))?);
    }
    Ok(client.call(&format!("dictation.{action}"), json!({}))?)
}

/// `transcripts.get` for an id: the given kind, or dictation first and
/// meeting second. A `NOT_FOUND` dictation stays `NOT_FOUND` when the
/// meeting kind is not implemented.
pub fn transcript_get(client: &Client, args: &Value) -> Result<Value, ToolError> {
    let id =
        str_arg(args, "id").ok_or_else(|| ToolError::InvalidParams("id is required".into()))?;
    if let Some(kind) = str_arg(args, "kind") {
        return Ok(client.call("transcripts.get", json!({"ref": {"kind": kind, "id": id}}))?);
    }
    match client.call(
        "transcripts.get",
        json!({"ref": {"kind": "dictation", "id": id}}),
    ) {
        Ok(v) => Ok(v),
        Err(first) => match client.call(
            "transcripts.get",
            json!({"ref": {"kind": "meeting", "id": id}}),
        ) {
            Ok(v) => Ok(v),
            Err(ClientError::Rpc(e))
                if e.app_code() == dettivo_proto::error::AppCode::NotImplemented =>
            {
                Err(first.into())
            }
            Err(second) => Err(second.into()),
        },
    }
}

fn insert(client: &Client, args: &Value) -> Result<Value, ToolError> {
    let mut params = Map::new();
    params.insert(
        "mode".into(),
        json!(str_arg(args, "mode").unwrap_or("polish")),
    );
    if let Some(text) = str_arg(args, "text") {
        params.insert("text".into(), json!(text));
    }
    if let Some(source) = args.get("source_ref").filter(|s| s.is_object()) {
        params.insert("source_ref".into(), source.clone());
    }
    if let Some(app) = str_arg(args, "expected_target_bundle_id") {
        params.insert("expected_target_bundle_id".into(), json!(app));
    }
    // The contract carries the pid as a string; the macOS schema takes an
    // integer, so both spellings reach the daemon as text.
    match args.get("expected_target_pid") {
        Some(Value::Number(n)) => {
            params.insert("expected_target_pid".into(), json!(n.to_string()));
        }
        Some(Value::String(s)) if !s.is_empty() => {
            params.insert("expected_target_pid".into(), json!(s));
        }
        _ => {}
    }
    Ok(client.call("insert.perform", Value::Object(params))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_clamp_and_queries_reject_control_characters() {
        assert_eq!(clamped_limit(&json!({}), 20), 20);
        assert_eq!(clamped_limit(&json!({"limit": 0}), 20), 1);
        assert_eq!(clamped_limit(&json!({"limit": 900}), 20), MAX_ITEMS as u64);
        assert!(valid_query("hello world"));
        assert!(!valid_query("hello\u{7}"));
        assert_eq!(str_arg(&json!({"a": ""}), "a"), None);
        assert_eq!(str_arg(&json!({"a": "x"}), "a"), Some("x"));
    }

    /// agent-surfaces/F4: a numeric `meeting_id` does not fall through to
    /// the dictation session; the call is refused before the daemon.
    #[test]
    fn a_meeting_id_of_the_wrong_type_is_refused_instead_of_cancelling_dictation() {
        let client = Client {
            socket: std::path::PathBuf::from("/nonexistent.sock"),
            token: None,
            timeout: std::time::Duration::from_millis(50),
        };
        let err = call(&client, "cancel_session", &json!({"meeting_id": 42})).unwrap_err();
        assert!(
            matches!(&err, ToolError::InvalidParams(m) if m == "meeting_id must be a string, not a number"),
            "{err:?}"
        );
        let err = call(&client, "cancel_session", &json!({})).unwrap_err();
        assert!(
            matches!(err, ToolError::Daemon(_)),
            "absent still means dictation: {err:?}"
        );
        let err = call(&client, "cancel_session", &json!({"meeting_id": null})).unwrap_err();
        assert!(
            matches!(err, ToolError::Daemon(_)),
            "null still means dictation: {err:?}"
        );
    }

    #[test]
    fn an_unknown_tool_names_the_known_ones_without_a_daemon() {
        let client = Client {
            socket: std::path::PathBuf::from("/nonexistent.sock"),
            token: None,
            timeout: std::time::Duration::from_millis(50),
        };
        let err = call(&client, "frob", &json!({})).unwrap_err();
        let text = err.text("frob");
        assert!(
            text.starts_with("Unknown tool: frob. Available tools: get_status, "),
            "{text}"
        );
        assert!(text.ends_with("list_automation_jobs"), "{text}");
    }
}
