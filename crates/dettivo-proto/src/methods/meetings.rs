//! `meetings.*`: `start`, `stop`, `cancel`, `status`, `list`, `get`,
//! `search` and `delete` (`docs/api/dettivo-ipc-v1.md` section 8.3 and
//! 9), plus the Linux additions `recover`, `discard`, `disclosure.get`
//! and `disclosure.acknowledge` (`docs/api/linux-deltas.md`). The notes
//! and analysis additions live in `meetings_notes`.

use crate::id::Id;
use crate::methods::meetings_notes::{Analysis, AnalysisStatus, NotesSource};
use crate::runtime::{HistoryStatus, JobStatus, TranscriptRef};
use serde::{Deserialize, Serialize};

/// Requested audio sources for `meetings.start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    /// Capture system/loopback audio.
    pub system_audio: bool,
    /// Capture the microphone.
    pub microphone: bool,
}

/// `meetings.start` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartParams {
    /// Requested audio sources.
    pub capture: Capture,
    /// Optional display title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Adopted from the Windows port (registered in
    /// `docs/api/linux-deltas.md`): the caller confirms the meeting
    /// participants have been told the meeting is recorded. The daemon
    /// answers `CONFLICT` with detail kind `meetingDisclosureRequired`
    /// when its configuration requires the acknowledgement and it is
    /// absent or `false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acknowledge_meeting_disclosure: Option<bool>,
    /// Linux addition: the language the meeting is transcribed in; absent
    /// means `[dictation] language`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Linux addition (ADR 0035): the speaker count the diarization pass
    /// is told; absent lets the clustering decide.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_speakers: Option<u32>,
    /// Linux addition: run the diarization pass once the meeting
    /// completes; absent means `[meetings.diarization] auto`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diarize: Option<bool>,
    /// Linux addition: run the analysis once the meeting finalises;
    /// absent means `[meetings.analysis] auto`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analyze: Option<bool>,
}

/// `meetings.start` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartResult {
    /// The started meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// The started job's status.
    pub job: JobStatus,
}

/// `meetings.stop` / `meetings.cancel` / `meetings.status` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingIdParams {
    /// The target meeting's id.
    pub meeting_id: Id,
}

/// `meetings.stop` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopResult {
    /// The meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// The finishing job's status.
    pub job: JobStatus,
}

/// `meetings.cancel` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelResult {
    /// The meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// The cancelled job's status.
    pub job: JobStatus,
}

/// `meetings.status` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusResult {
    /// The meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// Current lifecycle status.
    pub status: HistoryStatus,
    /// Live segment count so far.
    pub live_segment_count: u64,
    /// End timestamp in ms of the last live segment.
    pub live_last_end_ms: u64,
    /// Whether the meeting is in its finalizing phase.
    pub is_finalizing: bool,
    /// The active job's status, when a job is running.
    pub job: Option<JobStatus>,
    /// Linux addition: the capture facts of this meeting (`docs/api/
    /// linux-deltas.md`); absent on a port without the block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture: Option<CaptureStatus>,
    /// Linux addition: every meeting a daemon restart promoted to
    /// `partial`, waiting for `meetings.recover` or `meetings.discard`;
    /// omitted when there is none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recoverable: Vec<Recoverable>,
}

/// Linux addition on `meetings.status`: what the capture has written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureStatus {
    /// Milliseconds captured so far (or in total once stopped).
    pub duration_ms: u64,
    /// Microphone takes written (`microphone.wav`, `microphone-2.wav`, ...).
    pub microphone_takes: u32,
    /// True while the system track (the default sink's monitor) records.
    pub system_audio: bool,
    /// True when a daemon restart promoted the meeting to `partial`.
    pub is_partial: bool,
    /// Chunks the checkpoint recorded as finished; 0 before transcription.
    pub chunks_completed: u32,
    /// Chunks the checkpoint recorded in total; 0 before transcription.
    pub chunks_total: u32,
    /// Why the meeting is partial, when the checkpoint could not be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Linux addition: one meeting waiting for recover or discard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recoverable {
    /// The meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// Display title.
    pub title: String,
    /// ISO 8601 start timestamp.
    pub started_at: String,
    /// Milliseconds of audio preserved.
    pub duration_ms: u64,
    /// Chunks completed per the checkpoint.
    pub chunks_completed: u32,
    /// Chunks total per the checkpoint.
    pub chunks_total: u32,
    /// Why the meeting is partial.
    pub reason: String,
}

