//! Small meeting projections for lists, polling and recovery selection.

use crate::meetings::MeetingStatus;
use crate::{Store, StoreError};
use dettivo_proto::methods::meetings_notes::AnalysisStatus;
use rusqlite::{OptionalExtension, Row};

/// Display fields only; transcript, notes and analysis payloads stay in SQLite.
#[derive(Debug, Clone, PartialEq)]
pub struct MeetingSummary {
    /// Stable meeting identifier.
    pub id: String,
    /// Creation timestamp for cursor ordering.
    pub created_at: String,
    /// Display title.
    pub title: String,
    /// Capture start timestamp.
    pub started_at: String,
    /// Captured duration in milliseconds.
    pub duration_ms: u64,
    /// Stored lifecycle state.
    pub status: MeetingStatus,
    /// Analysis summary displayed in meeting lists.
    pub summary: String,
    /// Whether notes contain non-whitespace text.
    pub has_notes: bool,
    /// Analysis lifecycle state.
    pub analysis_status: AnalysisStatus,
    /// Whether capture was interrupted.
    pub is_partial: bool,
    /// Number of assigned speakers.
    pub speaker_count: u32,
    /// Capture or import origin.
    pub source_kind: String,
}

impl MeetingSummary {
    /// The contract's rounded duration.
    pub fn duration_seconds(&self) -> u64 {
        self.duration_ms.div_ceil(1000)
    }
}

// Rust str::trim uses Unicode White_Space; SQLite's default trim only removes spaces.
pub(crate) const SUMMARY_COLUMNS: &str = "id, created_at, title, started_at, duration_ms, status, summary, \
trim(notes_markdown, '\u{0009}\u{000a}\u{000b}\u{000c}\u{000d} \u{0085}\u{00a0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}') != '', \
analysis_status, is_partial, (SELECT COUNT(*) FROM meeting_speakers WHERE meeting_id = meetings.id), source_kind, seq";

pub(crate) fn summary_from_row(row: &Row<'_>) -> rusqlite::Result<(MeetingSummary, i64)> {
    Ok((
        MeetingSummary {
            id: row.get(0)?,
            created_at: row.get(1)?,
            title: row.get(2)?,
            started_at: row.get(3)?,
            duration_ms: row.get::<_, i64>(4)?.max(0) as u64,
            status: MeetingStatus::parse(&row.get::<_, String>(5)?)
                .unwrap_or(MeetingStatus::Failed),
            summary: row.get(6)?,
            has_notes: row.get(7)?,
            analysis_status: AnalysisStatus::parse(&row.get::<_, String>(8)?),
            is_partial: row.get(9)?,
            speaker_count: row.get(10)?,
            source_kind: row.get(11)?,
        },
        row.get(12)?,
    ))
}

/// Capture facts needed by polling and recoverable lists, without transcript payloads.
#[derive(Debug, Clone, PartialEq)]
pub struct MeetingCapture {
    /// Stable identifier.
    pub id: String,
    /// Display title.
    pub title: String,
    /// Start timestamp.
    pub started_at: String,
    /// Capture duration.
    pub duration_ms: u64,
    /// Lifecycle state.
    pub status: MeetingStatus,
    /// Capture or import origin.
    pub source_kind: String,
    /// Whether system audio was captured.
    pub system_audio: bool,
    /// Whether capture was interrupted.
    pub is_partial: bool,
    /// Transcribed chunk count.
    pub chunks_completed: u32,
    /// Total chunk count.
    pub chunks_total: u32,
    /// Microphone take count.
    pub microphone_takes: u32,
    /// Reason for recovery.
    pub recovery_reason: Option<String>,
    /// Last failure message.
    pub error_message: Option<String>,
}

const CAPTURE_COLUMNS: &str = "id, title, started_at, duration_ms, status, source_kind, system_audio, is_partial, chunks_completed, chunks_total, microphone_takes, recovery_reason, error_message";

fn capture_from_row(row: &Row<'_>) -> rusqlite::Result<MeetingCapture> {
    Ok(MeetingCapture {
        id: row.get(0)?,
        title: row.get(1)?,
        started_at: row.get(2)?,
        duration_ms: row.get::<_, i64>(3)?.max(0) as u64,
        status: MeetingStatus::parse(&row.get::<_, String>(4)?).unwrap_or(MeetingStatus::Failed),
        source_kind: row.get(5)?,
        system_audio: row.get(6)?,
        is_partial: row.get(7)?,
        chunks_completed: row.get::<_, i64>(8)?.max(0) as u32,
        chunks_total: row.get::<_, i64>(9)?.max(0) as u32,
        microphone_takes: row.get::<_, i64>(10)?.max(0) as u32,
        recovery_reason: row.get(11)?,
        error_message: row.get(12)?,
    })
}

/// Status projection with segment aggregates computed inside SQLite.
#[derive(Debug, Clone, PartialEq)]
pub struct MeetingProgress {
    /// Capture and job facts.
    pub capture: MeetingCapture,
    /// Number of transcript segments.
    pub segment_count: u64,
    /// End time of the last segment.
    pub last_end_ms: u64,
}

pub(crate) fn status_selection(statuses: &[MeetingStatus]) -> String {
    statuses
        .iter()
        .map(|s| format!("'{}'", s.as_str()))
        .collect::<Vec<_>>()
        .join(", ")
}

impl Store {
    /// Recovery candidates, filtered before reading any archive payload.
    pub fn meeting_captures_with_status(
        &self,
        statuses: &[MeetingStatus],
    ) -> Result<Vec<MeetingCapture>, StoreError> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {CAPTURE_COLUMNS} FROM meetings WHERE status IN ({}) ORDER BY created_at, seq",
            status_selection(statuses)
        ))?;
        Ok(stmt
            .query_map([], capture_from_row)?
            .collect::<Result<_, _>>()?)
    }

    /// Polling facts for one meeting. Full detail JSON is never transferred to Rust.
    pub fn meeting_progress(&self, id: &str) -> Result<Option<MeetingProgress>, StoreError> {
        Ok(self.conn().query_row(&format!(
            "SELECT {CAPTURE_COLUMNS}, \
             CASE WHEN segments IS NULL THEN 0 WHEN json_type(segments) = 'array' THEN json_array_length(segments) END, \
             CASE WHEN segments IS NULL OR json_array_length(segments) = 0 THEN 0 ELSE json_extract(segments, '$[#-1].end_ms') END \
             FROM meetings WHERE id = ?1"
        ), [id], |r| Ok(MeetingProgress {
            capture: capture_from_row(r)?, segment_count: r.get(13)?, last_end_ms: r.get(14)?,
        })).optional()?)
    }
}
