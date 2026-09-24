//! Comment-preserving edits: `set` and `unset` rewrite one key in the
//! file text through `toml_edit`, coerce a typed-as-string value to the
//! key's declared type, and refuse any result the schema would not parse,
//! so the daemon never writes a file it could not read back.

use dettivo_proto::methods::config::ValidationError;
use serde_json::Value;
use toml_edit::{DocumentMut, Item};

use super::schema::{Config, DEFAULT_TOML};
use super::validate::validate;

/// The declared type of a key, taken from the default configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A TOML string.
    Text,
    /// A TOML integer.
    Integer,
    /// A TOML float.
    Float,
    /// A TOML boolean.
    Boolean,
    /// A TOML array of strings.
    List,
    /// A TOML inline table of strings.
    Table,
}

/// Every leaf key of the schema in file order, with its declared type.
/// A table the default file writes as its own section (`[engines.whisper]`)
/// is walked into, so its keys read `engines.whisper.backend`; an inline
/// table (`replacements = {}`) is one key of kind `Table`.
pub fn keys() -> Vec<(String, Kind)> {
    let defaults = serde_json::to_value(Config::default()).unwrap_or(Value::Null);
    let mut out = Vec::new();
    if let Some(sections) = defaults.as_object() {
        for (section, table) in sections {
            collect_keys(section, table, &mut out);
        }
    }
    out
}

fn collect_keys(prefix: &str, table: &Value, out: &mut Vec<(String, Kind)>) {
    let Some(entries) = table.as_object() else {
        return;
    };
    for (name, value) in entries {
        let key = format!("{prefix}.{name}");
        if value.is_object() && is_section(&key) {
            collect_keys(&key, value, out);
            continue;
        }
        let kind = match value {
            Value::Bool(_) => Kind::Boolean,
            Value::Number(n) if n.is_i64() || n.is_u64() => Kind::Integer,
            Value::Number(_) => Kind::Float,
            Value::Array(_) => Kind::List,
            Value::Object(_) => Kind::Table,
            _ => Kind::Text,
        };
        out.push((key, kind));
    }
}

/// True when the default file writes `path` as a section header of its
/// own (`[engines.whisper]`) rather than as an inline table.
pub fn is_section(path: &str) -> bool {
    DEFAULT_TOML.contains(&format!("\n[{path}]\n"))
}

/// The declared type of `key`, or `None` when the schema has no such key.
pub fn kind_of(key: &str) -> Option<Kind> {
    keys()
        .into_iter()
        .find(|(k, _)| k == key)
        .map(|(_, kind)| kind)
}

/// Splits a key into the tables it sits under and its leaf name
/// (`engines.whisper.backend` is `["engines", "whisper"]` and `backend`);
/// anything the schema does not list is refused by name.
fn split(key: &str) -> Result<(Vec<&str>, &str), ValidationError> {
    kind_of(key).ok_or_else(|| unknown_key(key))?;
    let (path, name) = key.rsplit_once('.').ok_or_else(|| unknown_key(key))?;
    Ok((path.split('.').collect(), name))
}

/// The table `path` names, created on the way when `create` is set (an
/// intermediate table stays implicit so no empty header is written).
fn table_at<'a>(
    doc: &'a mut DocumentMut,
    path: &[&str],
    key: &str,
    create: bool,
) -> Result<Option<&'a mut toml_edit::Table>, ValidationError> {
    let mut table = doc.as_table_mut();
    let last = path.len().saturating_sub(1);
    for (depth, segment) in path.iter().enumerate() {
        let existed = table.contains_key(segment);
        if !existed && !create {
            return Ok(None);
        }
        let item = table
            .entry(segment)
            .or_insert_with(|| Item::Table(toml_edit::Table::new()));
        let next = item.as_table_mut().ok_or_else(|| ValidationError {
            key: Some(key.to_string()),
            line: None,
            message: format!("{} is not a table", path[..=depth].join(".")),
        })?;
        if !existed {
            next.set_implicit(depth != last);
        }
        table = next;
    }
    Ok(Some(table))
}

