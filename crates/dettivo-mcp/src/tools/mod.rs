//! The nineteen tools, with the names, titles, descriptions and input
//! schemas of the macOS server (`DettivoCLI/MCP/MCPTypes.swift`), so an
//! agent instruction written for one port drives the other. The four
//! meeting tools are hidden while `formats.meeting_export` is empty, as
//! on macOS; `dispatch` maps each tool to its daemon methods.

pub mod dispatch;
pub mod transfer;

use serde_json::{Map, Value, json};

use crate::bounds::MAX_ITEMS;

/// One tool as `tools/list` reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    /// The tool name an agent calls.
    pub name: &'static str,
    /// The display title.
    pub title: &'static str,
    /// One line on what it does.
    pub description: &'static str,
    /// The JSON schema of `arguments`.
    pub input_schema: Value,
}

impl Tool {
    /// The `tools/list` entry.
    pub fn definition(&self) -> Value {
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": self.input_schema,
        })
    }
}

/// The tools hidden while the daemon declares no meeting export format.
pub const MEETING_TOOLS: &[&str] = &[
    "start_meeting",
    "list_meetings",
    "get_meeting",
    "search_meetings",
];

fn kinds() -> Value {
    json!({"type": "array", "items": {"type": "string", "enum": ["dictation", "meeting"]}})
}

fn limit() -> Value {
    json!({"type": "integer", "minimum": 1, "maximum": MAX_ITEMS})
}

fn reference() -> Value {
    json!({
        "type": "object",
        "properties": {
            "kind": {"type": "string", "enum": ["dictation", "meeting"]},
            "id": {"type": "string"},
        },
        "required": ["kind", "id"],
        "additionalProperties": false,
    })
}

fn schema(properties: Value, required: &[&str]) -> Value {
    let mut out = Map::new();
    out.insert("type".into(), json!("object"));
    out.insert("properties".into(), properties);
    out.insert("additionalProperties".into(), json!(false));
    if !required.is_empty() {
        out.insert("required".into(), json!(required));
    }
    Value::Object(out)
}

fn tool(
    name: &'static str,
    title: &'static str,
    description: &'static str,
    input_schema: Value,
) -> Tool {
    Tool {
        name,
        title,
        description,
        input_schema,
    }
}

