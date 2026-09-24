//! The harness steps, in order: `initialize`, `ping`, the lists, the
//! read tools, the meeting tools over the seeded meeting, an insertion
//! through the mock backend, an export, the unknown tool, the guidance
//! answers, every resource URI, and last a
//! live `import_audio` of the fixture clip through the real engine
//! (skipped with the reason when the test model is absent); the item it
//! creates is removed after the run so the seed stays the newest row.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::harness::{MEETING_ID, PROTOCOL_VERSION, SAMPLE_ID, Session};
use crate::{SERVER_NAME, SERVER_VERSION, resources, tools};

fn ok(cond: bool, why: impl FnOnce() -> String) -> Result<(), String> {
    if cond { Ok(()) } else { Err(why()) }
}

fn structured(result: &Value) -> Result<&Value, String> {
    if result["isError"] == true {
        return Err(format!("tool error: {}", Session::text_of(result)));
    }
    Ok(&result["structuredContent"])
}

/// A step's error starting with this is a skip, not a failure.
pub const SKIP: &str = "skip: ";

type Step = (&'static str, fn(&mut Session) -> Result<(), String>);

/// Polls `list_transcripts` until `id` leaves `transcribing`; the row.
fn settled(s: &mut Session, id: &str) -> Result<Value, String> {
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        let v = structured(&s.call_tool("list_transcripts", json!({"limit": 50}))?)?.clone();
        let row = v["items"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| r["ref"]["id"] == id)
            .cloned()
            .ok_or_else(|| format!("{id} not listed"))?;
        if row["status"] != "transcribing" {
            return Ok(row);
        }
        if Instant::now() > deadline {
            return Err(format!("{id} still transcribing"));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// The representative steps, in order. `initialize` runs first so the
/// later ones speak to an initialised server.
pub fn steps() -> Vec<Step> {
    vec![
        ("initialize", |s| {
            let r = s.result("initialize", json!({"protocolVersion": PROTOCOL_VERSION, "capabilities": {}, "clientInfo": {"name": "harness", "version": "0"}}))?;
            ok(r["protocolVersion"] == PROTOCOL_VERSION, || {
                format!("protocolVersion {}", r["protocolVersion"])
            })?;
            ok(
                r["serverInfo"]["name"] == SERVER_NAME
                    && r["serverInfo"]["version"] == SERVER_VERSION,
                || format!("serverInfo {}", r["serverInfo"]),
            )?;
            ok(
                r["capabilities"]["tools"].is_object()
                    && r["capabilities"]["resources"].is_object(),
                || format!("capabilities {}", r["capabilities"]),
            )?;
            s.notify("notifications/initialized", json!({}))
        }),
        ("ping", |s| {
            let r = s.result("ping", json!({}))?;
            ok(r == json!({}), || format!("ping {r}"))
        }),
        ("get_status", |s| {
            let r = s.call_tool("get_status", json!({}))?;
            let v = structured(&r)?;
            ok(v["ok"] == true && v["recording_state"].is_string(), || {
                format!("health {v}")
            })?;
            ok(
                v["capabilities"]["formats"]["meeting_export"].is_array(),
                || format!("capabilities {}", v["capabilities"]),
            )?;
            ok(Session::text_of(&r).contains("\"recording_state\""), || {
                "text block".into()
            })
        }),
        ("tools/list", |s| {
            let caps = structured(&s.call_tool("get_status", json!({}))?)?["capabilities"].clone();
            let expected: Vec<Value> = tools::available(Some(&caps))
                .iter()
                .map(tools::Tool::definition)
                .collect();
            let r = s.result("tools/list", json!({}))?;
            let got = r["tools"].as_array().cloned().unwrap_or_default();
            ok(got.len() == expected.len(), || {
                format!("{} tools, expected {}", got.len(), expected.len())
            })?;
            for (g, e) in got.iter().zip(&expected) {
                ok(g == e, || format!("tool {} differs: {g}", e["name"]))?;
            }
            Ok(())
        }),
        ("resources/templates/list", |s| {
            let r = s.result("resources/templates/list", json!({}))?;
            let got = r["resourceTemplates"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            ok(got == resources::templates(), || {
                format!("templates {}", r["resourceTemplates"])
            })
        }),
        ("resources/list", |s| {
            let r = s.result("resources/list", json!({}))?;
            let uris: Vec<&str> = r["resources"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|x| x["uri"].as_str())
                .collect();
            ok(
                uris.first() == Some(&"status://current")
                    && uris.get(1) == Some(&resources::SPEECH_PROVIDERS),
                || format!("head {uris:?}"),
            )?;
            ok(
                uris.contains(&format!("transcript://{SAMPLE_ID}").as_str()),
                || format!("sample missing in {uris:?}"),
            )
        }),
        ("list_transcripts", |s| {
            let v = structured(&s.call_tool("list_transcripts", json!({"limit": 5}))?)?.clone();
            let items = v["items"].as_array().cloned().unwrap_or_default();
            ok(items.len() == 5, || format!("{} items", items.len()))?;
            ok(items[0]["ref"]["id"] == SAMPLE_ID, || {
                format!("first {}", items[0]["ref"])
            })
        }),
        ("search_transcripts", |s| {
            let v = structured(&s.call_tool("search_transcripts", json!({"query": "uploader"}))?)?
                .clone();
            let items = v["items"].as_array().cloned().unwrap_or_default();
            ok(!items.is_empty() && items[0]["snippet"].is_string(), || {
                format!("hits {v}")
            })
        }),
        ("get_latest_transcript", |s| {
            let v =
                structured(&s.call_tool("get_latest_transcript", json!({"kind": "dictation"}))?)?
                    .clone();
            ok(v["ref"]["id"] == SAMPLE_ID, || format!("latest {v}"))
        }),
        ("get_transcript", |s| {
            let v = structured(&s.call_tool("get_transcript", json!({"id": SAMPLE_ID}))?)?.clone();
            ok(
                v["ref"]["id"] == SAMPLE_ID
                    && v["text_polish"].as_str().is_some_and(|t| !t.is_empty()),
                || format!("item {v}"),
            )
        }),
        ("list_meetings", |s| {
            let v = structured(&s.call_tool("list_meetings", json!({"limit": 5}))?)?.clone();
            let items = v["items"].as_array().cloned().unwrap_or_default();
            ok(
                items
                    .iter()
                    .any(|m| m["ref"]["id"] == MEETING_ID && m["title"] == "Weekly sync"),
                || format!("meetings {v}"),
            )
        }),
        ("get_meeting", |s| {
            let v = structured(&s.call_tool("get_meeting", json!({"meeting_id": MEETING_ID}))?)?
                .clone();
            ok(
                v["transcript"] == "Hello."
                    && v["segments"].as_array().is_some_and(|x| x.len() == 1),
                || format!("meeting {v}"),
            )
        }),
        ("meeting://{id}", |s| {
            let r = s.result(
                "resources/read",
                json!({"uri": format!("meeting://{MEETING_ID}")}),
            )?;
            let v: Value = serde_json::from_str(r["contents"][0]["text"].as_str().unwrap_or(""))
                .map_err(|e| e.to_string())?;
            ok(v["ref"]["id"] == MEETING_ID, || format!("meeting {v}"))
        }),
        ("insert_transcript", |s| {
            let v = structured(&s.call_tool(
                "insert_transcript",
                json!({"mode": "raw", "source_ref": {"kind": "dictation", "id": SAMPLE_ID}}),
            )?)?
            .clone();
            ok(
                v["outcome"] == "inserted" && v["backend"]["name"] == "mock",
                || format!("insertion {v}"),
            )
        }),
        ("export_transcript", |s| {
            let v = structured(&s.call_tool(
                "export_transcript",
                json!({"ref": {"kind": "dictation", "id": SAMPLE_ID}, "format": "json"}),
            )?)?
            .clone();
            ok(
                v["transfer_id"].is_string()
                    && v["preview"].as_str().is_some_and(|p| p.contains(SAMPLE_ID)),
                || format!("export {v}"),
            )
        }),
        ("unknown_tool", |s| {
            let r = s.call_tool("frobnicate", json!({}))?;
            let text = Session::text_of(&r);
            ok(
                r["isError"] == true
                    && text.starts_with("Unknown tool: frobnicate. Available tools: get_status, ")
                    && text.contains("list_automation_jobs"),
                || format!("unknown {r}"),
            )
        }),
        ("create_automation_job", |s| {
            let r = s.call_tool("create_automation_job", json!({"trigger": "weekly_digest"}))?;
            let text = Session::text_of(&r);
            ok(
                r["isError"] == true
                    && text.contains("NOT_IMPLEMENTED")
                    && text.ends_with(crate::messages::ACTION_AUTOMATION_OFF),
                || format!("guidance {r}"),
            )
        }),
        // The polish layers landed (ADR 0023): the tool answers with the
        // rules the daemon holds, the QA seed's own among them.
        ("list_polish_rules", |s| {
            let r = s.call_tool("list_polish_rules", json!({}))?;
            let text = Session::text_of(&r);
            ok(
                r["isError"] != true && text.contains("Professional Tone"),
                || format!("rules {r}"),
            )
        }),
        ("status://current", |s| {
            let r = s.result("resources/read", json!({"uri": "status://current"}))?;
            let text = r["contents"][0]["text"].as_str().unwrap_or("");
            let v: Value =
                serde_json::from_str(text).map_err(|e| format!("text is not JSON: {e}"))?;
            ok(
                r["contents"][0]["uri"] == "status://current" && v["ok"] == true,
                || format!("status {r}"),
            )
        }),
        ("transcript://{id}", |s| {
            let r = s.result(
                "resources/read",
                json!({"uri": format!("transcript://{SAMPLE_ID}")}),
            )?;
            let v: Value = serde_json::from_str(r["contents"][0]["text"].as_str().unwrap_or(""))
                .map_err(|e| e.to_string())?;
            ok(v["ref"]["id"] == SAMPLE_ID, || format!("transcript {v}"))
        }),
        ("transcripts://search/{query}", |s| {
            let r = s.result(
                "resources/read",
                json!({"uri": "transcripts://search/uploader?kinds=dictation&limit=3"}),
            )?;
            let v: Value = serde_json::from_str(r["contents"][0]["text"].as_str().unwrap_or(""))
                .map_err(|e| e.to_string())?;
            ok(v["items"].as_array().is_some_and(|i| !i.is_empty()), || {
                format!("search {v}")
            })
        }),
        ("transcripts://latest/{kind}", |s| {
            let r = s.result(
                "resources/read",
                json!({"uri": "transcripts://latest/dictation"}),
            )?;
            let v: Value = serde_json::from_str(r["contents"][0]["text"].as_str().unwrap_or(""))
                .map_err(|e| e.to_string())?;
            ok(v["ref"]["id"] == SAMPLE_ID, || format!("latest {v}"))
        }),
        ("speech://providers", |s| {
            let r = s.result(
                "resources/read",
                json!({"uri": resources::SPEECH_PROVIDERS}),
            )?;
            let v: Value = serde_json::from_str(r["contents"][0]["text"].as_str().unwrap_or(""))
                .map_err(|e| e.to_string())?;
            ok(v["providers"].is_array(), || format!("providers {v}"))
        }),
        ("import_audio", |s| {
            let Some(fixture) = s.fixture.clone() else {
                return Err(format!(
                    "{SKIP}no test model or fixture (scripts/models/fetch-test-model.sh)"
                ));
            };
            let v = structured(&s.call_tool(
                "import_audio",
                json!({"file_path": fixture.to_string_lossy(), "language": "en"}),
            )?)?
            .clone();
            ok(
                v["is_partial"] == true && v["job"]["state"] == "running",
                || format!("import {v}"),
            )?;
            let id = v["ref"]["id"].as_str().unwrap_or("").to_string();
            s.created.push(id.clone());
            let row = settled(s, &id)?;
            ok(row["status"] == "completed", || format!("row {row}"))?;
            let got = structured(&s.call_tool("get_transcript", json!({"id": id}))?)?.clone();
            let text = got["text_polish"].as_str().unwrap_or("").to_lowercase();
            ok(
                text.contains("americans")
                    && got["segments"].as_array().is_some_and(|x| !x.is_empty()),
                || format!("transcript {got}"),
            )
        }),
    ]
}
