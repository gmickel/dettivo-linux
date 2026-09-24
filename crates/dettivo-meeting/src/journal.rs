//! The session journal: `journal.jsonl` under the meeting directory, one
//! line per event with its time on the meeting clock (`start`,
//! `system_unavailable`, `device_switch`, `gap`, `checkpoint`, `error`,
//! `stop`, `cancel`, `recovered`), so a recovery and a person can both
//! read what happened to the takes.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One journal line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// ISO 8601 UTC time.
    pub at: String,
    /// Milliseconds since the meeting started.
    pub offset_ms: u64,
    /// The event name.
    pub event: String,
    /// The track the event concerns, when one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<String>,
    /// The take file the event concerns, when one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub take: Option<String>,
    /// Free text: the device, the reason, the count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// An append-only journal; clones share the file and the clock.
#[derive(Clone)]
pub struct Journal {
    path: PathBuf,
    started: std::time::Instant,
}

impl Journal {
    /// Opens (creating) the journal under `dir`; offsets count from `started`.
    pub fn open(dir: &Path, started: std::time::Instant) -> Self {
        Self {
            path: dir.join(crate::JOURNAL_FILE),
            started,
        }
    }

    /// The file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Appends one entry; a journal that cannot be written is logged and
    /// never stops the capture.
    pub fn record(
        &self,
        event: &str,
        track: Option<crate::Track>,
        take: Option<&str>,
        detail: Option<String>,
    ) {
        let entry = Entry {
            at: dettivo_storage::time::now_iso(),
            offset_ms: self.started.elapsed().as_millis() as u64,
            event: event.to_string(),
            track: track.map(|t| t.prefix().to_string()),
            take: take.map(str::to_string),
            detail,
        };
        if let Err(e) = append(&self.path, &entry) {
            tracing::warn!(error = %e, "meeting journal not written");
        }
    }
}

/// Appends one entry to the journal at `path`.
pub fn append(path: &Path, entry: &Entry) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let mut line = serde_json::to_string(entry).map_err(std::io::Error::other)?;
    line.push('\n');
    file.write_all(line.as_bytes())
}

/// Reads every entry; a line that does not parse is skipped.
pub fn read(path: &Path) -> Vec<Entry> {
    std::fs::read_to_string(path)
        .map(|text| {
            text.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_append_as_lines_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::open(dir.path(), std::time::Instant::now());
        journal.record("start", None, None, None);
        journal.record(
            "gap",
            Some(crate::Track::Microphone),
            Some("microphone-2.wav"),
            Some("device lost".into()),
        );
        std::fs::write(journal.path(), {
            let mut t = std::fs::read_to_string(journal.path()).unwrap();
            t.push_str("not json\n");
            t
        })
        .unwrap();
        let entries = read(journal.path());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].event, "start");
        assert_eq!(entries[1].track.as_deref(), Some("microphone"));
        assert_eq!(entries[1].take.as_deref(), Some("microphone-2.wav"));
        assert!(entries[0].at.ends_with('Z'));
    }
}
