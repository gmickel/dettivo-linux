//! A list or table value typed as it appears in the file, `["a", "b"]` or
//! `{ foot = "ctrl+shift+v" }`, read as TOML so `dettivo config set` takes
//! what its help promises instead of splitting the brackets on commas.

use serde_json::{Map, Value};
use toml_edit::DocumentMut;

/// Whether `text` is written as a TOML array or inline table, `[...]` or
/// `{...}`, rather than in the comma form.
pub fn is_file_syntax(text: &str) -> bool {
    text.trim_start().starts_with(['[', '{'])
}

/// The JSON form of `text` when it is a TOML array or inline table of
/// strings, or an array whose items are strings or inline tables of
/// strings and string lists (`dictation.vocabulary` entries); `None` when
/// it does not parse or holds anything else.
pub fn parse(text: &str) -> Option<Value> {
    let text = text.trim();
    let doc = format!("v = {text}").parse::<DocumentMut>().ok()?;
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
        assert!(err.message.contains("a list of strings"), "{}", err.message);
    }

    #[test]
    fn config_set_writes_vocabulary_tables_and_keeps_comments_and_form() {
        const FILE: &str =
            "# mine\n[dictation]\n# names I use\nvocabulary = [\"Dettivo\"] # trailing\n";
        let typed = r#"["Dettivo", { term = "Clef", sounds_like = ["clay", "cleff"] }]"#;
        let as_json = json!(["Dettivo", {"term": "Clef", "sounds_like": ["clay", "cleff"]}]);
        for value in [json!(typed), as_json] {
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
        assert!(err.message.contains("a list of strings"), "{}", err.message);
    }
}
