//! The live checkpoint: `live-checkpoint.json` under the meeting
//! directory, schema version 1 (the macOS `LiveMeetingCheckpoint` fields
//! plus the takes), written atomically on an interval while recording so
//! the next daemon start knows what a killed meeting had: its takes with
//! their offsets, the retained segment tail (empty until transcription
//! exists) and the chunk counts.

use std::path::{Path, PathBuf};

use dettivo_audio::takes::Take;
use dettivo_proto::methods::meetings::Segment;
use serde::{Deserialize, Serialize};

use crate::Track;

/// The schema this crate writes and reads.
pub const SCHEMA_VERSION: u32 = 1;
/// The file name under the meeting directory.
pub const FILE: &str = "live-checkpoint.json";

/// One take as the checkpoint records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointTake {
    /// Which track.
    pub track: Track,
    /// File name relative to the meeting directory.
    pub file: String,
    /// Milliseconds since the meeting started when this take began.
    pub start_offset_ms: u64,
    /// The same instant in nanoseconds.
    pub start_offset_ns: u64,
    /// True when capture was interrupted before this take.
    pub gap_before: bool,
    /// Samples flushed to disk at the checkpoint.
    pub samples: u64,
}

impl CheckpointTake {
    /// A sidecar take on `track`.
    pub fn from_take(track: Track, take: &Take) -> Self {
        Self {
            track,
            file: take.file.clone(),
            start_offset_ms: take.start_offset_ms,
            start_offset_ns: take.start_offset_ns,
            gap_before: take.gap_before,
            samples: take.samples,
        }
    }
}

/// The checkpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Always [`SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The meeting id.
    pub meeting_id: String,
    /// ISO 8601 UTC start.
    pub started_at: String,
    /// ISO 8601 UTC time of this write.
    pub updated_at: String,
    /// Every take of both tracks at the time of the write.
    pub takes: Vec<CheckpointTake>,
    /// The retained live segment tail (empty until transcription exists).
    #[serde(default)]
    pub segments: Vec<Segment>,
    /// The next live segment sequence number.
    #[serde(default)]
    pub next_sequence: u64,
    /// Chunks transcribed so far (0 before transcription exists).
    #[serde(default)]
    pub chunks_completed: u32,
    /// Chunks planned in total (0 before transcription exists).
    #[serde(default)]
    pub chunks_total: u32,
    /// True while the meeting finalises.
    #[serde(default)]
    pub is_finalizing: bool,
    /// The last error the capture recorded, when any.
    #[serde(default)]
    pub last_error: Option<String>,
}

impl Checkpoint {
    /// A fresh checkpoint for `meeting_id`.
    pub fn new(meeting_id: &str, started_at: &str) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            meeting_id: meeting_id.to_string(),
            started_at: started_at.to_string(),
            updated_at: started_at.to_string(),
            takes: Vec::new(),
            segments: Vec::new(),
            next_sequence: 0,
            chunks_completed: 0,
            chunks_total: 0,
            is_finalizing: false,
            last_error: None,
        }
    }

    /// The file under `dir`.
    pub fn path(dir: &Path) -> PathBuf {
        dir.join(FILE)
    }

    /// Writes the checkpoint atomically (a temporary file renamed into
    /// place), so a reader never sees half of one.
    pub fn write(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(".live-checkpoint.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_string_pretty(self).map_err(std::io::Error::other)?,
        )?;
        std::fs::rename(tmp, Self::path(dir))
    }

    /// Reads the checkpoint under `dir`; the error names what is wrong
    /// (missing, unreadable, another schema).
    pub fn read(dir: &Path) -> Result<Self, String> {
        let path = Self::path(dir);
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let checkpoint: Self = serde_json::from_str(&text)
            .map_err(|e| format!("{}: not a checkpoint: {e}", path.display()))?;
        if checkpoint.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "{}: schema version {} is not {SCHEMA_VERSION}",
                path.display(),
                checkpoint.schema_version
            ));
        }
        Ok(checkpoint)
    }

    /// Removes the checkpoint under `dir` (a clean stop or cancel).
    pub fn remove(dir: &Path) {
        let _ = std::fs::remove_file(Self::path(dir));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoints_write_atomically_and_refuse_other_schemas() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = Checkpoint::new("m1", "2026-02-13T16:00:00Z");
        c.takes.push(CheckpointTake::from_take(
            Track::Microphone,
            &Take {
                index: 1,
                file: "microphone.wav".into(),
                start_offset_ms: 12,
                start_offset_ns: 12_000_100,
                gap_before: false,
                samples: 16_000,
            },
        ));
        c.write(dir.path()).unwrap();
        assert!(!dir.path().join(".live-checkpoint.tmp").exists());
        let back = Checkpoint::read(dir.path()).unwrap();
        assert_eq!(back, c);
        assert_eq!(back.takes[0].track, Track::Microphone);
        let mut other = c.clone();
        other.schema_version = 2;
        other.write(dir.path()).unwrap();
        assert!(
            Checkpoint::read(dir.path())
                .unwrap_err()
                .contains("schema version 2")
        );
        std::fs::write(Checkpoint::path(dir.path()), "{").unwrap();
        assert!(
            Checkpoint::read(dir.path())
                .unwrap_err()
                .contains("not a checkpoint")
        );
        Checkpoint::remove(dir.path());
        assert!(Checkpoint::read(dir.path()).is_err());
    }
}
