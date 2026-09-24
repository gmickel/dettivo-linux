//! Notes, analysis and the delete policies on a meeting (ADR 0036). The
//! notes are the user's Markdown: the row is what search and export read
//! and `notes.md` in the meeting directory is the copy a person opens,
//! written atomically beside the audio. The analysis is the model's
//! summary, decisions and action items, stored as JSON on the row and as
//! `analysis.json` beside the notes, with its status, error, model and
//! time. A delete clears what the contract's `artifact_policy` names and
//! nothing more: `transcript_only` keeps the row, the notes and the
//! audio; `transcript_and_audio` keeps the row's facts and the notes.

use std::path::Path;

use dettivo_proto::methods::meetings_notes::{Analysis, AnalysisStatus, NotesSource};
use rusqlite::params;

use crate::meetings::MeetingRow;
use crate::time::now_iso;
use crate::{Store, StoreError};

/// The notes file in the meeting directory.
pub const NOTES_FILE: &str = "notes.md";
/// The analysis file in the meeting directory.
pub const ANALYSIS_FILE: &str = "analysis.json";

/// Writes `bytes` to `path` through a sibling temporary file and a
/// rename, so a reader never sees a half-written file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Writes `notes.md` under `dir`; empty notes remove the file.
pub fn write_notes_file(dir: &Path, markdown: &str) -> Result<(), StoreError> {
    let path = dir.join(NOTES_FILE);
    if markdown.trim().is_empty() {
        match std::fs::remove_file(&path) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        }
    }
    let mut body = markdown.to_string();
    if !body.ends_with('\n') {
        body.push('\n');
    }
    write_atomic(&path, body.as_bytes())
}

/// Writes `analysis.json` under `dir` with the model and the time.
pub fn write_analysis_file(
    dir: &Path,
    analysis: &Analysis,
    model: &str,
    analyzed_at: &str,
) -> Result<(), StoreError> {
    let doc = serde_json::json!({
        "summary": analysis.summary,
        "decisions": analysis.decisions,
        "action_items": analysis.action_items,
        "model": model,
        "analyzed_at": analyzed_at,
    });
    let mut text = serde_json::to_string_pretty(&doc).unwrap_or_default();
    text.push('\n');
    write_atomic(&dir.join(ANALYSIS_FILE), text.as_bytes())
}

