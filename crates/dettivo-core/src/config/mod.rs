//! `config.toml`: the schema, loading with environment overrides and
//! per-key sources, validation that names key and line, and
//! comment-preserving edits (ADR 0009).

pub mod audio_schema;
pub mod default_toml;
pub mod edit;
pub mod import_schema;
pub mod keys;
pub mod llm_schema;
pub mod omarchy_schema;
pub mod polish_schema;
pub mod rest_schema;
pub mod schema;
pub mod speech_schema;
pub mod validate;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use dettivo_proto::methods::config::{Entry, Source, ValidationError};
use serde_json::Value;

pub use schema::{Config, DEFAULT_TOML, LogLevel};

use crate::atomic;
use crate::paths::{self, Paths};

/// The configuration as loaded: the typed values in force, where each key
/// came from, and the validation finding when the file did not parse (the
/// daemon then runs on defaults and reports through `system.health`).
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    /// The file that was (or would have been) read.
    pub path: PathBuf,
    /// The file's text, `None` when it does not exist.
    pub text: Option<String>,
    /// Values in force after file and environment.
    pub config: Config,
    /// Source per dotted key; keys absent here are defaults.
    pub sources: BTreeMap<String, Source>,
    /// The first validation finding, when the file is unusable.
    pub error: Option<ValidationError>,
}

impl Loaded {
    /// Reads `path` and applies the environment overrides `env` yields.
    pub fn load(path: &Path, env: impl Fn(&str) -> Option<OsString>) -> Self {
        let text = match fs::read_to_string(path) {
            Ok(t) => Some(t),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => {
                return Self {
                    path: path.to_path_buf(),
                    text: None,
                    config: Config::default(),
                    sources: BTreeMap::new(),
                    error: Some(ValidationError {
                        key: None,
                        line: None,
                        message: format!("cannot read {}: {e}", path.display()),
                    }),
                }
                .with_env(env);
            }
        };
        let (config, sources, error) = match text.as_deref() {
            None => (Config::default(), BTreeMap::new(), None),
            Some(t) => match validate::validate(t) {
                Ok(config) => (config, file_sources(t), None),
                Err(err) => (Config::default(), BTreeMap::new(), Some(err)),
            },
        };
        Self {
            path: path.to_path_buf(),
            text,
            config,
            sources,
            error,
        }
        .with_env(env)
    }

    /// The snapshot a reload installs: this load when the file parsed or
    /// is absent, else `previous`'s values with this load's text and
    /// finding, so an edit that breaks the file never drops what was in
    /// force (the `peer_token` requirement, retention) to the defaults.
    pub fn retaining(mut self, previous: &Loaded) -> Self {
        if self.error.is_some() && self.text.is_some() {
            self.config = previous.config.clone();
            self.sources = previous.sources.clone();
        }
        self
    }

    fn with_env(mut self, env: impl Fn(&str) -> Option<OsString>) -> Self {
        if let Some(v) = env(paths::ENV_SOCKET).filter(|v| !v.is_empty()) {
            self.config.ipc.socket = v.to_string_lossy().into_owned();
            self.sources
                .insert("ipc.socket".into(), Source::Environment);
        }
        if let Some(v) = env(paths::ENV_DATA_DIR).filter(|v| !v.is_empty()) {
            self.config.paths.data_dir = v.to_string_lossy().into_owned();
            self.sources
                .insert("paths.data_dir".into(), Source::Environment);
        }
        if let Some(v) = env(paths::ENV_QA) {
            self.config.qa.mode = paths::env_flag(&v);
            self.sources.insert("qa.mode".into(), Source::Environment);
        }
        self
    }

    /// The effective value of every key, or of one key, with its source.
    /// Empty path keys read as the location they resolve to.
    pub fn entries(&self, key: Option<&str>, base: &Paths) -> Result<Vec<Entry>, ValidationError> {
        let json = serde_json::to_value(&self.config).unwrap_or(Value::Null);
        let mut out = Vec::new();
        for (name, _) in edit::keys() {
            if key.is_some_and(|k| k != name) {
                continue;
            }
            let raw = name
                .split('.')
                .try_fold(&json, |node, segment| node.get(segment))
                .cloned()
                .unwrap_or(Value::Null);
            let (value, source) = self.resolved(&name, raw, base);
            out.push(Entry {
                key: name.clone(),
                value,
                source,
            });
        }
        if let Some(k) = key {
            if out.is_empty() {
                return Err(ValidationError {
                    key: Some(k.to_string()),
                    line: None,
                    message: format!("{k}: unknown key"),
                });
            }
        }
        Ok(out)
    }

