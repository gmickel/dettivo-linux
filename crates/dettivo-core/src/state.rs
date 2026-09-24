//! Runtime state that is not configuration (ADR 0009, FR-C7): window
//! geometry, acknowledgements, first-run progress, the daemon's last start.
//! It lives in `$XDG_STATE_HOME/dettivo/state.toml` and is never written
//! into `config.toml`. Every field defaults, so an older or hand-edited
//! file loads, and the tables the app owns (`[window]`, `[app]`,
//! `[first_run]`, ADR 0024) ride through a daemon save untouched.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::atomic;

/// The whole state file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Facts about the daemon process.
    pub daemon: DaemonState,
    /// One-time acknowledgements the user has given.
    pub acknowledgements: Acknowledgements,
    /// Every other table (the app's window geometry, last route and
    /// first-run progress), kept as it was read.
    #[serde(flatten)]
    pub other: toml::Table,
}

impl State {
    /// The first-run progress the app keeps: `completed_at` when first
    /// run finished, `step` for a flow closed mid-way.
    pub fn first_run(&self) -> (Option<String>, Option<String>) {
        let table = self.other.get("first_run").and_then(|v| v.as_table());
        let text = |key: &str| {
            table
                .and_then(|t| t.get(key))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        (text("completed_at"), text("step"))
    }
}

/// Facts about the daemon process.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DaemonState {
    /// Unix time of the most recent daemon start.
    pub last_started_unix: u64,
    /// How many times the daemon has started on this machine.
    pub start_count: u64,
}

/// One-time acknowledgements.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Acknowledgements {
    /// The meeting recording disclosure has been shown and accepted.
    pub meeting_disclosure: bool,
    /// ISO 8601 UTC time of that acknowledgement, when it was given.
    pub meeting_disclosure_at: Option<String>,
}

impl State {
    /// Reads the file; a missing file is the default state.
    pub fn load(path: &Path) -> Result<Self, StateError> {
        match fs::read_to_string(path) {
            Ok(text) => {
                toml::from_str(&text).map_err(|e| StateError::Parse(e.message().to_string()))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(StateError::Io(e)),
        }
    }

    /// Writes the file atomically, creating its directory `0700`.
    pub fn save(&self, path: &Path) -> Result<(), StateError> {
        let text = toml::to_string(self).map_err(|e| StateError::Parse(e.to_string()))?;
        atomic::write(path, text.as_bytes(), 0o600, 0o700).map_err(StateError::Io)
    }
}

/// Why the state file could not be read or written.
#[derive(Debug)]
pub enum StateError {
    /// The file exists but is not the state schema.
    Parse(String),
    /// The file system refused.
    Io(io::Error),
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(m) => write!(f, "state file does not parse: {m}"),
            Self::Io(e) => write!(f, "state file: {e}"),
        }
    }
}

impl std::error::Error for StateError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_default_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state").join("state.toml");
        assert_eq!(State::load(&path).unwrap(), State::default());
        let mut state = State::default();
        state.daemon.last_started_unix = 42;
        state.acknowledgements.meeting_disclosure = true;
        state.acknowledgements.meeting_disclosure_at = Some("2026-02-13T16:00:00Z".into());
        state.save(&path).unwrap();
        assert_eq!(State::load(&path).unwrap(), state);
    }

    #[test]
    fn partial_files_load_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.toml");
        fs::write(&path, "[daemon]\nstart_count = 3\n").unwrap();
        let state = State::load(&path).unwrap();
        assert_eq!(state.daemon.start_count, 3);
        assert!(!state.acknowledgements.meeting_disclosure);
        assert_eq!(state.first_run(), (None, None));
    }

    #[test]
    fn the_apps_tables_survive_a_daemon_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.toml");
        fs::write(
            &path,
            "[window]\nwidth = 1280\n\n[app]\nlast_route = \"history\"\n\n[first_run]\ncompleted_at = \"2026-09-04T10:00:00Z\"\nstep = \"\"\n",
        )
        .unwrap();
        let mut state = State::load(&path).unwrap();
        assert_eq!(
            state.first_run(),
            (Some("2026-09-04T10:00:00Z".to_string()), None)
        );
        state.daemon.start_count = 1;
        state.save(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("width = 1280"), "{text}");
        assert!(text.contains("last_route = \"history\""), "{text}");
        assert!(
            text.contains("completed_at = \"2026-09-04T10:00:00Z\""),
            "{text}"
        );
        assert!(text.contains("start_count = 1"), "{text}");
        assert_eq!(State::load(&path).unwrap(), state);
    }
}