fn unknown_key(key: &str) -> ValidationError {
    ValidationError {
        key: Some(key.to_string()),
        line: None,
        message: format!("{key}: unknown key"),
    }
}

/// Turns a JSON value into the TOML value the key declares. A string is
/// coerced (`"5000"` to an integer, `"true"` to a boolean) so a CLI can
/// send what the user typed; a typed JSON value must match.
pub fn coerce(key: &str, value: &Value) -> Result<toml_edit::Value, ValidationError> {
    let kind = kind_of(key).ok_or_else(|| unknown_key(key))?;
    let mismatch = |want: &str| ValidationError {
        key: Some(key.to_string()),
        line: None,
        message: format!("{key}: expected {want}, got {value}"),
    };
    Ok(match (kind, value) {
        (Kind::Text, Value::String(s)) => toml_edit::Value::from(s.as_str()),
        (Kind::Boolean, Value::Bool(b)) => toml_edit::Value::from(*b),
        (Kind::Boolean, Value::String(s)) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => toml_edit::Value::from(true),
            "false" | "0" | "no" | "off" => toml_edit::Value::from(false),
            _ => return Err(mismatch("a boolean")),
        },
        (Kind::Integer, Value::Number(n)) => match n.as_i64() {
            Some(i) => toml_edit::Value::from(i),
            None => return Err(mismatch("an integer")),
        },
        (Kind::Integer, Value::String(s)) => match s.trim().parse::<i64>() {
            Ok(i) => toml_edit::Value::from(i),
            Err(_) => return Err(mismatch("an integer")),
        },
        (Kind::Float, Value::Number(n)) => match n.as_f64() {
            Some(f) => toml_edit::Value::from(f),
            None => return Err(mismatch("a number")),
        },
        (Kind::Float, Value::String(s)) => match s.trim().parse::<f64>() {
            Ok(f) => toml_edit::Value::from(f),
            Err(_) => return Err(mismatch("a number")),
        },
        (Kind::List, Value::Array(items)) => {
            let mut array = toml_edit::Array::new();
            for item in items {
                match item {
                    Value::String(s) => array.push(s.as_str()),
                    _ => return Err(mismatch("a list of strings")),
                }
            }
            toml_edit::Value::from(array)
        }
        (Kind::List, Value::String(s)) => {
            let mut array = toml_edit::Array::new();
            for item in s.split(',').map(str::trim).filter(|i| !i.is_empty()) {
                array.push(item);
            }
            toml_edit::Value::from(array)
        }
        (Kind::Table, Value::Object(map)) => {
            let mut table = toml_edit::InlineTable::new();
            for (k, v) in map {
                match v {
                    Value::String(s) => {
                        table.insert(k, toml_edit::Value::from(s.as_str()));
                    }
                    _ => return Err(mismatch("a table of strings")),
                }
            }
            toml_edit::Value::from(table)
        }
        (Kind::Table, Value::String(s)) => {
            let mut table = toml_edit::InlineTable::new();
            for pair in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
                let Some((k, v)) = pair.split_once('=') else {
                    return Err(mismatch("a table of strings (key=value, ...)"));
                };
                table.insert(k.trim(), toml_edit::Value::from(v.trim()));
            }
            toml_edit::Value::from(table)
        }
        (Kind::List, _) => return Err(mismatch("a list of strings")),
        (Kind::Table, _) => return Err(mismatch("a table of strings")),
        (Kind::Text, _) => return Err(mismatch("a string")),
        (Kind::Boolean, _) => return Err(mismatch("a boolean")),
        (Kind::Integer, _) => return Err(mismatch("an integer")),
        (Kind::Float, _) => return Err(mismatch("a number")),
    })
}

