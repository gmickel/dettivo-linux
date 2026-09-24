//! Artifacts on disk and what keeps or removes them: a retained take moves
//! from the session directory into `<data>/dictations/<id>/microphone.wav`
//! with `metadata.json` beside it, the artifact policy decides what a
//! delete removes, and the sweep drops audio past its retention and rows
//! past `max_items`, oldest first, skipping items a job is still using.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::item::DictationItem;
use crate::time::{iso_from_unix, now_unix};
use crate::{Store, StoreError};

/// The file name of a retained take.
pub const AUDIO_FILE: &str = "microphone.wav";
/// The sidecar beside it.
pub const METADATA_FILE: &str = "metadata.json";

/// What delete removes and what the store writes beside the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArtifactPolicy {
    /// Audio and `metadata.json` are written; delete removes the whole
    /// item directory.
    #[default]
    Keep,
    /// Audio only, no sidecar; delete removes the audio.
    AudioOnly,
    /// Nothing is written beside the database; delete removes the row.
    None,
}

impl ArtifactPolicy {
    /// The `config.toml` spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::AudioOnly => "audio_only",
            Self::None => "none",
        }
    }

    /// Parses the `config.toml` spelling.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "keep" => Some(Self::Keep),
            "audio_only" => Some(Self::AudioOnly),
            "none" => Some(Self::None),
            _ => Option::None,
        }
    }
}

/// The `[history]` keys the sweep and the writes honour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retention {
    /// Retain the take of a finished dictation.
    pub keep_audio: bool,
    /// Days after which retained audio is removed; 0 keeps it.
    pub audio_retention_days: u64,
    /// Rows kept; 0 keeps every row.
    pub max_items: u64,
    /// The artifact policy.
    pub artifacts: ArtifactPolicy,
}

impl Default for Retention {
    fn default() -> Self {
        Self {
            keep_audio: true,
            audio_retention_days: 30,
            max_items: 0,
            artifacts: ArtifactPolicy::Keep,
        }
    }
}

/// The item directories under `<data>/dictations/`.
#[derive(Debug, Clone)]
pub struct Artifacts {
    root: PathBuf,
}

/// What a sweep did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SweepReport {
    /// Audio files removed past retention.
    pub audio_removed: usize,
    /// Rows removed past `max_items`.
    pub rows_removed: usize,
}

impl Artifacts {
    /// Item directories under `root`.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// The root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// One item's directory.
    pub fn dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    /// Moves a session's takes into the item directory as one
    /// `microphone.wav` (several takes are concatenated) and returns its
    /// path; `None` when the policy keeps no audio or there is no take.
    pub fn retain_take(
        &self,
        id: &str,
        take_dir: &Path,
        retention: &Retention,
    ) -> Result<Option<PathBuf>, StoreError> {
        if !retention.keep_audio || retention.artifacts == ArtifactPolicy::None {
            return Ok(None);
        }
        let takes = take_files(take_dir);
        if takes.is_empty() {
            return Ok(None);
        }
        let dir = self.dir(id);
        std::fs::create_dir_all(&dir)?;
        let target = dir.join(AUDIO_FILE);
        if takes.len() == 1 {
            move_file(&takes[0], &target)?;
        } else {
            concatenate(&takes, &target)?;
            for t in &takes {
                let _ = std::fs::remove_file(t);
            }
        }
        Ok(Some(target))
    }

    /// Copies an audio file into the item directory (imports, re-runs).
    pub fn adopt_audio(&self, id: &str, source: &Path) -> Result<PathBuf, StoreError> {
        let dir = self.dir(id);
        std::fs::create_dir_all(&dir)?;
        let target = dir.join(AUDIO_FILE);
        std::fs::copy(source, &target)?;
        Ok(target)
    }

    /// Writes `metadata.json` for an item when the policy keeps sidecars.
    pub fn write_metadata(
        &self,
        item: &DictationItem,
        retention: &Retention,
    ) -> Result<(), StoreError> {
        if retention.artifacts != ArtifactPolicy::Keep {
            return Ok(());
        }
        let dir = self.dir(&item.id);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join(METADATA_FILE),
            serde_json::to_string_pretty(item).unwrap_or_default(),
        )?;
        Ok(())
    }

    /// Removes an item's audio file (and the directory when it is empty).
    pub fn remove_audio(&self, item: &DictationItem) -> Result<(), StoreError> {
        if let Some(path) = &item.audio_path {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        let _ = std::fs::remove_dir(self.dir(&item.id));
        Ok(())
    }

    /// Removes what the policy says a delete removes.
    pub fn remove(&self, item: &DictationItem, policy: ArtifactPolicy) -> Result<(), StoreError> {
        match policy {
            ArtifactPolicy::Keep => match std::fs::remove_dir_all(self.dir(&item.id)) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.into()),
            },
            ArtifactPolicy::AudioOnly => self.remove_audio(item),
            ArtifactPolicy::None => Ok(()),
        }
    }
}

