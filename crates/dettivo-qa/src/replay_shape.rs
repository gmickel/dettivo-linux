//! The shape of a contract result for the replays that compare by shape
//! (the machine-specific and store-specific methods): objects keep their
//! keys, a list keeps the structure of its rows, every scalar keeps its
//! type, so a value that varies by machine still has to keep its type
//! and a row its fields. Only what varies is tolerated: an empty list on
//! either side (no device on a runner), and a `null` where the other
//! machine set an optional field.

use serde_json::Value;

/// The shape of a value: objects keep their keys, a list becomes the
/// shape of its elements merged (one element, its keys the union), and
/// every scalar becomes its type (`string`, `number`, `bool`, `null`).
/// `shapes_agree` compares two shapes.
pub fn shape_of(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            Value::Object(map.iter().map(|(k, v)| (k.clone(), shape_of(v))).collect())
        }
        Value::Array(items) => {
            let merged = items
                .iter()
                .map(shape_of)
                .reduce(|a, b| merge_shapes(&a, &b));
            Value::Array(merged.into_iter().collect())
        }
        Value::String(_) => Value::String("string".into()),
        Value::Number(_) => Value::String("number".into()),
        Value::Bool(_) => Value::String("bool".into()),
        Value::Null => Value::String("null".into()),
    }
}

/// Two element shapes as one: objects take the union of their keys, a
/// null gives way to a type, and two types that differ are listed.
fn merge_shapes(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let mut out = x.clone();
            for (k, v) in y {
                let merged = match out.get(k) {
                    Some(have) => merge_shapes(have, v),
                    None => v.clone(),
                };
                out.insert(k.clone(), merged);
            }
            Value::Object(out)
        }
        (Value::Array(x), Value::Array(y)) => match (x.first(), y.first()) {
            (Some(p), Some(q)) => Value::Array(vec![merge_shapes(p, q)]),
            (Some(p), None) | (None, Some(p)) => Value::Array(vec![p.clone()]),
            (None, None) => Value::Array(Vec::new()),
        },
        (Value::String(x), Value::String(y)) if x == y || y == "null" => a.clone(),
        (Value::String(x), Value::String(_)) if x == "null" => b.clone(),
        (Value::String(x), Value::String(y)) => Value::String(format!("{x}|{y}")),
        _ => b.clone(),
    }
}