/// Removes `analysis.json` under `dir`; a missing file is fine.
pub fn remove_analysis_file(dir: &Path) -> Result<(), StoreError> {
    match std::fs::remove_file(dir.join(ANALYSIS_FILE)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

impl Store {
    /// Sets the notes of a meeting and returns the row as stored; the
    /// FTS index follows through the update trigger.
    pub fn set_meeting_notes(
        &self,
        id: &str,
        markdown: &str,
        source: NotesSource,
    ) -> Result<MeetingRow, StoreError> {
        let now = now_iso();
        let n = self.conn().execute(
            "UPDATE meetings SET notes_markdown = ?2, notes_source = ?3, notes_updated_at = ?4, \
             updated_at = ?4 WHERE id = ?1",
            params![id, markdown, source.as_str(), now],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        self.get_meeting(id)?
            .ok_or_else(|| StoreError::NotFound(id.to_string()))
    }

    /// Sets the title of a meeting (`title_source` becomes `manual`) and
    /// returns the row as stored; the FTS index follows through the
    /// update trigger.
    pub fn set_meeting_title(&self, id: &str, title: &str) -> Result<MeetingRow, StoreError> {
        let n = self.conn().execute(
            "UPDATE meetings SET title = ?2, title_source = 'manual', updated_at = ?3 \
             WHERE id = ?1",
            params![id, title, now_iso()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        self.get_meeting(id)?
            .ok_or_else(|| StoreError::NotFound(id.to_string()))
    }

    /// Marks where the analysis stands without touching the analysis
    /// itself (a failed regenerate keeps the old one).
    pub fn set_meeting_analysis_status(
        &self,
        id: &str,
        status: AnalysisStatus,
        error: Option<&str>,
    ) -> Result<(), StoreError> {
        let n = self.conn().execute(
            "UPDATE meetings SET analysis_status = ?2, analysis_error = ?3, updated_at = ?4 \
             WHERE id = ?1",
            params![id, status.as_str(), error, now_iso()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    /// Settles the analysis and diarization attempts a lost daemon left
    /// `queued` or `running`: both become `failed` with `reason`, the
    /// earlier analysis and speaker assignment stay on the row. Returns
    /// how many rows moved.
    pub fn fail_stale_meeting_passes(&self, reason: &str) -> Result<usize, StoreError> {
        let now = now_iso();
        let analyses = self.conn().execute(
            "UPDATE meetings SET analysis_status = 'failed', analysis_error = ?1, \
             updated_at = ?2 WHERE analysis_status IN ('queued', 'running')",
            params![reason, now],
        )?;
        let passes = self.conn().execute(
            "UPDATE meetings SET diarization = json_set(diarization, '$.status', 'failed', \
             '$.error', ?1), updated_at = ?2 \
             WHERE json_extract(diarization, '$.status') IN ('queued', 'running')",
            params![reason, now],
        )?;
        Ok(analyses + passes)
    }

    /// Stores a ready analysis with its model and time; the summary column
    /// and the FTS index follow.
    pub fn set_meeting_analysis(
        &self,
        id: &str,
        analysis: &Analysis,
        model: &str,
        analyzed_at: &str,
    ) -> Result<(), StoreError> {
        let json = serde_json::to_string(analysis).unwrap_or_default();
        let n = self.conn().execute(
            "UPDATE meetings SET analysis = ?2, analysis_text = ?3, analysis_status = 'ready', \
             analysis_error = NULL, analysis_model = ?4, analysis_at = ?5, summary = ?6, \
             updated_at = ?7 WHERE id = ?1",
            params![
                id,
                json,
                analysis.searchable_text(),
                model,
                analyzed_at,
                analysis.summary.trim(),
                now_iso()
            ],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    /// `transcript_only`: clears the texts, the segments (and with them the
    /// speakers and the diarization block), the summary and the analysis,
    /// and with them the FTS entries; the row, the notes and the audio
    /// facts stay.
    pub fn clear_meeting_transcript(&self, id: &str) -> Result<(), StoreError> {
        let tx = self.conn().unchecked_transaction()?;
        let n = tx.execute(
            "UPDATE meetings SET raw_text = '', final_text = '', segments = NULL, summary = '', \
             diarization = NULL, speaker_names = '', \
             analysis = NULL, analysis_text = '', analysis_status = 'none', \
             analysis_error = NULL, analysis_model = NULL, analysis_at = NULL, updated_at = ?2 \
             WHERE id = ?1",
            params![id, now_iso()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        self.replace_speakers(id, &[])?;
        Ok(tx.commit()?)
    }

    /// `transcript_and_audio`: forgets the audio facts once the takes are
    /// gone; the row's other facts and the notes stay.
    pub fn clear_meeting_audio(&self, id: &str) -> Result<(), StoreError> {
        let n = self.conn().execute(
            "UPDATE meetings SET audio_dir = NULL, system_audio = 0, microphone_takes = 0, \
             updated_at = ?2 WHERE id = ?1",
            params![id, now_iso()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings::MeetingStatus;
    use dettivo_proto::methods::meetings::{Segment, SegmentSource};

    fn completed() -> MeetingRow {
        let mut m = MeetingRow::new();
        m.status = MeetingStatus::Completed;
        m.raw_text = "the budget".into();
        m.final_text = "The budget.".into();
        m.segments = vec![Segment {
            index: 0,
            start_ms: 0,
            end_ms: 900,
            text: "The budget.".into(),
            speaker: Some("Mara".into()),
            speaker_id: Some("speaker_00".into()),
            speaker_confidence: Some(1.0),
            source_type: SegmentSource::Microphone,
            words: Vec::new(),
            gap_before_ms: None,
            polished_text: Some("The budget.".into()),
        }];
        m.audio_dir = Some("/tmp/x".into());
        m.microphone_takes = 1;
        m
    }

    #[test]
    fn notes_and_analysis_land_on_the_row_and_the_policies_clear_what_they_name() {
        let store = Store::in_memory().unwrap();
        let row = completed();
        store.insert_meeting(&row).unwrap();
        let back = store
            .set_meeting_notes(&row.id, "# Notes\n\nHire.", NotesSource::Live)
            .unwrap();
        assert_eq!(back.notes_source, NotesSource::Live);
        assert!(back.notes_updated_at.is_some());
        let analysis = Analysis {
            summary: "Budget agreed.".into(),
            decisions: vec!["Hire two".into()],
            action_items: Vec::new(),
        };
        store
            .set_meeting_analysis(&row.id, &analysis, "mock", "2026-02-13T16:31:00Z")
            .unwrap();
        let back = store.get_meeting(&row.id).unwrap().unwrap();
        assert_eq!(back.analysis_status, AnalysisStatus::Ready);
        assert_eq!(back.summary, "Budget agreed.");
        assert_eq!(back.analysis_model.as_deref(), Some("mock"));
        store
            .set_meeting_analysis_status(&row.id, AnalysisStatus::Failed, Some("timeout"))
            .unwrap();
        let back = store.get_meeting(&row.id).unwrap().unwrap();
        assert_eq!(back.analysis, Some(analysis), "a failed run keeps the old");
        assert_eq!(back.analysis_error.as_deref(), Some("timeout"));
        store.clear_meeting_transcript(&row.id).unwrap();
        let back = store.get_meeting(&row.id).unwrap().unwrap();
        assert!(back.final_text.is_empty() && back.segments.is_empty());
        assert!(back.analysis.is_none() && back.summary.is_empty());
        assert_eq!(back.notes_markdown, "# Notes\n\nHire.", "notes stay");
        assert_eq!(back.audio_dir.as_deref(), Some("/tmp/x"), "audio stays");
        store.clear_meeting_audio(&row.id).unwrap();
        let back = store.get_meeting(&row.id).unwrap().unwrap();
        assert!(back.audio_dir.is_none() && back.microphone_takes == 0);
        assert!(store.clear_meeting_transcript("nope").is_err());
    }

    /// A daemon that dies mid-pass leaves `running` on the row; the next
    /// open settles both passes to `failed` and keeps what the earlier
    /// runs produced (meetings/F13).
    #[test]
    fn a_restart_settles_running_analysis_and_diarization_and_keeps_their_results() {
        use dettivo_proto::methods::speakers::{Diarization, DiarizationStatus};
        let store = Store::in_memory().unwrap();
        let mut row = completed();
        row.diarization = Some(Diarization {
            status: DiarizationStatus::Running,
            ..Default::default()
        });
        store.insert_meeting(&row).unwrap();
        let analysis = Analysis {
            summary: "Budget agreed.".into(),
            decisions: Vec::new(),
            action_items: Vec::new(),
        };
        store
            .set_meeting_analysis(&row.id, &analysis, "mock", "2026-02-13T16:31:00Z")
            .unwrap();
        store
            .set_meeting_analysis_status(&row.id, AnalysisStatus::Running, None)
            .unwrap();
        let untouched = completed();
        store.insert_meeting(&untouched).unwrap();
        // A finalisation that planned both passes and never started them.
        let mut queued = completed();
        queued.diarization = Some(Diarization {
            status: DiarizationStatus::Queued,
            ..Default::default()
        });
        queued.analysis_status = AnalysisStatus::Queued;
        store.insert_meeting(&queued).unwrap();

        assert_eq!(
            store
                .fail_stale_meeting_passes("the daemon restarted")
                .unwrap(),
            4
        );
        let planned = store.get_meeting(&queued.id).unwrap().unwrap();
        assert_eq!(planned.analysis_status, AnalysisStatus::Failed);
        assert_eq!(
            planned.analysis_error.as_deref(),
            Some("the daemon restarted")
        );
        let block = planned.diarization.unwrap();
        assert_eq!(block.status, DiarizationStatus::Failed);
        assert_eq!(block.error.as_deref(), Some("the daemon restarted"));

        let back = store.get_meeting(&row.id).unwrap().unwrap();
        assert_eq!(back.analysis_status, AnalysisStatus::Failed);
        assert_eq!(back.analysis_error.as_deref(), Some("the daemon restarted"));
        assert_eq!(back.analysis, Some(analysis), "the earlier analysis stays");
        let block = back.diarization.unwrap();
        assert_eq!(block.status, DiarizationStatus::Failed);
        assert_eq!(block.error.as_deref(), Some("the daemon restarted"));
        assert_eq!(
            back.segments[0].speaker.as_deref(),
            Some("Mara"),
            "the speaker assignment stays"
        );
        let other = store.get_meeting(&untouched.id).unwrap().unwrap();
        assert_eq!(other.analysis_status, AnalysisStatus::None);
        assert!(other.diarization.is_none());
        assert_eq!(store.fail_stale_meeting_passes("again").unwrap(), 0);
    }

    /// A rename lands on the row as `manual` and the title index follows;
    /// an unknown meeting is `NotFound`.
    #[test]
    fn a_rename_stores_the_title_as_manual_and_the_index_follows() {
        let store = Store::in_memory().unwrap();
        let mut row = completed();
        row.title = "Weekly sync".into();
        row.title_source = "auto".into();
        store.insert_meeting(&row).unwrap();
        let back = store.set_meeting_title(&row.id, "Budget call").unwrap();
        assert_eq!(back.title, "Budget call");
        assert_eq!(back.title_source, "manual");
        let hits = store.search_meetings("budget", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, row.id);
        assert_eq!(hits[0].matched_field, "title");
        assert!(store.search_meetings("weekly", 10).unwrap().is_empty());
        assert!(matches!(
            store.set_meeting_title("nope", "x"),
            Err(StoreError::NotFound(_))
        ));
    }

    #[test]
    fn the_files_are_written_atomically_and_removed_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        let meeting = dir.path().join("m");
        write_notes_file(&meeting, "hello").unwrap();
        assert_eq!(
            std::fs::read_to_string(meeting.join(NOTES_FILE)).unwrap(),
            "hello\n"
        );
        assert!(!meeting.join("notes.md.tmp").exists());
        write_notes_file(&meeting, "  ").unwrap();
        assert!(!meeting.join(NOTES_FILE).exists());
        let analysis = Analysis {
            summary: "S".into(),
            decisions: Vec::new(),
            action_items: Vec::new(),
        };
        write_analysis_file(&meeting, &analysis, "mock", "2026-02-13T16:31:00Z").unwrap();
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(meeting.join(ANALYSIS_FILE)).unwrap())
                .unwrap();
        assert_eq!(doc["model"], "mock");
        remove_analysis_file(&meeting).unwrap();
        remove_analysis_file(&meeting).unwrap();
    }
}