fn parse_document(text: &str) -> Result<DocumentMut, ValidationError> {
    text.parse::<DocumentMut>().map_err(|e| ValidationError {
        key: None,
        line: e.span().map(|s| super::validate::line_of(text, s.start)),
        message: e.message().to_string(),
    })
}

/// Returns the file text with `key` set to `value`, every other byte kept.
/// The result is validated against the schema before it is returned.
pub fn set(text: &str, key: &str, value: &Value) -> Result<String, ValidationError> {
    let (path, name) = split(key)?;
    let toml_value = coerce(key, value)?;
    let mut doc = parse_document(text)?;
    let table = table_at(&mut doc, &path, key, true)?.ok_or_else(|| unknown_key(key))?;
    table.set_implicit(false);
    match table.get_mut(name) {
        Some(item) if item.is_value() => {
            let decor = item.as_value().map(|v| v.decor().clone());
            let mut new_value = toml_value;
            if let Some(decor) = decor {
                *new_value.decor_mut() = decor;
            }
            *item = Item::Value(new_value);
        }
        _ => {
            table.insert(name, Item::Value(toml_value));
        }
    }
    let out = doc.to_string();
    validate(&out)?;
    Ok(out)
}

/// Returns the file text with `key` set to a structured `value` (an
/// array of tables or a table of tables), every other byte and every
/// comment kept. This is what `polish.rules.*` and `polish.apps.set`
/// write with: their values are tables, which `set` deliberately does not
/// coerce (ADR 0023). The result is validated before it is returned.
pub fn set_structured(text: &str, key: &str, value: &Value) -> Result<String, ValidationError> {
    let (path, name) = split(key)?;
    let item = item_for(key, value)?;
    let mut doc = parse_document(text)?;
    let table = table_at(&mut doc, &path, key, true)?.ok_or_else(|| unknown_key(key))?;
    table.set_implicit(false);
    if is_empty(value) {
        table.remove(name);
    } else {
        table.insert(name, item);
    }
    let out = doc.to_string();
    validate(&out)?;
    Ok(out)
}

fn is_empty(value: &Value) -> bool {
    match value {
        Value::Array(items) => items.is_empty(),
        Value::Object(map) => map.is_empty(),
        Value::Null => true,
        _ => false,
    }
}

/// A JSON value as the TOML item it renders to, by way of a one-key
/// document so an array of tables comes out as `[[key]]` rather than an
/// inline array.
fn item_for(key: &str, value: &Value) -> Result<Item, ValidationError> {
    let invalid = |message: String| ValidationError {
        key: Some(key.to_string()),
        line: None,
        message,
    };
    let wrapper = serde_json::json!({ "value": value });
    let rendered = toml::to_string(&wrapper)
        .map_err(|e| invalid(format!("{key}: cannot be written as TOML: {e}")))?;
    let doc = parse_document(&rendered)?;
    Ok(doc
        .get("value")
        .cloned()
        .unwrap_or(Item::Value(toml_edit::Value::from(toml_edit::Array::new()))))
}

