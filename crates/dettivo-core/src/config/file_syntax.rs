//! A list or table value typed as it appears in the file, `["a", "b"]` or
//! `{ foot = "ctrl+shift+v" }`, read as TOML so `dettivo config set` takes
//! what its help promises instead of splitting the brackets on commas,
//! and a JSON list written back as the TOML array it stands for.

use serde_json::{Map, Value};
use toml_edit::DocumentMut;

/// Whether `text` is written as a TOML array or inline table, `[...]` or
/// `{...}`, rather than in the comma form.
pub fn is_file_syntax(text: &str) -> bool {
    text.trim_start().starts_with(['[', '{'])
}

/// The JSON form of `text` when it is a TOML array or inline table of
/// strings, or an array whose items are strings or inline tables of
/// strings and string lists (`dictation.vocabulary` entries); text that is
/// no TOML value is read as JSON, as the contract writes the same value.
/// `None` when it parses as neither or holds anything else.
pub fn parse(text: &str) -> Option<Value> {
    let text = text.trim();
    let Ok(doc) = format!("v = {text}").parse::<DocumentMut>() else {
        let json = serde_json::from_str::<Value>(text).ok();
        return json.filter(|v| v.is_array() || v.is_object());
    };
    let value = doc.get("v")?.as_value()?;
    if let Some(array) = value.as_array() {
        let items = array.iter().map(|v| match v.as_inline_table() {
            Some(table) => table_of(table, true),
            None => string(v),
        });
        return items.collect::<Option<Vec<_>>>().map(Value::Array);
    }
    table_of(value.as_inline_table()?, false)
}

fn string(value: &toml_edit::Value) -> Option<Value> {
    value.as_str().map(|s| Value::String(s.into()))
}

fn table_of(table: &toml_edit::InlineTable, lists: bool) -> Option<Value> {
    let entries = table.iter().map(|(k, v)| {
        let value = match v.as_array() {
            Some(items) if lists => Value::Array(items.iter().map(string).collect::<Option<_>>()?),
            _ => string(v)?,
        };
        Some((k.to_string(), value))
    });
    entries.collect::<Option<Map<_, _>>>().map(Value::Object)
}

/// A JSON list as the TOML array it writes: strings, and tables that are
/// `dictation.vocabulary` entries (the schema refuses a table under any
/// other list key); `None` for any other item.
pub fn list(items: &[Value]) -> Option<toml_edit::Array> {
    let mut array = toml_edit::Array::new();
    for item in items {
        match item {
            Value::String(s) => array.push(s.as_str()),
            Value::Object(map) => array.push(list_item_table(map)?),
            _ => return None,
        }
    }
    Some(array)
}

/// A list item's table: string values, or lists of strings. `term` is
/// written first, as the file documents the entry, since a JSON object
/// arrives with its keys sorted.
fn list_item_table(map: &Map<String, Value>) -> Option<toml_edit::InlineTable> {
    let mut table = toml_edit::InlineTable::new();
    let term = map.get_key_value("term").into_iter();
    for (k, v) in term.chain(map.iter().filter(|(k, _)| *k != "term")) {
        let value = match v {
            Value::String(s) => toml_edit::Value::from(s.as_str()),
            Value::Array(items) => {
                let strings = items.iter().map(Value::as_str);
                toml_edit::Value::from(strings.collect::<Option<toml_edit::Array>>()?)
            }
            _ => return None,
        };
        table.insert(k, value);
    }
    Some(table)
}

/// Keeps a hand-written multi-line list's layout when `old` is replaced
/// by `new`: an entry that survives (the same term) keeps the comment lines
/// above it and the comment after its comma, a new entry takes a line of
/// its own at the list's indent, and the closing bracket stays on its
/// line. A list written on one line is left as `new` writes it.
pub fn keep_layout(old: &toml_edit::Array, new: &mut toml_edit::Array) {
    let raw =
        |r: Option<&toml_edit::RawString>| r.and_then(|r| r.as_str()).unwrap_or("").to_string();
    let prefixes: Vec<String> = old.iter().map(|v| raw(v.decor().prefix())).collect();
    let trailing = raw(Some(old.trailing()));
    if !prefixes.iter().chain([&trailing]).any(|p| p.contains('\n')) {
        return;
    }
    // A prefix holds the comment ending the previous line (the head) and
    // the lines above its own entry (the tail).
    let split = |s: &str| match s.find('\n') {
        Some(i) => (s[..i].to_string(), s[i..].to_string()),
        None => (s.to_string(), String::new()),
    };
    let (mut heads, above): (Vec<String>, Vec<String>) = prefixes.iter().map(|p| split(p)).unzip();
    let (last_head, closing) = split(&trailing);
    heads.push(last_head);
    let indent = above
        .iter()
        .find_map(|t| t.rsplit_once('\n'))
        .map_or("  ", |(_, i)| i);
    let fresh = format!("\n{indent}");
    let key = |v: &toml_edit::Value| {
        let term = v.as_inline_table().and_then(|t| t.get("term"));
        term.unwrap_or(v).as_str().map(str::to_string)
    };
    let mut used = vec![false; prefixes.len()];
    let mut after = heads[0].clone();
    for item in new.iter_mut() {
        let found = (0..used.len()).find(|&i| !used[i] && key(old.get(i).unwrap()) == key(item));
        let (lines, next, suffix) = match found {
            Some(i) => {
                used[i] = true;
                let suffix = raw(old.get(i).unwrap().decor().suffix());
                (above[i].clone(), heads[i + 1].clone(), suffix)
            }
            None => (fresh.clone(), String::new(), String::new()),
        };
        let lines = if lines.is_empty() && after.contains('#') {
            fresh.clone()
        } else {
            lines
        };
        item.decor_mut().set_prefix(format!("{after}{lines}"));
        item.decor_mut().set_suffix(suffix);
        after = next;
    }
    let closing = if closing.is_empty() && after.contains('#') {
        "\n".into()
    } else {
        closing
    };
    new.set_trailing_comma(old.trailing_comma() && !new.is_empty());
    new.set_trailing(format!("{after}{closing}"));
}

