//! A list or table value typed as it appears in the file, `["a", "b"]` or
//! `{ foot = "ctrl+shift+v" }`, read as TOML so `dettivo config set` takes
//! what its help promises instead of splitting the brackets on commas.

use serde_json::{Map, Value};
use toml_edit::DocumentMut;

/// The JSON form of `text` when it is a TOML array or inline table of
/// strings; `None` when it is neither or does not parse, so the caller
/// falls back to the comma form or reports the type it wanted.
pub fn parse(text: &str) -> Option<Value> {
    let text = text.trim();
    if !text.starts_with(['[', '{']) {
        return None;
    }
    let doc = format!("v = {text}").parse::<DocumentMut>().ok()?;
    let value = doc.get("v")?.as_value()?;
    if let Some(array) = value.as_array() {
        let items = array
            .iter()
            .map(|v| v.as_str().map(|s| Value::String(s.into())));
        return items.collect::<Option<Vec<_>>>().map(Value::Array);
    }
    let table = value.as_inline_table()?;
    let entries = table
        .iter()
        .map(|(k, v)| v.as_str().map(|s| (k.to_string(), Value::String(s.into()))));
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
        assert_eq!(parse("SapienXT, Dettivo"), None);
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
    }
}
