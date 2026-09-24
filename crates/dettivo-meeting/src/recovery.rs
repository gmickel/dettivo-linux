//! Recovery at daemon start (FR-G7): a meeting row a killed daemon left
//! `recording` or `stopping` is promoted to `partial` with its takes
//! repaired and counted, the chunk counts read from the live checkpoint,
//! the live segment tail it retained shown on the row, and the reason
//! recorded when the checkpoint could not be read. Nothing is removed;
//! `meetings.recover` keeps the meeting and finalises it,
//! `meetings.discard` removes it.

use std::path::Path;

use dettivo_audio::takes::{Take, Takes, read_sidecar, repair_wav};
use dettivo_storage::meetings::{MeetingRow, MeetingStatus};
use dettivo_storage::time::now_iso;

use crate::Track;
use crate::checkpoint::Checkpoint;
use crate::journal::{self, Entry};

/// What the promotion found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Promoted {
    /// Milliseconds of audio preserved across the tracks.
    pub duration_ms: u64,
    /// Microphone takes on disk.
    pub microphone_takes: u32,
    /// True when a system take exists.
    pub system_audio: bool,
    /// The checkpoint's chunk counts, `(0, 0)` without one.
    pub chunks: (u32, u32),
    /// Checkpoint, missing input or unknown timing problems encountered.
    pub reason: Option<String>,
}

/// Reconciles known take identities with the checkpoint and journal,
/// repairs their sample counts and preserves unknown files for inspection.
fn repair_track(
    dir: &Path,
    track: Track,
    checkpoint: Option<&Checkpoint>,
    issues: &mut Vec<String>,
) -> Takes {
    let sidecar = if track == Track::Microphone {
        dir.join(dettivo_audio::takes::SIDECAR)
    } else {
        dir.join(format!("{}-takes.json", track.prefix()))
    };
    let mut takes = read_sidecar(&sidecar).unwrap_or_default();
    if let Some(checkpoint) = checkpoint {
        for take in checkpoint.takes.iter().filter(|t| t.track == track) {
            if !takes.takes.iter().any(|t| t.file == take.file) {
                takes.takes.push(Take {
                    index: take_index(&take.file, track).unwrap_or(1),
                    file: take.file.clone(),
                    start_offset_ms: take.start_offset_ms,
                    start_offset_ns: take.start_offset_ns,
                    gap_before: take.gap_before,
                    samples: take.samples,
                });
            }
        }
    }
    for entry in journal::read(&dir.join(crate::JOURNAL_FILE)) {
        if entry.track.as_deref() != Some(track.prefix()) || entry.event != "gap" {
            continue;
        }
        if let Some(file) = entry.take
            && let Some(index) = take_index(&file, track)
            && !takes.takes.iter().any(|t| t.file == file)
        {
            takes.takes.push(Take {
                index,
                file,
                start_offset_ms: entry.offset_ms,
                start_offset_ns: entry.offset_ms.saturating_mul(1_000_000),
                gap_before: true,
                samples: 0,
            });
        }
    }
    let files: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.file_name().to_str().map(str::to_string))
                .filter(|n| take_index(n, track).is_some())
                .collect()
        })
        .unwrap_or_default();
    let mut timing_unknown = false;
    for file in files {
        if !takes.takes.iter().any(|t| t.file == file) {
            issues.push(format!("unknown take timing: {file}; audio retained"));
            timing_unknown = true;
        }
    }
    takes
        .takes
        .sort_by_key(|t| (t.start_offset_ms, t.start_offset_ns, t.index));
    for take in &mut takes.takes {
        if take_index(&take.file, track).is_none()
            || !dir
                .join(&take.file)
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_file())
        {
            issues.push(format!("missing or invalid take: {}", take.file));
            continue;
        }
        match repair_wav(&dir.join(&take.file)) {
            Ok(samples) => take.samples = samples,
            Err(e) => issues.push(format!("{}: {e}", take.file)),
        }
    }
    if !timing_unknown
        && !takes.takes.is_empty()
        && let Err(e) = std::fs::write(
            &sidecar,
            serde_json::to_string_pretty(&takes).unwrap_or_default(),
        )
    {
        issues.push(format!("{}: {e}", sidecar.display()));
    }
    takes
}

fn take_index(file: &str, track: Track) -> Option<u32> {
    if file == format!("{}.wav", track.prefix()) {
        return Some(1);
    }
    file.strip_prefix(&format!("{}-", track.prefix()))?
        .strip_suffix(".wav")?
        .parse::<u32>()
        .ok()
        .filter(|index| *index >= 2)
}

