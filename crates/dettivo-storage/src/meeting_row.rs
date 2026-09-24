//! The meeting row: the macOS `MeetingSession` fields as one row of the
//! `meetings` table (migration `0004-meetings`), the diarization block
//! and the speakers (`0006-speakers`, ADR 0035), the notes and analysis
//! columns migration `0007-notes-analysis` adds (ADR 0036), and the
//! mapping between the struct and the columns. The speakers travel with
//! the row: every write replaces them, every read attaches them. The
//! repository over the rows lives in `meetings`.

use dettivo_proto::methods::meetings::Segment;
use dettivo_proto::methods::meetings_notes::{Analysis, AnalysisStatus, NotesSource};
use dettivo_proto::methods::speakers::{Diarization, Speaker};
use rusqlite::Row;
use serde::{Deserialize, Serialize};

use crate::time::now_iso;

/// The columns of a meeting row, in the order `from_row` reads them; the
/// last is `seq`.
pub(crate) const COLUMNS: &str = "id, created_at, updated_at, title, title_source, status, \
source_kind, started_at, ended_at, duration_ms, language, stt_provider, stt_model, is_partial, \
chunks_completed, chunks_total, disclosure_acknowledged_at, system_audio, microphone_takes, \
audio_dir, raw_text, final_text, summary, segments, error_code, error_message, \
original_filename, recovery_reason, diarization, speaker_names, notes_markdown, notes_source, \
notes_updated_at, analysis, analysis_text, analysis_status, analysis_error, analysis_model, \
analysis_at, seq";

/// The mutable columns, in the order `values` yields them: the first
/// `ROW_FIELD_COUNT` belong to the capture, the transcription and the
/// speaker pass and are what `update_meeting` rewrites; the rest (the
/// summary, the notes and the analysis) belong to their own setters in
/// `meeting_notes` and are written whole only by the insert.
pub(crate) const FIELDS: &str = "created_at, updated_at, title, title_source, status, source_kind, \
started_at, ended_at, duration_ms, language, stt_provider, stt_model, is_partial, \
chunks_completed, chunks_total, disclosure_acknowledged_at, system_audio, microphone_takes, \
audio_dir, raw_text, final_text, segments, error_code, error_message, \
original_filename, recovery_reason, diarization, speaker_names, summary, notes_markdown, \
notes_source, notes_updated_at, analysis, analysis_text, analysis_status, analysis_error, \
analysis_model, analysis_at";

/// How many mutable columns `FIELDS` names.
pub(crate) const FIELD_COUNT: usize = 38;

/// How many of `FIELDS` a whole-row update owns.
pub(crate) const ROW_FIELD_COUNT: usize = 28;

/// The index of `seq` in a `COLUMNS` selection.
pub(crate) const SEQ_INDEX: usize = 39;

/// Where a meeting stands (the macOS `MeetingSessionStatus` plus the Linux
/// `stopping`, `stopped` and `partial` states, ADR 0027).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeetingStatus {
    /// Both tracks record.
    Recording,
    /// The stop was asked for; the takes are being closed.
    Stopping,
    /// The capture ended; transcription has not run.
    Stopped,
    /// A job is producing the transcript.
    Transcribing,
    /// The transcript is final.
    Completed,
    /// The capture or the job failed; the error fields say why.
    Failed,
    /// Cancelled by the client; the takes are gone.
    Cancelled,
    /// A daemon restart interrupted the meeting; its audio is preserved.
    Partial,
}

impl MeetingStatus {
    /// The stored spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recording => "recording",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Transcribing => "transcribing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Partial => "partial",
        }
    }

    /// Parses the stored spelling.
    pub fn parse(text: &str) -> Option<Self> {
        [
            Self::Recording,
            Self::Stopping,
            Self::Stopped,
            Self::Transcribing,
            Self::Completed,
            Self::Failed,
            Self::Cancelled,
            Self::Partial,
        ]
        .into_iter()
        .find(|s| s.as_str() == text)
    }
}

