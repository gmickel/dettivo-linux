//! The macOS bounds every tool result and resource body pass through
//! before they reach an agent: 50 items per list, 100 keys per object,
//! 20 000 characters per text, depth 8, and 2 000 000 bytes per transport
//! message. What is cut is marked so an agent knows the shape is partial:
//! a text ends in `[truncated N chars]`, an object carries
//! `_truncated_keys`, a value past the depth becomes the depth marker,
//! and the whole payload carries `_truncated: true` (a cut list, which
//! has no room for a marker of its own, sets it too).

use serde_json::{Map, Value};

/// Items kept per list.
pub const MAX_ITEMS: usize = 50;
/// Keys kept per object.
pub const MAX_OBJECT_KEYS: usize = 100;
/// Characters kept per text.
pub const MAX_TEXT_CHARS: usize = 20_000;
/// Nesting depth kept; a value at this depth becomes [`DEPTH_MARKER`].
pub const MAX_DEPTH: usize = 8;
/// Bytes accepted per transport message in either direction.
pub const MAX_MESSAGE_BYTES: u64 = 2_000_000;

/// The value a container past the depth limit is replaced with.
pub const DEPTH_MARKER: &str = "[truncated: max depth reached]";
/// The key an object over the key limit gains, holding the dropped count.
pub const TRUNCATED_KEYS: &str = "_truncated_keys";
/// The top-level marker on a bounded payload.
pub const TRUNCATED: &str = "_truncated";

/// A bounded payload and whether anything was cut.
#[derive(Debug, Clone, PartialEq)]
pub struct Bounded {
    /// The payload with the bounds applied.
    pub value: Value,
    /// True when a list, an object, a text or the depth was cut.
    pub truncated: bool,
}

/// Applies every bound to `value`.
pub fn bounded(value: &Value) -> Bounded {
    let mut truncated = false;
    let value = bound(value, 0, &mut truncated);
    Bounded { value, truncated }
}

fn bound(value: &Value, depth: usize, truncated: &mut bool) -> Value {
    if depth >= MAX_DEPTH {
        *truncated = true;
        return Value::String(DEPTH_MARKER.into());
    }
    match value {
        Value::String(text) => {
            let cut = bounded_text(text);
            if cut.len() != text.len() {
                *truncated = true;
            }
            Value::String(cut)
        }
        Value::Array(items) => {
            if items.len() > MAX_ITEMS {
                *truncated = true;
            }
            Value::Array(
                items
                    .iter()
                    .take(MAX_ITEMS)
                    .map(|v| bound(v, depth + 1, truncated))
                    .collect(),
            )
        }
        Value::Object(fields) => {
            let mut out = Map::new();
            for (key, v) in fields.iter().take(MAX_OBJECT_KEYS) {
                out.insert(key.clone(), bound(v, depth + 1, truncated));
            }
            if fields.len() > MAX_OBJECT_KEYS {
                *truncated = true;
                out.insert(
                    TRUNCATED_KEYS.into(),
                    Value::from(fields.len() - MAX_OBJECT_KEYS),
                );
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// The first [`MAX_TEXT_CHARS`] characters of a text, ending in the
/// omitted count when it was longer.
pub fn bounded_text(text: &str) -> String {
    let count = text.chars().count();
    if count <= MAX_TEXT_CHARS {
        return text.to_string();
    }
    let prefix: String = text.chars().take(MAX_TEXT_CHARS).collect();
    format!("{prefix}\n\n[truncated {} chars]", count - MAX_TEXT_CHARS)
}

/// The macOS scan: true when a text carries a truncation marker or an
/// object carries `_truncated_keys`. A cut list leaves no trace here,
/// which is why [`Bounded::truncated`] exists.
pub fn contains_truncation(value: &Value) -> bool {
    match value {
        Value::String(s) => s.contains("[truncated"),
        Value::Array(items) => items.iter().any(contains_truncation),
        Value::Object(fields) => {
            fields.contains_key(TRUNCATED_KEYS) || fields.values().any(contains_truncation)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_short_payload_passes_untouched() {
        let v = json!({"ok": true, "items": [1, 2, 3], "text": "hi"});
        let b = bounded(&v);
        assert_eq!(b.value, v);
        assert!(!b.truncated);
        assert!(!contains_truncation(&b.value));
    }

    #[test]
    fn long_text_keeps_the_prefix_and_names_the_omitted_count() {
        let text: String = "é".repeat(MAX_TEXT_CHARS + 7);
        let b = bounded(&json!({"text_polish": text}));
        let cut = b.value["text_polish"].as_str().unwrap();
        assert!(cut.ends_with("\n\n[truncated 7 chars]"), "{cut}");
        assert_eq!(
            cut.chars().count(),
            MAX_TEXT_CHARS + "\n\n[truncated 7 chars]".len()
        );
        assert!(b.truncated);
        assert!(contains_truncation(&b.value));
    }

    #[test]
    fn long_lists_are_cut_to_fifty_and_flagged() {
        let items: Vec<Value> = (0..60).map(Value::from).collect();
        let b = bounded(&json!({"items": items}));
        assert_eq!(b.value["items"].as_array().unwrap().len(), MAX_ITEMS);
        assert!(b.truncated);
        assert!(!contains_truncation(&b.value));
    }

    #[test]
    fn wide_objects_keep_a_hundred_keys_and_count_the_rest() {
        let mut fields = Map::new();
        for i in 0..120 {
            fields.insert(format!("k{i:03}"), Value::from(i));
        }
        let b = bounded(&Value::Object(fields));
        let obj = b.value.as_object().unwrap();
        assert_eq!(obj.len(), MAX_OBJECT_KEYS + 1);
        assert_eq!(obj[TRUNCATED_KEYS], json!(20));
        assert!(b.truncated);
        assert!(contains_truncation(&b.value));
    }

    #[test]
    fn nesting_past_depth_eight_becomes_the_marker() {
        let mut v = json!("leaf");
        for _ in 0..10 {
            v = json!({"child": v});
        }
        let b = bounded(&v);
        let mut cursor = &b.value;
        for _ in 0..MAX_DEPTH {
            cursor = &cursor["child"];
        }
        assert_eq!(cursor, &json!(DEPTH_MARKER));
        assert!(b.truncated);
        let mut shallow = json!("leaf");
        for _ in 0..MAX_DEPTH - 1 {
            shallow = json!({"child": shallow});
        }
        assert!(!bounded(&shallow).truncated);
    }
}
