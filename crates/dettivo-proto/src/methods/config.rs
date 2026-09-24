//! `config.*`: the Linux addition that lets every client read and write
//! the configuration file through the daemon (ADR 0009), so a hand edit, a
//! CLI `config set` and a GUI settings route all validate on the same path.
//! Signalled by `system.capabilities.config.methods`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Where an effective value came from, in override order (a later source
/// wins over an earlier one).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// The built-in default; the key is absent from the file and the environment.
    Default,
    /// The value written in `config.toml`.
    File,
    /// An environment variable override, which wins over the file.
    Environment,
}

/// One effective key: its dotted name, the value in force and its source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Dotted key, `section.name`.
    pub key: String,
    /// The value in force, as JSON.
    pub value: Value,
    /// Where that value came from.
    pub source: Source,
}

/// `config.get`: one key, or every key when `key` is absent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetParams {
    /// The key to read; every key when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// Result of `config.get`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetResult {
    /// The requested keys in file order.
    pub entries: Vec<Entry>,
}

/// Result of `config.set`: the entry as it now reads.
pub type SetResult = Entry;

/// `config.set`: a JSON string is coerced to the key's declared type, so a
/// CLI can send what the user typed; a typed JSON value must already match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetParams {
    /// Dotted key to write.
    pub key: String,
    /// The new value, typed or as the string the user typed.
    pub value: Value,
}

/// `config.unset`: removes the key from the file so its default applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnsetParams {
    /// Dotted key to remove.
    pub key: String,
}

/// Result of `config.unset`: the entry as it now reads (its default).
pub type UnsetResult = Entry;

/// One validation finding: the key when it is known, the line when the
/// file names one, and always a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationError {
    /// The offending key, when the finding is about one.
    pub key: Option<String>,
    /// The 1-based line in the file, when the finding points at one.
    pub line: Option<u32>,
    /// What is wrong, in one sentence.
    pub message: String,
}

/// `config.validate`: the file as it is on disk right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidateResult {
    /// True when the file parses and every key is known and well typed.
    pub ok: bool,
    /// The file that was validated.
    pub path: String,
    /// Every finding; empty when `ok`.
    pub errors: Vec<ValidationError>,
}

/// `config.path`: every location the daemon resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathResult {
    /// `config.toml`.
    pub config: String,
    /// `state.toml`, runtime state that is not configuration.
    pub state: String,
    /// The data directory (history, models).
    pub data: String,
    /// The per-session runtime directory that holds the socket.
    pub runtime: String,
    /// The Unix socket the daemon listens on.
    pub socket: String,
    /// The `0600` token file consulted in `peer_token` mode.
    pub token_file: String,
}

/// The declared type of a key, as `config.keys` reports it and as
/// `config.set` coerces a string to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyKind {
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

/// One key of the schema: its dotted name, the section it sits in, its
/// declared type, its default and the sentence the default file documents
/// it with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyInfo {
    /// Dotted key, `section.name` (`engines.whisper.backend` under a
    /// nested section).
    pub key: String,
    /// The TOML table the key lives in (`engines.whisper`).
    pub section: String,
    /// The declared type.
    pub kind: KeyKind,
    /// The built-in default, as JSON.
    pub default: Value,
    /// The comment the default file carries above the key.
    pub doc: String,
}

/// Result of `config.keys`: every key of the schema in file order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeysResult {
    /// The keys, sorted by section and then by name.
    pub keys: Vec<KeyInfo>,
}

/// `config.print_default`: the fully commented default file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrintDefaultResult {
    /// The default file, every key present and commented.
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&Source::Environment).unwrap(),
            "\"environment\""
        );
    }

    #[test]
    fn key_kinds_serialize_snake_case() {
        assert_eq!(
            serde_json::to_string(&KeyKind::Boolean).unwrap(),
            "\"boolean\""
        );
        let info: KeyInfo = serde_json::from_str(
            r#"{"key":"qa.mode","section":"qa","kind":"boolean","default":false,"doc":"QA mode."}"#,
        )
        .unwrap();
        assert_eq!(info.kind, KeyKind::Boolean);
    }

    #[test]
    fn get_params_accepts_an_empty_object() {
        let p: GetParams = serde_json::from_str("{}").unwrap();
        assert_eq!(p.key, None);
    }
}