/// One meeting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeetingRow {
    /// The UUID the contract addresses the meeting by.
    pub id: String,
    /// ISO 8601 UTC creation time (the start).
    pub created_at: String,
    /// ISO 8601 UTC time of the last change.
    pub updated_at: String,
    /// The display title.
    pub title: String,
    /// `auto` (derived) or `manual` (given by the caller).
    pub title_source: String,
    /// Where the meeting stands.
    pub status: MeetingStatus,
    /// `capture` (two-stream capture) or `audioImport`.
    pub source_kind: String,
    /// ISO 8601 UTC start.
    pub started_at: String,
    /// ISO 8601 UTC end, once stopped.
    pub ended_at: Option<String>,
    /// Milliseconds of audio.
    pub duration_ms: u64,
    /// The language the meeting is transcribed in.
    pub language: String,
    /// The speech provider frozen at start.
    pub stt_provider: String,
    /// The model frozen at start.
    pub stt_model: String,
    /// True when a restart promoted the meeting to `partial`.
    pub is_partial: bool,
    /// Chunks finished per the checkpoint.
    pub chunks_completed: u32,
    /// Chunks total per the checkpoint.
    pub chunks_total: u32,
    /// When the recording disclosure was acknowledged for this meeting.
    pub disclosure_acknowledged_at: Option<String>,
    /// True when the system track (the default sink's monitor) recorded.
    pub system_audio: bool,
    /// Microphone takes written.
    pub microphone_takes: u32,
    /// The meeting directory holding the takes, the journal and the sidecars.
    pub audio_dir: Option<String>,
    /// The transcript before the raw pipeline.
    pub raw_text: String,
    /// The transcript after it (the polished join once finalised).
    pub final_text: String,
    /// The analysis summary, empty until one is ready.
    pub summary: String,
    /// The transcript segments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segments: Vec<Segment>,
    /// A stable error code when `failed`.
    pub error_code: Option<String>,
    /// The error message when `failed`.
    pub error_message: Option<String>,
    /// The upload's file name for an imported meeting.
    pub original_filename: Option<String>,
    /// Why the meeting is partial, when the checkpoint could not be read.
    pub recovery_reason: Option<String>,
    /// The diarization block (ADR 0035); `None` before any pass was asked
    /// for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diarization: Option<Diarization>,
    /// The speakers, `you` first, then in order of first appearance;
    /// written with the row and read back with it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub speakers: Vec<Speaker>,
    /// The user's notes, Markdown; never touched by a model.
    #[serde(default)]
    pub notes_markdown: String,
    /// Who last wrote the notes.
    #[serde(default)]
    pub notes_source: NotesSource,
    /// When the notes were last written.
    #[serde(default)]
    pub notes_updated_at: Option<String>,
    /// The analysis, once ready (kept through a failed regenerate).
    #[serde(default)]
    pub analysis: Option<Analysis>,
    /// Where the analysis stands.
    #[serde(default)]
    pub analysis_status: AnalysisStatus,
    /// Why the last analysis failed.
    #[serde(default)]
    pub analysis_error: Option<String>,
    /// The model that produced the analysis.
    #[serde(default)]
    pub analysis_model: Option<String>,
    /// When the analysis was produced.
    #[serde(default)]
    pub analysis_at: Option<String>,
}