    fn resolved(&self, key: &str, raw: Value, base: &Paths) -> (Value, Source) {
        let source = self.sources.get(key).copied().unwrap_or(Source::Default);
        let empty = raw.as_str().is_some_and(str::is_empty);
        match key {
            "ipc.socket" if empty => (path_value(&self.socket(base)), base.socket_source),
            "paths.data_dir" if empty => (path_value(&self.data_dir(base)), base.data_dir_source),
            "paths.models_dir" if empty => (path_value(&self.models_dir(base)), Source::Default),
            "ipc.token_file" if empty => (path_value(&self.token_file(base)), Source::Default),
            "history.db_path" if empty => (path_value(&self.db_path(base)), Source::Default),
            _ => (raw, source),
        }
    }

    /// The socket path in force: environment, then file, then default.
    pub fn socket(&self, base: &Paths) -> PathBuf {
        if base.socket_source == Source::Environment || self.config.ipc.socket.is_empty() {
            base.socket.clone()
        } else {
            PathBuf::from(&self.config.ipc.socket)
        }
    }

    /// The data directory in force.
    pub fn data_dir(&self, base: &Paths) -> PathBuf {
        if base.data_dir_source == Source::Environment || self.config.paths.data_dir.is_empty() {
            base.data_dir.clone()
        } else {
            PathBuf::from(&self.config.paths.data_dir)
        }
    }

    /// The models directory in force.
    pub fn models_dir(&self, base: &Paths) -> PathBuf {
        if self.config.paths.models_dir.is_empty() {
            self.data_dir(base).join("models")
        } else {
            PathBuf::from(&self.config.paths.models_dir)
        }
    }

    /// The history database in force.
    pub fn db_path(&self, base: &Paths) -> PathBuf {
        if self.config.history.db_path.is_empty() {
            self.data_dir(base).join("dettivo.db")
        } else {
            PathBuf::from(&self.config.history.db_path)
        }
    }

    /// The token file in force.
    pub fn token_file(&self, base: &Paths) -> PathBuf {
        if self.config.ipc.token_file.is_empty() {
            base.token_file.clone()
        } else {
            PathBuf::from(&self.config.ipc.token_file)
        }
    }
}

fn path_value(path: &Path) -> Value {
    Value::String(path.to_string_lossy().into_owned())
}