/// Promotes `row` to `partial` from what its directory holds and appends
/// `recovered` to the journal. The row is the caller's to store.
pub fn promote(dir: &Path, row: &mut MeetingRow) -> Promoted {
    let checkpoint = Checkpoint::read(dir);
    let mut issues = Vec::new();
    let mic = repair_track(
        dir,
        Track::Microphone,
        checkpoint.as_ref().ok(),
        &mut issues,
    );
    let sys = repair_track(dir, Track::System, checkpoint.as_ref().ok(), &mut issues);
    let duration_ms = mic
        .takes
        .iter()
        .chain(&sys.takes)
        .map(|take| {
            take.start_offset_ms
                .saturating_add(take.samples * 1000 / u64::from(dettivo_audio::SAMPLE_RATE))
        })
        .max()
        .unwrap_or(0);
    let chunks = match checkpoint {
        Ok(c) => {
            // The live tail the checkpoint retained shows until the
            // finalisation replaces it.
            if !c.segments.is_empty() {
                row.final_text = c
                    .segments
                    .iter()
                    .map(|s| s.text.trim())
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                row.raw_text = row.final_text.clone();
                row.segments = c.segments;
            }
            (c.chunks_completed, c.chunks_total)
        }
        Err(e) => {
            issues.push(e);
            (0, 0)
        }
    };
    let reason = (!issues.is_empty()).then(|| issues.join("; "));
    row.status = MeetingStatus::Partial;
    row.is_partial = true;
    row.chunks_completed = chunks.0;
    row.chunks_total = chunks.1;
    row.duration_ms = duration_ms;
    row.microphone_takes = mic.takes.len() as u32;
    row.system_audio = !sys.takes.is_empty();
    row.recovery_reason = Some(
        reason
            .clone()
            .unwrap_or_else(|| "the daemon restarted while the meeting recorded".into()),
    );
    if row.ended_at.is_none() {
        row.ended_at = Some(now_iso());
    }
    let _ = journal::append(
        &dir.join(crate::JOURNAL_FILE),
        &Entry {
            at: now_iso(),
            offset_ms: duration_ms,
            event: "recovered".into(),
            track: None,
            take: None,
            detail: Some(format!(
                "partial; duration_ms={duration_ms} takes={} chunks={}/{}{}",
                mic.takes.len(),
                chunks.0,
                chunks.1,
                reason
                    .as_deref()
                    .map(|r| format!("; checkpoint: {r}"))
                    .unwrap_or_default()
            )),
        },
    );
    Promoted {
        duration_ms,
        microphone_takes: mic.takes.len() as u32,
        system_audio: !sys.takes.is_empty(),
        chunks,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dettivo_audio::takes::TakeWriter;

    #[test]
    fn a_killed_meeting_is_promoted_with_its_takes_repaired() {
        let dir = tempfile::tempdir().unwrap();
        let clock = std::time::Instant::now();
        let mut mic = TakeWriter::with_prefix(dir.path(), "microphone", clock).unwrap();
        let mut sys = TakeWriter::with_prefix(dir.path(), "system", clock).unwrap();
        mic.write(&[3; 16_000]).unwrap();
        sys.write(&[4; 8_000]).unwrap();
        mic.flush().unwrap();
        sys.flush().unwrap();
        // The kill: samples written after the last header update sit past
        // the length the header states.
        drop(mic);
        drop(sys);
        let mut bytes = std::fs::read(dir.path().join("microphone.wav")).unwrap();
        bytes.extend(std::iter::repeat_n(3u8, 8_000));
        std::fs::write(dir.path().join("microphone.wav"), bytes).unwrap();
        let mut c = Checkpoint::new("m", "2026-02-13T16:00:00Z");
        c.chunks_completed = 2;
        c.chunks_total = 5;
        c.write(dir.path()).unwrap();

        let mut row = MeetingRow::new();
        let promoted = promote(dir.path(), &mut row);
        assert_eq!(row.status, MeetingStatus::Partial);
        assert!(row.is_partial);
        assert_eq!((row.chunks_completed, row.chunks_total), (2, 5));
        assert_eq!(promoted.microphone_takes, 1);
        assert!(promoted.system_audio);
        assert_eq!(
            promoted.duration_ms, 1250,
            "the samples past the header count"
        );
        assert!(promoted.reason.is_none());
        let entries = journal::read(&dir.path().join(crate::JOURNAL_FILE));
        assert_eq!(entries.last().unwrap().event, "recovered");

        // An unreadable checkpoint still preserves the audio and names why.
        std::fs::write(Checkpoint::path(dir.path()), "{").unwrap();
        let mut row = MeetingRow::new();
        let again = promote(dir.path(), &mut row);
        assert_eq!(again.duration_ms, 1250);
        assert!(
            again
                .reason
                .as_deref()
                .unwrap()
                .contains("not a checkpoint")
        );
        assert!(
            row.recovery_reason
                .as_deref()
                .unwrap()
                .contains("not a checkpoint")
        );
    }
}
