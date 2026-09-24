//! The key registry behind `config.keys` (ADR 0033): every leaf key of the
//! schema with its section, its declared type, its default and the
//! sentence the default file documents it with, read from the comments of
//! `DEFAULT_TOML` so the file, the CLI and the settings routes describe a
//! key with the same words.

use dettivo_proto::methods::config::{KeyInfo, KeyKind};
use serde_json::Value;
use toml_edit::{DocumentMut, Item};

use super::edit::{self, Kind};
use super::schema::{Config, DEFAULT_TOML};

impl From<Kind> for KeyKind {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Text => Self::Text,
            Kind::Integer => Self::Integer,
            Kind::Float => Self::Float,
            Kind::Boolean => Self::Boolean,
            Kind::List => Self::List,
            Kind::Table => Self::Table,
        }
    }
}

/// Every key of the schema in file order with its documentation.
pub fn describe() -> Vec<KeyInfo> {
    let defaults = serde_json::to_value(Config::default()).unwrap_or(Value::Null);
    let doc = DEFAULT_TOML.parse::<DocumentMut>().ok();
    edit::keys()
        .into_iter()
        .map(|(key, kind)| {
            let (section, _) = key.rsplit_once('.').unwrap_or((&key, ""));
            let default = key
                .split('.')
                .try_fold(&defaults, |node, segment| node.get(segment))
                .cloned()
                .unwrap_or(Value::Null);
            KeyInfo {
                section: section.to_string(),
                kind: kind.into(),
                default,
                doc: doc.as_ref().map(|d| doc_of(d, &key)).unwrap_or_default(),
                key,
            }
        })
        .collect()
}

/// The comment lines above `key` in the default file, joined into one
/// paragraph; a section's introduction that sits above its first key is
/// part of that key's sentence.
fn doc_of(doc: &DocumentMut, key: &str) -> String {
    let (path, leaf) = key.rsplit_once('.').unwrap_or((key, ""));
    let mut item: &Item = doc.as_item();
    for segment in path.split('.') {
        let Some(next) = item.get(segment) else {
            return String::new();
        };
        item = next;
    }
    let Some(table) = item.as_table() else {
        return String::new();
    };
    let prefix = table
        .key(leaf)
        .and_then(|k| k.leaf_decor().prefix())
        .and_then(|p| p.as_str().map(str::to_string))
        .unwrap_or_default();
    prefix
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix('#'))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_carries_its_section_type_default_and_sentence() {
        let keys = describe();
        let names: Vec<&str> = keys.iter().map(|k| k.key.as_str()).collect();
        assert_eq!(names.len(), edit::keys().len());
        let find = |k: &str| keys.iter().find(|i| i.key == k).unwrap().clone();
        let level = find("daemon.log_level");
        assert_eq!(level.section, "daemon");
        assert_eq!(level.kind, KeyKind::Text);
        assert_eq!(level.default, Value::String("info".into()));
        assert!(
            level.doc.starts_with("Lowest level written to the journal"),
            "{}",
            level.doc
        );
        let backend = find("engines.whisper.backend");
        assert_eq!(backend.section, "engines.whisper");
        assert!(backend.doc.contains("Compute backend"), "{}", backend.doc);
        let context = find("engines.llm.context_length");
        assert_eq!(context.kind, KeyKind::Integer);
        assert_eq!(context.default, Value::from(4096));
        for info in &keys {
            assert!(
                !info.doc.is_empty(),
                "{} has no sentence in DEFAULT_TOML",
                info.key
            );
        }
    }
}