#[cfg(test)]
mod tests {
    use super::parse;
    use serde_json::json;

    #[test]
    fn reads_arrays_and_inline_tables_of_strings() {
        assert_eq!(
            parse(r#"["SapienXT", "Dettivo"]"#),
            Some(json!(["SapienXT", "Dettivo"]))
        );
        assert_eq!(
            parse(r#" { foot = "ctrl+shift+v" } "#),
            Some(json!({"foot": "ctrl+shift+v"}))
        );
        assert!(!super::is_file_syntax("SapienXT, Dettivo"));
        assert_eq!(parse("[1, 2]"), None);
        assert_eq!(parse("[\"unclosed\""), None);
    }

    #[test]
    fn config_set_writes_a_list_typed_as_in_the_file() {
        let value = json!(r#"["SapienXT", "Dettivo"]"#);
        let out = crate::config::edit::set("", "dictation.vocabulary", &value).unwrap();
        assert!(
            out.contains(r#"vocabulary = ["SapienXT", "Dettivo"]"#),
            "{out}"
        );
        let err =
            crate::config::edit::set("", "dictation.vocabulary", &json!("[1, 2]")).unwrap_err();
        assert!(err.message.contains("a list of terms"), "{}", err.message);
    }

    #[test]
    fn config_set_writes_vocabulary_tables_and_keeps_comments_and_form() {
        const FILE: &str =
            "# mine\n[dictation]\n# names I use\nvocabulary = [\"Dettivo\"] # trailing\n";
        let typed = r#"["Dettivo", { term = "Clef", sounds_like = ["clay", "cleff"] }]"#;
        let as_json = json!(["Dettivo", {"term": "Clef", "sounds_like": ["clay", "cleff"]}]);
        // Typed as in the file, sent as JSON, or typed as JSON on the CLI.
        for value in [json!(typed), json!(as_json.to_string()), as_json] {
            let out = crate::config::edit::set(FILE, "dictation.vocabulary", &value).unwrap();
            assert_eq!(
                out,
                format!("# mine\n[dictation]\n# names I use\nvocabulary = {typed} # trailing\n")
            );
            let config = crate::config::validate::validate(&out).unwrap();
            assert_eq!(config.dictation.vocabulary[1].aliases(), ["clay", "cleff"]);
        }
        // A table is a vocabulary entry only; another list key refuses it.
        let err =
            crate::config::edit::set("", "insert.terminal_app_ids", &json!(r#"[{ term = "x" }]"#))
                .unwrap_err();
        assert_eq!(err.key.as_deref(), Some("insert.terminal_app_ids[0]"));
        let err = crate::config::edit::set(
            "",
            "dictation.vocabulary",
            &json!([{"term": "x", "sounds_like": [1]}]),
        )
        .unwrap_err();
        assert!(
            err.message.contains("{ term, sounds_like } tables"),
            "{}",
            err.message
        );
    }

    #[test]
    fn config_set_keeps_a_multi_line_list_and_its_entries_comments() {
        let file = |entries: &str| format!("[dictation]\n# mine\nvocabulary = [\n{entries}]\n");
        let before = file(concat!(
            "  # names\n",
            "  \"Dettivo\", # plain string\n",
            "  { term = \"Clef\", sounds_like = [\"clay\", \"cleff\"] }, # table form\n",
            "  \"Omarchy\",\n",
        ));
        let value = json!(r#"["Dettivo", { term = "Clef", sounds_like = ["clay"] }, "SapienXT"]"#);
        let out = crate::config::edit::set(&before, "dictation.vocabulary", &value).unwrap();
        let edited = file(concat!(
            "  # names\n",
            "  \"Dettivo\", # plain string\n",
            "  { term = \"Clef\", sounds_like = [\"clay\"] }, # table form\n",
            "  \"SapienXT\",\n",
        ));
        assert_eq!(out, edited);
        let value = json!(["Dettivo", "SapienXT"]);
        let out = crate::config::edit::set(&out, "dictation.vocabulary", &value).unwrap();
        let removed = file("  # names\n  \"Dettivo\", # plain string\n  \"SapienXT\",\n");
        assert_eq!(out, removed);
    }
}