/// Which keys the file text sets, so each can be labelled `File`.
fn file_sources(text: &str) -> BTreeMap<String, Source> {
    fn walk(prefix: &str, table: &toml_edit::Table, out: &mut BTreeMap<String, Source>) {
        for (name, item) in table.iter() {
            let key = format!("{prefix}.{name}");
            match item.as_table() {
                Some(nested) if edit::is_section(&key) => walk(&key, nested, out),
                _ => {
                    out.insert(key, Source::File);
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    let Ok(doc) = text.parse::<toml_edit::DocumentMut>() else {
        return out;
    };
    for (section, item) in doc.iter() {
        if let Some(table) = item.as_table() {
            walk(section, table, &mut out);
        }
    }
    out
}

/// Writes new file text atomically (`0600`, directory `0700`). The caller
/// has already validated `text` through `edit::set` or `edit::unset`.
pub fn write_text(path: &Path, text: &str) -> io::Result<()> {
    atomic::write(path, text.as_bytes(), 0o600, 0o700)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, OsString> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), OsString::from(v)))
            .collect();
        move |key| map.get(key).cloned()
    }

    fn base() -> Paths {
        Paths::from_env(env(&[
            ("HOME", "/home/u"),
            ("XDG_RUNTIME_DIR", "/run/user/7"),
        ]))
    }

    #[test]
    fn missing_file_is_defaults_with_resolved_paths() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = Loaded::load(&dir.path().join("config.toml"), env(&[]));
        assert!(loaded.text.is_none());
        assert!(loaded.error.is_none());
        let entries = loaded.entries(Some("ipc.socket"), &base()).unwrap();
        assert_eq!(
            entries[0].value,
            Value::String("/run/user/7/dettivo/dettivo.sock".into())
        );
        assert_eq!(entries[0].source, Source::Default);
        assert!(loaded.entries(Some("nope.key"), &base()).is_err());
    }

    #[test]
    fn file_values_are_labelled_and_environment_wins() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[daemon]\nlog_level = \"debug\"\n[qa]\nmode = true\n[ipc]\nsocket = \"/tmp/file.sock\"\n").unwrap();
        let loaded = Loaded::load(&path, env(&[(paths::ENV_QA, "0")]));
        assert_eq!(loaded.config.daemon.log_level, LogLevel::Debug);
        assert!(!loaded.config.qa.mode);
        let all = loaded.entries(None, &base()).unwrap();
        let find = |k: &str| all.iter().find(|e| e.key == k).unwrap().clone();
        assert_eq!(find("daemon.log_level").source, Source::File);
        assert_eq!(find("qa.mode").source, Source::Environment);
        assert_eq!(find("daemon.shutdown_timeout_ms").source, Source::Default);
        assert_eq!(find("engines.whisper.backend").source, Source::Default);
        assert_eq!(
            find("engines.whisper.backend").value,
            Value::String("auto".into())
        );
        assert_eq!(loaded.socket(&base()), PathBuf::from("/tmp/file.sock"));
        let env_base = Paths::from_env(env(&[
            ("HOME", "/home/u"),
            (paths::ENV_SOCKET, "/tmp/env.sock"),
        ]));
        let loaded = Loaded::load(&path, env(&[(paths::ENV_SOCKET, "/tmp/env.sock")]));
        assert_eq!(loaded.socket(&env_base), PathBuf::from("/tmp/env.sock"));
        assert_eq!(
            loaded.entries(Some("ipc.socket"), &env_base).unwrap()[0].source,
            Source::Environment
        );
    }

    #[test]
    fn nested_section_keys_read_from_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "[engines.whisper]\nbackend = \"cpu\"\n[engines.llm]\ncontext_length = 2048\n",
        )
        .unwrap();
        let loaded = Loaded::load(&path, env(&[]));
        assert!(loaded.error.is_none(), "{:?}", loaded.error);
        let all = loaded.entries(None, &base()).unwrap();
        let find = |k: &str| all.iter().find(|e| e.key == k).unwrap().clone();
        assert_eq!(find("engines.whisper.backend").source, Source::File);
        assert_eq!(
            find("engines.whisper.backend").value,
            Value::String("cpu".into())
        );
        assert_eq!(find("engines.llm.context_length").value, Value::from(2048));
        assert_eq!(find("engines.llm.backend").source, Source::Default);
        assert_eq!(find("engines.parakeet.backend").source, Source::Default);
    }

    #[test]
    fn a_reload_of_a_broken_file_keeps_the_previous_values_and_the_finding() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "[ipc]\nauth_mode = \"peer_token\"\n[history]\naudio_retention_days = 365\n",
        )
        .unwrap();
        let previous = Loaded::load(&path, env(&[]));
        assert!(previous.error.is_none());
        fs::write(
            &path,
            "[ipc]\nauth_mode = \"peer_token\"\n[history]\naudio_retention_days = \n",
        )
        .unwrap();
        let reloaded = Loaded::load(&path, env(&[])).retaining(&previous);
        assert_eq!(reloaded.config, previous.config, "the values in force stay");
        assert_eq!(reloaded.sources, previous.sources);
        assert_eq!(reloaded.error.unwrap().line, Some(4));
        assert!(
            reloaded.text.unwrap().ends_with("= \n"),
            "the text is the file as it is"
        );
        // A file that disappears is the defaults, as at a first start.
        fs::remove_file(&path).unwrap();
        let gone = Loaded::load(&path, env(&[])).retaining(&previous);
        assert_eq!(gone.config, Config::default());
        assert!(gone.error.is_none());
    }

    #[test]
    fn a_broken_file_loads_defaults_and_keeps_the_finding() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[daemon]\nlog_level = \"loud\"\n").unwrap();
        let loaded = Loaded::load(&path, env(&[]));
        assert_eq!(loaded.config, Config::default());
        let err = loaded.error.unwrap();
        assert_eq!(err.key.as_deref(), Some("daemon.log_level"));
        assert_eq!(err.line, Some(2));
    }
}