/// Returns the file text with `key` removed, so its default applies.
pub fn unset(text: &str, key: &str) -> Result<String, ValidationError> {
    let (path, name) = split(key)?;
    let mut doc = parse_document(text)?;
    if let Some(table) = table_at(&mut doc, &path, key, false)? {
        // The removed key's leading comment describes the key that follows
        // it in the user's eyes as much as the key itself, so it moves to
        // the next key instead of vanishing.
        let order: Vec<String> = table.iter().map(|(k, _)| k.to_string()).collect();
        let prefix = table
            .key(name)
            .and_then(|k| k.leaf_decor().prefix())
            .and_then(|p| p.as_str().map(str::to_string));
        if table.remove(name).is_some() {
            let next = order.iter().skip_while(|k| *k != name).nth(1);
            if let (Some(prefix), Some(next)) = (prefix, next) {
                if let Some(mut key) = table.key_mut(next) {
                    let decor = key.leaf_decor_mut();
                    let existing = decor
                        .prefix()
                        .and_then(|p| p.as_str())
                        .unwrap_or("")
                        .to_string();
                    decor.set_prefix(format!("{prefix}{existing}"));
                }
            }
        }
    }
    let out = doc.to_string();
    validate(&out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const FILE: &str = "# top comment\n\n[daemon]\n# keep me\nlog_level = \"info\" # trailing\nshutdown_timeout_ms = 5000\n\n[qa]\nmode = false\n";

    #[test]
    fn set_keeps_comments_ordering_and_trailing_decor() {
        let out = set(FILE, "daemon.log_level", &json!("debug")).unwrap();
        assert_eq!(
            out,
            "# top comment\n\n[daemon]\n# keep me\nlog_level = \"debug\" # trailing\nshutdown_timeout_ms = 5000\n\n[qa]\nmode = false\n"
        );
    }

    #[test]
    fn set_coerces_strings_to_the_declared_type_and_adds_missing_sections() {
        let out = set(FILE, "daemon.shutdown_timeout_ms", &json!("250")).unwrap();
        assert!(out.contains("shutdown_timeout_ms = 250\n"), "{out}");
        let out = set(FILE, "ipc.max_line_bytes", &json!(65536)).unwrap();
        assert!(out.contains("[ipc]\nmax_line_bytes = 65536\n"), "{out}");
        let out = set(FILE, "qa.mode", &json!("yes")).unwrap();
        assert!(out.contains("mode = true"), "{out}");
    }

    #[test]
    fn set_refuses_unknown_keys_and_bad_values_naming_the_key() {
        let err = set(FILE, "daemon.colour", &json!("blue")).unwrap_err();
        assert_eq!(err.key.as_deref(), Some("daemon.colour"));
        assert_eq!(err.message, "daemon.colour: unknown key");
        let err = set(FILE, "daemon.log_level", &json!("loud")).unwrap_err();
        assert_eq!(err.key.as_deref(), Some("daemon.log_level"));
        assert!(err.message.contains("unknown variant `loud`"));
        let err = set(FILE, "daemon.shutdown_timeout_ms", &json!("soon")).unwrap_err();
        assert_eq!(err.key.as_deref(), Some("daemon.shutdown_timeout_ms"));
        assert_eq!(
            err.message,
            "daemon.shutdown_timeout_ms: expected an integer, got \"soon\""
        );
        assert!(set(FILE, "nonsense", &json!(1)).is_err());
    }

    #[test]
    fn unset_removes_the_key_and_keeps_the_rest() {
        let out = unset(FILE, "daemon.log_level").unwrap();
        assert_eq!(
            out,
            "# top comment\n\n[daemon]\n# keep me\nshutdown_timeout_ms = 5000\n\n[qa]\nmode = false\n"
        );
        assert_eq!(unset(FILE, "ipc.socket").unwrap(), FILE);
        assert!(unset(FILE, "daemon.colour").is_err());
    }

    #[test]
    fn keys_cover_every_section_in_order() {
        let names: Vec<String> = keys().into_iter().map(|(k, _)| k).collect();
        assert_eq!(
            names[0], "audio.input_device",
            "sections sort alphabetically"
        );
        assert!(names.contains(&"qa.mode".to_string()));
        assert!(names.contains(&"engines.whisper.backend".to_string()));
        assert!(names.contains(&"engines.llm.context_length".to_string()));
        assert!(!names.contains(&"engines.whisper".to_string()));
        assert_eq!(kind_of("engines.llm.context_length"), Some(Kind::Integer));
        assert_eq!(kind_of("dictation.replacements"), Some(Kind::Table));
        assert_eq!(kind_of("engines.whisper"), None);
        assert_eq!(kind_of("qa.mode"), Some(Kind::Boolean));
        assert_eq!(kind_of("ipc.max_line_bytes"), Some(Kind::Integer));
        assert_eq!(kind_of("insert.terminal_app_ids"), Some(Kind::List));
        assert_eq!(kind_of("insert.paste_keys"), Some(Kind::Table));
        assert_eq!(kind_of("nope"), None);
    }

    #[test]
    fn lists_and_tables_coerce_from_json_and_from_typed_text() {
        let out = set(FILE, "insert.terminal_app_ids", &json!(["foot", "kitty"])).unwrap();
        assert!(
            out.contains("terminal_app_ids = [\"foot\", \"kitty\"]"),
            "{out}"
        );
        let out = set(
            FILE,
            "insert.self_app_ids",
            &json!("dettivo-app, dettivo-osd"),
        )
        .unwrap();
        assert!(
            out.contains("self_app_ids = [\"dettivo-app\", \"dettivo-osd\"]"),
            "{out}"
        );
        let out = set(FILE, "insert.paste_keys", &json!({"foot": "ctrl+shift+v"})).unwrap();
        assert!(
            out.contains("paste_keys = { foot = \"ctrl+shift+v\" }"),
            "{out}"
        );
        let out = set(
            FILE,
            "insert.paste_keys",
            &json!("foot=ctrl+shift+v, Code=ctrl+v"),
        )
        .unwrap();
        assert!(out.contains("Code = \"ctrl+v\""), "{out}");
        let err = set(FILE, "insert.terminal_app_ids", &json!(5)).unwrap_err();
        assert!(err.message.contains("a list of strings"), "{}", err.message);
        let err = set(FILE, "insert.paste_keys", &json!("nonsense")).unwrap_err();
        assert!(err.message.contains("key=value"), "{}", err.message);
    }

    #[test]
    fn structured_values_are_written_as_tables_and_keep_the_comments() {
        const FILE: &str =
            "# the whole file\n[polish]\n# which transforms run\ntransforms = [\"fixGrammar\"]\n";
        let out = set_structured(
            FILE,
            "polish.rules",
            &json!([{"id": "r1", "name": "House style", "enabled": true, "content": "Keep it short."}]),
        )
        .unwrap();
        assert!(out.contains("# the whole file"), "{out}");
        assert!(out.contains("# which transforms run"), "{out}");
        assert!(out.contains("[[polish.rules]]"), "{out}");
        assert!(out.contains("name = \"House style\""), "{out}");
        let out = set_structured(
            &out,
            "polish.apps",
            &json!({"org.mozilla.Thunderbird": {"preset": "email"}}),
        )
        .unwrap();
        assert!(out.contains("[polish.apps"), "{out}");
        assert!(out.contains("preset = \"email\""), "{out}");
        // An empty value removes the key rather than writing an empty
        // table the schema would have to read back.
        let out = set_structured(&out, "polish.rules", &json!([])).unwrap();
        assert!(!out.contains("[[polish.rules]]"), "{out}");
        assert!(out.contains("# which transforms run"), "{out}");
    }

    #[test]
    fn nested_section_keys_are_written_under_their_own_header() {
        let out = set(FILE, "engines.whisper.backend", &json!("cpu")).unwrap();
        assert!(
            out.ends_with("[engines.whisper]\nbackend = \"cpu\"\n"),
            "{out}"
        );
        assert!(
            !out.contains("[engines]\n"),
            "no empty parent header: {out}"
        );
        let out = set(&out, "engines.llm.context_length", &json!("2048")).unwrap();
        assert!(
            out.contains("[engines.llm]\ncontext_length = 2048\n"),
            "{out}"
        );
        let out = unset(&out, "engines.whisper.backend").unwrap();
        assert!(!out.contains("backend = \"cpu\""), "{out}");
        assert!(out.contains("context_length = 2048"), "{out}");
        assert_eq!(unset(FILE, "engines.parakeet.backend").unwrap(), FILE);
        assert!(set(FILE, "engines.whisper", &json!({"backend": "cpu"})).is_err());
        let err = set(FILE, "engines.llm.backend", &json!("cuda")).unwrap_err();
        assert_eq!(err.key.as_deref(), Some("engines.llm.backend"));
    }
}