/// `meetings.recover` and `meetings.discard` params (Linux additions).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoverParams {
    /// The partial meeting.
    pub meeting_id: Id,
}

/// `meetings.recover` result: the meeting kept with its audio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoverResult {
    /// The meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// The status after the recovery (`stopped` until transcription lands).
    pub status: HistoryStatus,
    /// Milliseconds of audio preserved.
    pub duration_ms: u64,
}

/// `meetings.discard` result: the meeting removed per the artifact policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscardResult {
    /// The meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// Always true; a meeting that is not partial is `CONFLICT`.
    pub discarded: bool,
}

/// `meetings.delete` params (section 9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteParams {
    /// The meeting to delete.
    pub meeting_id: Id,
    /// How much of the meeting's artifacts to remove; absent on Linux
    /// means `[meetings] delete_artifact_policy` (docs/api/linux-deltas.md).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_policy: Option<ArtifactPolicy>,
}

/// How much of a deleted meeting's artifacts to remove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactPolicy {
    /// Remove only the transcript.
    TranscriptOnly,
    /// Remove the transcript and the recorded audio.
    TranscriptAndAudio,
    /// Remove every artifact.
    All,
}

/// `meetings.delete` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteResult {
    /// The meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// Always true on success.
    pub deleted: bool,
}

/// `meetings.disclosure.get` and `meetings.disclosure.acknowledge` result
/// (Linux additions): the recording disclosure and whether it was
/// acknowledged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisclosureResult {
    /// True once the acknowledgement is recorded in the state file.
    pub acknowledged: bool,
    /// ISO 8601 time of the acknowledgement, when any.
    pub acknowledged_at: Option<String>,
    /// The disclosure text the user acknowledges (the macOS message).
    pub message: String,
}

/// `meetings.list` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListParams {
    /// Maximum items to return.
    pub limit: u32,
    /// Pagination cursor from a prior page.
    pub cursor: Option<String>,
}

/// One row of a `meetings.list` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListItem {
    /// The meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// Display title.
    pub title: String,
    /// ISO 8601 start timestamp.
    pub started_at: String,
    /// Duration in seconds.
    pub duration_seconds: u64,
    /// Current lifecycle status.
    pub status: HistoryStatus,
    /// Linux addition (ADR 0035): how many speakers the meeting has; 0
    /// before the diarization pass. Omitted on the wire when 0.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub speaker_count: u32,
    /// Linux addition: the analysis summary, empty until one is ready.
    #[serde(default)]
    pub summary: String,
    /// Linux addition: true when the meeting has notes.
    #[serde(default)]
    pub has_notes: bool,
    /// Linux addition: where the analysis stands.
    #[serde(default)]
    pub analysis_status: AnalysisStatus,
    /// Linux addition: true when a restart left the meeting partial.
    #[serde(default)]
    pub is_partial: bool,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// `meetings.list` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListResult {
    /// The page of meetings.
    pub items: Vec<ListItem>,
    /// Cursor for the next page, `None` when exhausted.
    pub next_cursor: Option<String>,
}

/// `meetings.get` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetParams {
    /// The target meeting's id.
    pub meeting_id: Id,
}

/// Audio source for a transcript segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentSource {
    /// Captured from the microphone.
    Microphone,
    /// Captured from system/loopback audio.
    System,
    /// Merged from multiple sources.
    Merged,
}

/// One word inside a segment with its timestamps, a Linux addition on
/// `transcripts.get` for imported and re-run items whose engine aligns
/// words (registered in `docs/api/linux-deltas.md`); absent otherwise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Word {
    /// Start offset in ms.
    pub start_ms: u64,
    /// End offset in ms.
    pub end_ms: u64,
    /// The word as the engine wrote it, punctuation attached.
    pub text: String,
    /// The engine's confidence in (0, 1].
    pub confidence: f64,
}