impl MeetingRow {
    /// A new recording meeting starting now with a fresh id.
    pub fn new() -> Self {
        let now = now_iso();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: now.clone(),
            updated_at: now.clone(),
            title: String::new(),
            title_source: "auto".into(),
            status: MeetingStatus::Recording,
            source_kind: "capture".into(),
            started_at: now,
            ended_at: None,
            duration_ms: 0,
            language: String::new(),
            stt_provider: String::new(),
            stt_model: String::new(),
            is_partial: false,
            chunks_completed: 0,
            chunks_total: 0,
            disclosure_acknowledged_at: None,
            system_audio: false,
            microphone_takes: 0,
            audio_dir: None,
            raw_text: String::new(),
            final_text: String::new(),
            summary: String::new(),
            segments: Vec::new(),
            error_code: None,
            error_message: None,
            original_filename: None,
            recovery_reason: None,
            diarization: None,
            speakers: Vec::new(),
            notes_markdown: String::new(),
            notes_source: NotesSource::User,
            notes_updated_at: None,
            analysis: None,
            analysis_status: AnalysisStatus::None,
            analysis_error: None,
            analysis_model: None,
            analysis_at: None,
        }
    }

    /// The contract's `duration_seconds`, rounded.
    pub fn duration_seconds(&self) -> u64 {
        self.duration_ms.div_ceil(1000)
    }

    /// The title a meeting without one gets: `Meeting <date> <time>` from
    /// its start in the host timezone, so it agrees with the local facts line.
    pub fn derived_title(started_at: &str) -> String {
        let stamp = crate::time::local_title_stamp(started_at)
            .unwrap_or_else(|| started_at.get(..16).unwrap_or(started_at).replace('T', " "));
        format!("Meeting {stamp}")
    }

    /// The speaker names joined, for the FTS column.
    pub fn speaker_names(&self) -> String {
        self.speakers
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The speaker with `speaker_id`.
    pub fn speaker(&self, speaker_id: &str) -> Option<&Speaker> {
        self.speakers.iter().find(|s| s.speaker_id == speaker_id)
    }

    /// The speaker names as the exports list them: the speakers' names
    /// once the diarization pass named them, else the distinct labels on
    /// the segments.
    pub fn speaker_labels(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .speakers
            .iter()
            .map(|s| s.name.clone())
            .filter(|n| !n.is_empty())
            .collect();
        if !out.is_empty() {
            return out;
        }
        for s in self.segments.iter().filter_map(|s| s.speaker.as_deref()) {
            if !out.iter().any(|o| o == s) {
                out.push(s.to_string());
            }
        }
        out
    }

    /// The transcript text as the polished segments read it, or the
    /// engine's words for a segment the pass has not seen.
    pub fn polished_join(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.polished_text.as_deref().unwrap_or(&s.text).trim())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl Default for MeetingRow {
    fn default() -> Self {
        Self::new()
    }
}

fn segments_json(row: &MeetingRow) -> Option<String> {
    (!row.segments.is_empty()).then(|| serde_json::to_string(&row.segments).unwrap_or_default())
}

/// A JSON column: SQL NULL is absent data; text that does not parse is
/// an error naming the meeting and the column, so a row whose stored
/// payload cannot be read is never read back as empty and rewritten.
fn json_column<T: serde::de::DeserializeOwned>(
    row: &Row<'_>,
    index: usize,
    id: &str,
    column: &str,
) -> rusqlite::Result<Option<T>> {
    let text: Option<String> = row.get(index)?;
    match text {
        None => Ok(None),
        Some(text) => serde_json::from_str(&text).map(Some).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                rusqlite::types::Type::Text,
                format!("meeting {id}: the {column} column holds JSON that does not parse: {e}")
                    .into(),
            )
        }),
    }
}

pub(crate) fn from_row(row: &Row<'_>) -> rusqlite::Result<(MeetingRow, i64)> {
    let status: String = row.get(5)?;
    let id: String = row.get(0)?;
    let segments: Vec<Segment> = json_column(row, 23, &id, "segments")?.unwrap_or_default();
    let duration: i64 = row.get(9)?;
    let is_partial: i64 = row.get(13)?;
    let system_audio: i64 = row.get(17)?;
    let diarization = json_column(row, 28, &id, "diarization")?;
    let notes_source: String = row.get(31)?;
    let analysis = json_column(row, 33, &id, "analysis")?;
    let analysis_status: String = row.get(35)?;
    Ok((
        MeetingRow {
            id,
            created_at: row.get(1)?,
            updated_at: row.get(2)?,
            title: row.get(3)?,
            title_source: row.get(4)?,
            status: MeetingStatus::parse(&status).unwrap_or(MeetingStatus::Failed),
            source_kind: row.get(6)?,
            started_at: row.get(7)?,
            ended_at: row.get(8)?,
            duration_ms: u64::try_from(duration).unwrap_or(0),
            language: row.get(10)?,
            stt_provider: row.get(11)?,
            stt_model: row.get(12)?,
            is_partial: is_partial != 0,
            chunks_completed: row.get::<_, i64>(14).map(|n| n.max(0) as u32)?,
            chunks_total: row.get::<_, i64>(15).map(|n| n.max(0) as u32)?,
            disclosure_acknowledged_at: row.get(16)?,
            system_audio: system_audio != 0,
            microphone_takes: row.get::<_, i64>(18).map(|n| n.max(0) as u32)?,
            audio_dir: row.get(19)?,
            raw_text: row.get(20)?,
            final_text: row.get(21)?,
            summary: row.get(22)?,
            segments,
            error_code: row.get(24)?,
            error_message: row.get(25)?,
            original_filename: row.get(26)?,
            recovery_reason: row.get(27)?,
            diarization,
            speakers: Vec::new(),
            notes_markdown: row.get(30)?,
            notes_source: NotesSource::parse(&notes_source),
            notes_updated_at: row.get(32)?,
            analysis,
            analysis_status: AnalysisStatus::parse(&analysis_status),
            analysis_error: row.get(36)?,
            analysis_model: row.get(37)?,
            analysis_at: row.get(38)?,
        },
        row.get(SEQ_INDEX)?,
    ))
}