/// The take files of a session directory, in order (`takes.json` when
/// present, else `microphone*.wav` sorted).
fn take_files(dir: &Path) -> Vec<PathBuf> {
    if let Ok(text) = std::fs::read_to_string(dir.join("takes.json")) {
        if let Ok(doc) = serde_json::from_str::<serde_json::Value>(&text) {
            let files: Vec<PathBuf> = doc["takes"]
                .as_array()
                .map(|takes| {
                    takes
                        .iter()
                        .filter_map(|t| t["file"].as_str())
                        .filter(|f| !f.contains('/') && f.ends_with(".wav"))
                        .map(|f| dir.join(f))
                        .filter(|p| p.is_file())
                        .collect()
                })
                .unwrap_or_default();
            if !files.is_empty() {
                return files;
            }
        }
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().is_some_and(|x| x == "wav")
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.starts_with("microphone"))
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// Renames, or copies and removes when the directories sit on different
/// file systems (state and data may).
fn move_file(from: &Path, to: &Path) -> Result<(), StoreError> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to)?;
    std::fs::remove_file(from)?;
    Ok(())
}

/// Concatenates 16 kHz mono takes into one file.
fn concatenate(takes: &[PathBuf], target: &Path) -> Result<(), StoreError> {
    let map = |e: hound::Error| StoreError::Io(e.to_string());
    let first = hound::WavReader::open(&takes[0]).map_err(map)?;
    let spec = first.spec();
    drop(first);
    let mut writer = hound::WavWriter::create(target, spec).map_err(map)?;
    for take in takes {
        let mut reader = hound::WavReader::open(take).map_err(map)?;
        for sample in reader.samples::<i16>() {
            writer.write_sample(sample.map_err(map)?).map_err(map)?;
        }
    }
    writer.finalize().map_err(map)?;
    Ok(())
}

/// Deletes an item and its artifacts per the policy.
pub fn delete_item(
    store: &Store,
    artifacts: &Artifacts,
    policy: ArtifactPolicy,
    id: &str,
) -> Result<DictationItem, StoreError> {
    let item = store.delete_row(id)?;
    artifacts.remove(&item, policy)?;
    Ok(item)
}

/// Removes audio past retention and rows past `max_items`, oldest first,
/// leaving items in `busy` alone. `now` is seconds since the epoch.
pub fn sweep(
    store: &Store,
    artifacts: &Artifacts,
    retention: &Retention,
    now: Option<i64>,
    busy: &HashSet<String>,
) -> Result<SweepReport, StoreError> {
    let now = now.unwrap_or_else(now_unix);
    let mut report = SweepReport::default();
    if retention.audio_retention_days > 0 && retention.artifacts != ArtifactPolicy::None {
        let cutoff = iso_from_unix(now - (retention.audio_retention_days as i64) * 86_400);
        for item in store.with_audio_before(&cutoff)? {
            if busy.contains(&item.id) {
                continue;
            }
            artifacts.remove_audio(&item)?;
            store.clear_audio(&item.id)?;
            report.audio_removed += 1;
        }
    }
    if retention.max_items > 0 {
        for item in store.beyond(retention.max_items)? {
            if busy.contains(&item.id) {
                continue;
            }
            store.delete_row(&item.id)?;
            artifacts.remove(&item, retention.artifacts)?;
            report.rows_removed += 1;
        }
    }
    if report.audio_removed > 0 || report.rows_removed > 0 {
        tracing::info!(
            audio_removed = report.audio_removed,
            rows_removed = report.rows_removed,
            "history retention sweep"
        );
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policies_round_trip_and_defaults_match_the_config() {
        for p in [
            ArtifactPolicy::Keep,
            ArtifactPolicy::AudioOnly,
            ArtifactPolicy::None,
        ] {
            assert_eq!(ArtifactPolicy::parse(p.as_str()), Some(p));
        }
        assert_eq!(ArtifactPolicy::parse("nope"), Option::None);
        let r = Retention::default();
        assert!(r.keep_audio);
        assert_eq!(r.audio_retention_days, 30);
        assert_eq!(r.max_items, 0);
    }
}