/// One transcript segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    /// Zero-based segment index.
    pub index: u32,
    /// Start offset in ms.
    pub start_ms: u64,
    /// End offset in ms.
    pub end_ms: u64,
    /// Segment text.
    pub text: String,
    /// Speaker label, when diarization is available: the speaker's name
    /// (`You` on the microphone, `Speaker 1` or the renamed name).
    pub speaker: Option<String>,
    /// Linux addition (ADR 0035): the speaker's stable id (`you`,
    /// `speaker_00`, ...); omitted while unassigned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<String>,
    /// Linux addition: the winning speaker's share of the segment's
    /// diarized speech (0.6 to 1); omitted while unassigned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_confidence: Option<f64>,
    /// Which audio source produced this segment.
    pub source_type: SegmentSource,
    /// Linux addition: the words with timestamps when the engine aligned
    /// them; omitted on the wire when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<Word>,
    /// Linux addition: milliseconds of capture missing before this
    /// segment (a device switch, a window the live engine skipped);
    /// omitted on the wire when none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_before_ms: Option<u64>,
    /// Linux addition: the segment after the deterministic Polish pass
    /// the finalisation runs (`text` stays the engine's words); omitted
    /// until the meeting finalised.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polished_text: Option<String>,
}

/// `meetings.get` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetResult {
    /// The meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// Display title.
    pub title: String,
    /// ISO 8601 start timestamp.
    pub started_at: String,
    /// Duration in seconds.
    pub duration_seconds: u64,
    /// Current lifecycle status.
    pub status: HistoryStatus,
    /// Full transcript text (the polished join once finalised).
    pub transcript: String,
    /// Per-segment breakdown.
    pub segments: Vec<Segment>,
    /// Linux addition (ADR 0035): the speakers, `you` first, then in order
    /// of first appearance; omitted while there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub speakers: Vec<crate::methods::speakers::Speaker>,
    /// Linux addition: the diarization block; omitted before any pass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diarization: Option<crate::methods::speakers::Diarization>,
    /// Linux addition: the user's notes, Markdown.
    #[serde(default)]
    pub notes: String,
    /// Linux addition: who last wrote the notes.
    #[serde(default)]
    pub notes_source: NotesSource,
    /// Linux addition: the analysis, when ready.
    #[serde(default)]
    pub analysis: Option<Analysis>,
    /// Linux addition: where the analysis stands.
    #[serde(default)]
    pub analysis_status: AnalysisStatus,
    /// Linux addition: ISO 8601 end, once stopped.
    #[serde(default)]
    pub ended_at: Option<String>,
    /// Linux addition: the language the meeting was transcribed in.
    #[serde(default)]
    pub language: String,
    /// Linux addition: the speech provider frozen at start.
    #[serde(default)]
    pub stt_provider_id: String,
    /// Linux addition: the model frozen at start.
    #[serde(default)]
    pub stt_model_id: String,
}

/// `meetings.search` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchParams {
    /// Free-text query.
    pub query: String,
    /// Maximum results to return.
    pub limit: u32,
}

/// One `meetings.search` hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchItem {
    /// The matched meeting's reference.
    #[serde(rename = "ref")]
    pub reference: TranscriptRef,
    /// Matched text snippet.
    pub snippet: String,
    /// Relevance score; only meaningful when
    /// `knowledge.semantic_search=true` (section 8.3).
    pub score: Option<f64>,
    /// Linux addition: the column the snippet came from (`title`,
    /// `transcript`, `notes`, `analysis`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_field: Option<String>,
}

/// `meetings.search` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchResult {
    /// Matched meetings.
    pub items: Vec<SearchItem>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn start_params_title_is_optional() {
        let v = json!({"capture": {"system_audio": true, "microphone": true}});
        let params: StartParams = serde_json::from_value(v).unwrap();
        assert_eq!(params.title, None);
        assert_eq!(params.language, None);
    }

    #[test]
    fn status_omits_the_linux_blocks_when_empty() {
        let v = json!({
            "ref": {"kind": "meeting", "id": "0f8fad5b-d9cb-469f-a165-70867728950e"},
            "status": "recording", "live_segment_count": 0, "live_last_end_ms": 0,
            "is_finalizing": false, "job": null
        });
        let status: StatusResult = serde_json::from_value(v.clone()).unwrap();
        assert!(status.recoverable.is_empty() && status.capture.is_none());
        assert_eq!(serde_json::to_value(&status).unwrap(), v);
        assert_eq!(
            serde_json::to_string(&ArtifactPolicy::TranscriptAndAudio).unwrap(),
            "\"transcript_and_audio\""
        );
    }
}
