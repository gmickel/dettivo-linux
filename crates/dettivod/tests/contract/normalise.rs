//! The tolerance the contract replay applies to a live answer before it
//! is held against its fixture: the keys a server generates on the spot,
//! and the method-specific shapes whose content describes the machine or
//! the seeded store rather than the contract. Nothing is copied from the
//! fixture into the answer; what varies is asserted, then levelled on
//! both sides.

use serde_json::{Value, json};

/// Values a server generates on the spot (fixtures/README.md).
const TOLERANT: &[&str] = &[
    "job_id",
    "transfer_id",
    "subscription_id",
    "expires_at",
    "build",
    "app_version",
    "uptime_seconds",
    "path",
    "latency_ms",
];

/// Drops tolerant keys anywhere in the tree.
fn strip(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|k, _| !TOLERANT.contains(&k.as_str()));
            for v in map.values_mut() {
                strip(v);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(strip),
        _ => {}
    }
}

/// Method-specific tolerance on top of the key list: the platform block
/// and `config.path` describe the machine, `audio.devices` its devices,
/// and the history results its content; the shapes are asserted and the
/// content gives way to the fixture's.
pub fn normalise(method: &str, mut value: Value, fixture: &Value) -> Value {
    if method == "system.capabilities" {
        let platform = value["result"]["platform"].take();
        assert_eq!(platform["os"], "linux", "platform block: {platform}");
        assert_eq!(platform["transport"], "unix_socket");
        // The hotkey backend describes the desktop (a portal or none).
        let hotkeys = value["result"]["hotkeys"].take();
        assert!(hotkeys["backend"].is_string(), "hotkeys block: {hotkeys}");
        assert!(hotkeys["available"].is_array(), "hotkeys block: {hotkeys}");
    }
    if method == "hotkeys.status" {
        // Availability and its reasons describe the machine; the keys and
        // types are what the fixture pins.
        if let Some(result) = value["result"].as_object_mut() {
            for key in ["backend", "requested"] {
                assert!(result[key].is_string(), "{key}: {}", result[key]);
                result.insert(key.into(), Value::String("x".into()));
            }
            for key in ["portal", "evdev"] {
                assert!(
                    result[key]["available"].is_boolean(),
                    "{key}: {}",
                    result[key]
                );
                result.insert(key.into(), json!({"available": false, "reason": "x"}));
            }
            assert!(result["bound"].is_array());
            result.insert("bound".into(), json!([]));
            result.insert("error".into(), Value::Null);
        }
    }
    if method == "audio.devices" {
        if let Some(result) = value["result"].as_object_mut() {
            assert!(
                result["pipewire"].is_boolean(),
                "pipewire: {}",
                result["pipewire"]
            );
            assert!(
                result["default_source"].is_null() || result["default_source"].is_string(),
                "default_source: {}",
                result["default_source"]
            );
            assert!(
                result["devices"].is_array(),
                "devices: {}",
                result["devices"]
            );
            result.insert("devices".into(), Value::Array(Vec::new()));
            result.insert("default_source".into(), Value::Null);
            result.insert("default_sink".into(), Value::Null);
            result.insert("pipewire".into(), Value::Bool(true));
        }
    }
    if method == "config.path" {
        if let Some(result) = value["result"].as_object_mut() {
            for v in result.values_mut() {
                *v = Value::Null;
            }
        }
    }
    if method == "transcripts.list" {
        // The seed holds twelve items and the contract sample is the newest
        // one, so the first row must equal the fixture's; the rest are rows.
        if let Some(items) = value["result"]["items"].as_array_mut() {
            assert!(!items.is_empty(), "no items listed");
            for row in items.iter() {
                assert!(
                    row["ref"]["id"].is_string() && row["status"].is_string(),
                    "{row}"
                );
            }
            items.truncate(1);
        }
    }
    if method == "transcripts.search" || method == "meetings.search" {
        // The seeded store answers the fixture's query: the rows deserialise
        // as the protocol type with no score (semantic search is off,
        // FR-Y4), the fixture's row is among them, and only the extra
        // seeded hits are set aside; nothing is copied from the fixture
        // into the answer, so an empty result fails here.
        let result = value["result"].clone();
        if method == "transcripts.search" {
            let typed: dettivo_proto::methods::transcripts::SearchResult =
                serde_json::from_value(result.clone())
                    .unwrap_or_else(|e| panic!("{method} result: {e}"));
            assert!(typed.items.iter().all(|i| i.score.is_none()), "{result}");
        } else {
            let typed: dettivo_proto::methods::meetings::SearchResult =
                serde_json::from_value(result.clone())
                    .unwrap_or_else(|e| panic!("{method} result: {e}"));
            assert!(typed.items.iter().all(|i| i.score.is_none()), "{result}");
        }
        let expected = fixture["result"]["items"].as_array().unwrap();
        let same = |i: &Value, row: &Value| {
            i["ref"] == row["ref"] && i["matched_field"] == row["matched_field"]
        };
        if let Some(items) = value["result"]["items"].as_array_mut() {
            assert!(
                !items.is_empty(),
                "{method}: the seeded store answered nothing"
            );
            for row in expected {
                assert!(
                    items.iter().any(|i| same(i, row)),
                    "{method}: the fixture's row {} is not among the seeded hits: {items:?}",
                    row["ref"]
                );
            }
            items.retain(|i| expected.iter().any(|row| same(i, row)));
            // The snippet's window around the hit is the store's choice;
            // the fixture's text has to be inside it, then the field is
            // levelled on both sides rather than copied across.
            for i in items.iter_mut() {
                let row = expected.iter().find(|row| same(i, row)).unwrap();
                let core = row["snippet"].as_str().unwrap().trim_matches('…');
                let got = i["snippet"].as_str().unwrap_or_default().to_string();
                assert!(
                    got.contains(core),
                    "{method}: snippet {got:?} does not carry the fixture's {core:?}"
                );
                i["snippet"] = Value::Null;
            }
        }
    }
    if method == "transcripts.stats" {
        if let Some(result) = value["result"].as_object_mut() {
            assert!(result["item_count"].as_u64().is_some());
            assert!(result["size_bytes"].as_u64().is_some_and(|n| n > 0));
            assert_eq!(result["schema_version"], 8);
            assert_eq!(result["last_migration"], "0008-analysis-queued");
            for key in ["item_count", "size_bytes", "audio_items"] {
                result.insert(key.into(), fixture["result"][key].clone());
            }
        }
    }
    strip(&mut value);
    value
}

#[test]
#[should_panic(expected = "the seeded store answered nothing")]
fn an_empty_search_result_never_passes_as_the_fixture() {
    let fixture = json!({"result": {"items": [
        {"matched_field": "transcript", "ref": {"id": "a", "kind": "meeting"}, "score": null, "snippet": "…the API…"}
    ]}});
    normalise(
        "transcripts.search",
        json!({"jsonrpc": "2.0", "id": "1", "result": {"items": []}}),
        &fixture,
    );
}

#[test]
#[should_panic(expected = "transcripts.search result")]
fn a_malformed_search_row_never_passes_as_the_fixture() {
    let fixture = json!({"result": {"items": [
        {"matched_field": "transcript", "ref": {"id": "a", "kind": "meeting"}, "score": null, "snippet": "…the API…"}
    ]}});
    normalise(
        "transcripts.search",
        json!({"jsonrpc": "2.0", "id": "1", "result": {"items": [
            {"matched_field": "transcript", "ref": {"id": "a", "kind": "meeting"}, "score": "high", "snippet": 7}
        ]}}),
        &fixture,
    );
}