pub(crate) fn values(row: &MeetingRow) -> [Box<dyn rusqlite::ToSql>; FIELD_COUNT] {
    [
        Box::new(row.created_at.clone()),
        Box::new(row.updated_at.clone()),
        Box::new(row.title.clone()),
        Box::new(row.title_source.clone()),
        Box::new(row.status.as_str()),
        Box::new(row.source_kind.clone()),
        Box::new(row.started_at.clone()),
        Box::new(row.ended_at.clone()),
        Box::new(i64::try_from(row.duration_ms).unwrap_or(i64::MAX)),
        Box::new(row.language.clone()),
        Box::new(row.stt_provider.clone()),
        Box::new(row.stt_model.clone()),
        Box::new(i64::from(row.is_partial)),
        Box::new(i64::from(row.chunks_completed)),
        Box::new(i64::from(row.chunks_total)),
        Box::new(row.disclosure_acknowledged_at.clone()),
        Box::new(i64::from(row.system_audio)),
        Box::new(i64::from(row.microphone_takes)),
        Box::new(row.audio_dir.clone()),
        Box::new(row.raw_text.clone()),
        Box::new(row.final_text.clone()),
        Box::new(segments_json(row)),
        Box::new(row.error_code.clone()),
        Box::new(row.error_message.clone()),
        Box::new(row.original_filename.clone()),
        Box::new(row.recovery_reason.clone()),
        Box::new(
            row.diarization
                .as_ref()
                .map(|d| serde_json::to_string(d).unwrap_or_default()),
        ),
        Box::new(row.speaker_names()),
        Box::new(row.summary.clone()),
        Box::new(row.notes_markdown.clone()),
        Box::new(row.notes_source.as_str()),
        Box::new(row.notes_updated_at.clone()),
        Box::new(
            row.analysis
                .as_ref()
                .map(|a| serde_json::to_string(a).unwrap_or_default()),
        ),
        Box::new(
            row.analysis
                .as_ref()
                .map(Analysis::searchable_text)
                .unwrap_or_default(),
        ),
        Box::new(row.analysis_status.as_str()),
        Box::new(row.analysis_error.clone()),
        Box::new(row.analysis_model.clone()),
        Box::new(row.analysis_at.clone()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_column_lists_agree_with_the_mapping() {
        assert_eq!(COLUMNS.split(", ").count(), SEQ_INDEX + 1);
        assert_eq!(FIELDS.split(", ").count(), FIELD_COUNT);
        assert_eq!(
            FIELDS.split(", ").nth(ROW_FIELD_COUNT),
            Some("summary"),
            "the summary, the notes and the analysis follow the row's own fields"
        );
        assert_eq!(
            COLUMNS.split(", ").nth(SEQ_INDEX),
            Some("seq"),
            "seq is the last column"
        );
        let row = MeetingRow::new();
        assert_eq!(values(&row).len(), FIELD_COUNT);
        for s in ["recording", "stopping", "stopped", "partial", "cancelled"] {
            assert_eq!(MeetingStatus::parse(s).unwrap().as_str(), s);
        }
    }
}