/// Whether a shape this machine produced fits the fixture's: keys and
/// nesting agree, a leaf keeps its type, a list may be empty on either
/// side, and a `null` on either side stands for an optional field that
/// the other machine set.
pub fn shapes_agree(want: &Value, got: &Value) -> bool {
    match (want, got) {
        (Value::Object(w), Value::Object(g)) => {
            w.len() == g.len()
                && w.iter()
                    .all(|(k, v)| g.get(k).is_some_and(|x| shapes_agree(v, x)))
        }
        (Value::Array(w), Value::Array(g)) => match (w.first(), g.first()) {
            (Some(p), Some(q)) => shapes_agree(p, q),
            _ => true,
        },
        (Value::String(w), Value::String(g)) => {
            w == g || w == "null" || g == "null" || w.split('|').any(|t| t == g)
        }
        (Value::String(w), _) | (_, Value::String(w)) => w == "null",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::normalise;
    use serde_json::json;

    fn fixture(name: &str) -> Value {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../dettivo-proto/fixtures");
        serde_json::from_str(&std::fs::read_to_string(root.join(name)).unwrap()).unwrap()
    }

    #[test]
    fn normalized_replies_keep_errors_and_capability_constants_exact() {
        for name in [
            "speech/models.delete.error-conflict-selected.json",
            "speech/models.download.error-invalid-params-unknown.json",
            "speech/selection.set.error-invalid-params-empty.json",
            "system/capabilities.json",
        ] {
            let doc = fixture(name);
            let method = doc["method"].as_str().unwrap();
            let response = doc.get("response").or(doc.get("error")).unwrap();
            let want = normalise(method, response.clone());
            assert!(
                crate::replay::replies_agree(method, &want, &want),
                "{name}: identical reply"
            );
            if method == "system.capabilities" {
                let mut machine = response.clone();
                machine["result"]["hotkeys"]["backend"] = json!("hyprland");
                machine["result"]["rest"]["port"] = json!(49152);
                assert!(crate::replay::replies_agree(
                    method,
                    &want,
                    &normalise(method, machine.clone())
                ));
                machine["result"]["rest"]["port"] = json!("49152");
                assert!(!crate::replay::replies_agree(
                    method,
                    &want,
                    &normalise(method, machine)
                ));
            }
            let mutations = if method == "system.capabilities" {
                vec![
                    ("/result/auth/rest_token_required", json!(false)),
                    ("/result/transfers/max_inflight", json!(999)),
                    ("/result/config/methods/1", json!("wrong.method")),
                    ("/result/config/methods", json!([])),
                ]
            } else {
                vec![
                    ("/error/code", json!(-1)),
                    ("/error/message", json!("wrong error")),
                    ("/error/data/retryable", json!(true)),
                ]
            };
            for (path, value) in mutations {
                let mut got = want.clone();
                *got.pointer_mut(path).unwrap() = value;
                assert!(
                    !crate::replay::replies_agree(method, &want, &got),
                    "{name}: changed {path}"
                );
            }
        }
    }

    #[test]
    fn transcript_fixtures_cover_registered_meeting_and_search_rows() {
        let listing = fixture("transcripts/list.json");
        let rows = listing["response"]["result"]["items"].as_array().unwrap();
        let meeting = rows
            .iter()
            .find(|r| r["ref"]["kind"] == "meeting")
            .expect("ADR 0038 meeting row is represented");
        assert!(meeting["speaker_count"].is_number());
        assert!(meeting["analysis_status"].is_string());
        assert!(meeting["is_partial"].is_boolean());
        let search = fixture("transcripts/search.json");
        let hits = search["response"]["result"]["items"].as_array().unwrap();
        let dictation = hits
            .iter()
            .find(|r| r["ref"]["kind"] == "dictation")
            .expect("ADR 0025 dictation search row is represented");
        assert_eq!(dictation["ref"], dictation["item"]["ref"]);
        for key in ["title", "started_at", "status", "app_id", "mode", "source"] {
            assert!(dictation["item"][key].is_string(), "search item {key}");
        }
        assert!(dictation["item"]["duration_seconds"].is_number());
        let ranges = dictation["matches"].as_array().unwrap();
        assert!(!ranges.is_empty());
        let text: Vec<_> = dictation["snippet"].as_str().unwrap().chars().collect();
        for range in ranges {
            let start = range["start"].as_u64().unwrap() as usize;
            let end = range["end"].as_u64().unwrap() as usize;
            assert_eq!(
                text[start..end].iter().collect::<String>().to_lowercase(),
                "api"
            );
        }
        assert!(
            hits.iter()
                .any(|r| r["ref"]["kind"] == "meeting" && r["matched_field"].is_string())
        );
    }

    /// qa-packs/F9 (fn-43): the shape keeps every leaf's type and every
    /// array's element structure, so a wrong scalar type or a malformed
    /// element fails the replay while an empty list and a null where the
    /// fixture has a value (or a value where it has null) still pass.
    #[test]
    fn typed_shapes_catch_a_wrong_type_and_a_malformed_element() {
        let want = json!({"result": {
            "default_source": "alsa_input", "pipewire": true, "pinned": "",
            "devices": [{"name": "a", "kind": "source", "is_default": true}]
        }});
        let agree = |got: Value| {
            shapes_agree(
                &normalise("audio.devices", want.clone()),
                &normalise("audio.devices", got),
            )
        };
        assert!(agree(json!({"result": {
            "default_source": "other", "pipewire": false, "pinned": "x",
            "devices": [{"name": "b", "kind": "sink_monitor", "is_default": false},
                        {"name": "c", "kind": "source", "is_default": true}]
        }})));
        assert!(
            agree(json!({"result": {
                "default_source": null, "pipewire": true, "pinned": "", "devices": []
            }})),
            "an empty list and a null optional field are this machine's"
        );
        assert!(
            !agree(json!({"result": {
                "default_source": "x", "pipewire": "yes", "pinned": "", "devices": []
            }})),
            "a string where the fixture has a bool"
        );
        assert!(
            !agree(json!({"result": {
                "default_source": "x", "pipewire": true, "pinned": "",
                "devices": [{"name": "b", "kind": "source"}]
            }})),
            "an element missing a key"
        );
        assert!(
            !agree(json!({"result": {
                "default_source": "x", "pipewire": true, "pinned": "",
                "devices": [{"name": "b", "kind": 3, "is_default": true}]
            }})),
            "an element with a wrong type"
        );
        assert!(
            !agree(json!({"result": {
                "default_source": "x", "pipewire": true, "devices": []
            }})),
            "a missing field"
        );
        // Rows that differ in an optional field merge into one row shape.
        let rows = shape_of(&json!([{"a": 1, "e": null}, {"a": 2, "e": "x"}]));
        assert_eq!(rows, json!([{"a": "number", "e": "string"}]));
    }
}