/// Every tool in the macOS order.
pub fn catalog() -> Vec<Tool> {
    vec![
        tool(
            "get_status",
            "Get Status",
            "Get app and recording health state.",
            schema(json!({}), &[]),
        ),
        tool(
            "list_transcripts",
            "List Transcripts",
            "List transcript history (dictation + meetings).",
            schema(
                json!({"kinds": kinds(), "limit": limit(), "cursor": {"type": "string"}}),
                &[],
            ),
        ),
        tool(
            "get_transcript",
            "Get Transcript",
            "Get one transcript by id and optional kind.",
            schema(
                json!({"id": {"type": "string"}, "kind": {"type": "string", "enum": ["dictation", "meeting"]}}),
                &["id"],
            ),
        ),
        tool(
            "search_transcripts",
            "Search Transcripts",
            "Search transcript text by keyword.",
            schema(
                json!({"query": {"type": "string"}, "kinds": kinds(), "limit": limit()}),
                &["query"],
            ),
        ),
        tool(
            "get_latest_transcript",
            "Get Latest Transcript",
            "Get latest transcript ref for dictation/meeting/any.",
            schema(
                json!({"kind": {"type": "string", "enum": ["dictation", "meeting", "any"]}}),
                &[],
            ),
        ),
        tool(
            "start_dictation",
            "Start Dictation",
            "Start dictation capture.",
            schema(
                json!({"language": {"type": "string"}, "mode": {"type": "string", "enum": ["raw", "polish"]}}),
                &[],
            ),
        ),
        tool(
            "stop_session",
            "Stop Session",
            "Stop dictation or meeting recording.",
            schema(json!({"meeting_id": {"type": "string"}}), &[]),
        ),
        tool(
            "cancel_session",
            "Cancel Session",
            "Cancel dictation or meeting recording.",
            schema(json!({"meeting_id": {"type": "string"}}), &[]),
        ),
        tool(
            "start_meeting",
            "Start Meeting",
            "Start meeting capture with system audio/microphone.",
            schema(
                json!({
                    "title": {"type": "string"},
                    "capture": {
                        "type": "object",
                        "properties": {"system_audio": {"type": "boolean"}, "microphone": {"type": "boolean"}},
                        "additionalProperties": false,
                    },
                    "acknowledge_meeting_disclosure": {"type": "boolean"},
                }),
                &[],
            ),
        ),
        tool(
            "list_meetings",
            "List Meetings",
            "List meeting sessions.",
            schema(json!({"limit": limit(), "cursor": {"type": "string"}}), &[]),
        ),
        tool(
            "get_meeting",
            "Get Meeting",
            "Get a single meeting session.",
            schema(json!({"meeting_id": {"type": "string"}}), &["meeting_id"]),
        ),
        tool(
            "search_meetings",
            "Search Meetings",
            "Search meeting transcripts.",
            schema(
                json!({"query": {"type": "string"}, "limit": limit()}),
                &["query"],
            ),
        ),
        tool(
            "import_audio",
            "Import Audio",
            "Upload local audio file and dettivo using transfer/import pipeline.",
            schema(
                json!({
                    "file_path": {"type": "string"},
                    "target_kind": {"type": "string", "enum": ["dictation", "meeting"]},
                    "language": {"type": "string"},
                    "mode": {"type": "string", "enum": ["raw", "polish"]},
                }),
                &["file_path"],
            ),
        ),
        tool(
            "export_transcript",
            "Export Transcript",
            "Export transcript via transfer pipeline to a file.",
            schema(
                json!({
                    "ref": reference(),
                    "format": {"type": "string", "enum": ["txt", "md", "json", "srt", "vtt"]},
                    "out_path": {"type": "string"},
                }),
                &["ref", "format"],
            ),
        ),
        tool(
            "insert_transcript",
            "Insert Transcript",
            "Insert text or source transcript into focused app.",
            schema(
                json!({
                    "mode": {"type": "string", "enum": ["raw", "polish", "clipboard_only"]},
                    "text": {"type": "string"},
                    "source_ref": reference(),
                    "expected_target_bundle_id": {"type": "string"},
                    "expected_target_pid": {"type": "integer"},
                }),
                &[],
            ),
        ),
        tool(
            "list_polish_rules",
            "List Polish Rules",
            "List custom and built-in polish rules.",
            schema(json!({}), &[]),
        ),
        tool(
            "set_polish_app",
            "Set App Polish Preset",
            "Set polish preset for bundle id.",
            schema(
                json!({"bundle_id": {"type": "string"}, "preset": {"type": "string"}}),
                &["bundle_id", "preset"],
            ),
        ),
        tool(
            "create_automation_job",
            "Create Automation Job",
            "Create and run a meeting email automation job (recap or weekly digest).",
            schema(
                json!({
                    "trigger": {"type": "string", "enum": ["analysis_ready", "weekly_digest"]},
                    "meeting_id": {"type": "string"},
                    "force": {"type": "boolean"},
                }),
                &["trigger"],
            ),
        ),
        tool(
            "list_automation_jobs",
            "List Automation Jobs",
            "List recent meeting email automation job runs.",
            schema(json!({"limit": limit()}), &[]),
        ),
    ]
}

/// The tools a daemon with `capabilities` exposes: every tool, minus the
/// meeting tools when `formats.meeting_export` is an empty list. No
/// capabilities (the daemon did not answer) lists every tool.
pub fn available(capabilities: Option<&Value>) -> Vec<Tool> {
    let hide_meetings = capabilities
        .and_then(|c| c["formats"]["meeting_export"].as_array())
        .is_some_and(Vec::is_empty);
    catalog()
        .into_iter()
        .filter(|t| !(hide_meetings && MEETING_TOOLS.contains(&t.name)))
        .collect()
}

/// The names of `tools`, in order.
pub fn names(tools: &[Tool]) -> Vec<&'static str> {
    tools.iter().map(|t| t.name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nineteen_tools_minus_the_meeting_tools_when_no_meeting_export_exists() {
        let all = catalog();
        assert_eq!(all.len(), 19);
        let unique: std::collections::BTreeSet<&str> = all.iter().map(|t| t.name).collect();
        assert_eq!(unique.len(), 19);
        assert_eq!(available(None).len(), 19);
        let with = json!({"formats": {"meeting_export": ["txt"]}});
        assert_eq!(available(Some(&with)).len(), 19);
        let without = json!({"formats": {"meeting_export": []}});
        let shown = available(Some(&without));
        assert_eq!(shown.len(), 15);
        assert!(shown.iter().all(|t| !MEETING_TOOLS.contains(&t.name)));
    }

    #[test]
    fn every_schema_is_a_closed_object_and_required_names_a_property() {
        for t in catalog() {
            let s = &t.input_schema;
            assert_eq!(s["type"], "object", "{}", t.name);
            assert_eq!(s["additionalProperties"], false, "{}", t.name);
            let props = s["properties"].as_object().unwrap();
            for r in s["required"].as_array().into_iter().flatten() {
                assert!(props.contains_key(r.as_str().unwrap()), "{}", t.name);
            }
            let def = t.definition();
            assert_eq!(def["name"], t.name);
            assert_eq!(def["inputSchema"], *s);
        }
    }
}
