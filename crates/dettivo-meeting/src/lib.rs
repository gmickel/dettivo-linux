//! The meeting session (FR-G1, FR-G2, FR-G7, FR-A2, ADR 0027, ADR 0030):
//! one meeting per daemon that records the microphone and the monitor of
//! the default sink as two takes on one clock under the meeting's
//! directory, journals what happens to them, writes a live checkpoint on
//! an interval, survives a microphone that vanishes by starting a new
//! take on the new default, transcribes both tracks while they record
//! (`live`) and again in full when the meeting stops (`finalize`), and
//! leaves a killed daemon a directory it can promote to a partial meeting
//! at its next start (`recovery`) and finalise from there. `diarize`
//! holds the post-meeting speaker pass's track reader and assignment
//! rule (ADR 0035); `transcript` reads the transcript so far, live until
//! the settled row replaces it (ADR 0071).

pub mod checkpoint;
pub mod diarize;
pub mod finalize;
pub mod journal;
pub mod live;
pub mod machine;
pub mod recovery;
mod sentences;
pub mod source;
pub mod transcript;
mod worker;
mod worker_end;
mod worker_microphone;

use std::time::Duration;

use dettivo_storage::meetings::MeetingRow;
use dettivo_storage::retention::ArtifactPolicy;
use dettivo_transcribe::live::{LiveSegment, LiveSettings};
use serde::{Deserialize, Serialize};

/// Package name of this crate, as declared in its manifest.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Package names of the workspace crates this crate builds on.
pub const UPSTREAM: &[&str] = &[
    dettivo_proto::CRATE_NAME,
    dettivo_audio::CRATE_NAME,
    dettivo_storage::CRATE_NAME,
    dettivo_transcribe::CRATE_NAME,
    dettivo_speech::CRATE_NAME,
    dettivo_engine_proto::CRATE_NAME,
];

/// The file the journal is appended to under the meeting directory.
pub const JOURNAL_FILE: &str = "journal.jsonl";
/// The capture manifest under the meeting directory; transcript content lives in the store.
pub const METADATA_FILE: &str = "metadata.json";

/// Where a meeting stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// No meeting.
    Idle,
    /// Both tracks record.
    Recording,
    /// The stop was asked for; the takes are being closed.
    Stopping,
    /// The capture ended; the takes are final and finalisation follows.
    Stopped,
    /// Finalisation runs over every take.
    Transcribing,
    /// The transcript is on the row.
    Completed,
    /// Cancelled; the takes are gone.
    Cancelled,
    /// The capture failed (a take that could not be written) or the
    /// finalisation did; the takes so far are kept.
    Failed,
    /// A daemon restart interrupted the meeting; its audio is preserved.
    Partial,
}

impl State {
    /// The wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Recording => "recording",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Transcribing => "transcribing",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::Partial => "partial",
        }
    }
}

/// Which track a level or a take belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Track {
    /// The microphone (the default source, or the pinned device).
    Microphone,
    /// The monitor of the default sink.
    System,
}

impl Track {
    /// The take file prefix (`microphone.wav`, `system.wav`).
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Microphone => "microphone",
            Self::System => "system",
        }
    }
}

/// A meter sample for the publisher.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Level {
    /// Which track.
    pub track: Track,
    /// Root mean square, 0..1.
    pub rms: f32,
    /// Peak, 0..1.
    pub peak: f32,
}

/// What a meeting freezes at start.
#[derive(Debug, Clone, PartialEq)]
pub struct Policy {
    /// Keep both tracks after the meeting.
    pub keep_audio: bool,
    /// What lives in the meeting directory once it stopped.
    pub artifacts: ArtifactPolicy,
    /// How often the live checkpoint is written.
    pub checkpoint_interval: Duration,
    /// The meter interval.
    pub level_interval_ms: u64,
    /// Transcribe while recording; off runs the finalisation alone.
    pub live: bool,
    /// The live tuning; the finalisation shares its cross-source padding
    /// even when the live path is off.
    pub tuning: LiveSettings,
    /// The chunked pipeline's values for finalisation.
    pub transcribe: dettivo_transcribe::Settings,
    /// The vocabulary prompt, when any.
    pub prompt: Option<String>,
}

/// A state change for the publisher, with the bounded live metadata the
/// `meeting.state` topic carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateChange {
    /// The meeting id.
    pub meeting_id: String,
    /// The job id (`job_meeting_<n>`).
    pub job_id: String,
    /// The state entered.
    pub state: State,
    /// The state left.
    pub previous: State,
    /// Why, when it matters (`device lost: <name>`, `cancelled`, an error).
    pub reason: Option<String>,
    /// Milliseconds captured so far.
    pub duration_ms: u64,
    /// Microphone takes written so far.
    pub microphone_takes: u32,
    /// True while the system track records.
    pub system_audio: bool,
    /// Final live segments so far.
    pub live_segment_count: u64,
    /// The end of the last final live segment.
    pub live_last_end_ms: u64,
    /// True while finalisation runs.
    pub is_finalizing: bool,
    /// The speaker pass the finalisation planned (`queued`), on the
    /// `completed` transition; absent otherwise.
    pub diarization_status: Option<dettivo_proto::methods::speakers::DiarizationStatus>,
    /// The analysis the finalisation planned (`queued`), on the
    /// `completed` transition; absent otherwise.
    pub analysis_status: Option<dettivo_proto::methods::meetings_notes::AnalysisStatus>,
}

/// Where finalisation stands, for `job.progress`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FinalizeProgress {
    /// `transcribing`, `merging`, `done`, `failed`, `cancelled`.
    pub stage: dettivo_transcribe::Stage,
    /// Chunks finished over every take.
    pub chunks_done: u32,
    /// Chunks in total.
    pub chunks_total: u32,
}

impl FinalizeProgress {
    /// `chunks_done / chunks_total`, 1 when done.
    pub fn fraction(&self) -> f64 {
        if self.chunks_total == 0 {
            f64::from(self.stage == dettivo_transcribe::Stage::Done)
        } else {
            f64::from(self.chunks_done) / f64::from(self.chunks_total)
        }
    }
}

/// Receives state changes, levels, live segments and finalisation progress.
pub trait Publisher: Send + Sync {
    /// A meeting state change.
    fn state(&self, change: &StateChange);
    /// A meter sample while recording.
    fn level(&self, level: Level);
    /// A live segment, provisional or final.
    fn segment(&self, meeting_id: &str, segment: &LiveSegment);
    /// Finalisation progress of the job.
    fn progress(&self, job_id: &str, meeting_id: &str, progress: FinalizeProgress);
}

/// Keeps the meeting row current: the store writes it at start and on
/// every change the worker makes, so a client that lists meetings sees
/// the same facts the session holds.
pub trait Archive: Send + Sync {
    /// Inserts the row at start.
    fn started(&self, row: &MeetingRow) -> Result<(), String>;
    /// Rewrites the row after a change.
    fn updated(&self, row: &MeetingRow) -> Result<(), String>;
    /// Runs once a finalisation produced the transcript and before the
    /// completed row is stored, so the daemon can fill what derives from
    /// the segments (the polished text, ADR 0036).
    fn completed(&self, _row: &mut MeetingRow) {}
    /// Runs once the completed row is stored and the finalisation slot is
    /// freed, so the daemon can start what follows it (the analysis).
    fn settled(&self, _row: &MeetingRow, _keep_audio: bool, _artifacts: ArtifactPolicy) {}
}

/// Why a session call was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeetingError {
    /// A meeting is active; `start` is refused (`CONFLICT`).
    Active,
    /// No meeting is active (`NOT_FOUND`).
    NoSession,
    /// The microphone could not open.
    Audio(String),
    /// The store refused the row.
    Store(String),
    /// The live tuning cannot cut windows; names the keys.
    Settings(String),
}

impl std::fmt::Display for MeetingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "A meeting session is active"),
            Self::NoSession => write!(f, "no meeting session is active"),
            Self::Audio(e) => write!(f, "capture failed: {e}"),
            Self::Store(e) => write!(f, "history: {e}"),
            Self::Settings(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for MeetingError {}

/// The disclosure text a user acknowledges before the first meeting, as
/// the macOS app shows it (`AppState.meetingDisclosureMessage`).
pub const DISCLOSURE_MESSAGE: &str = "This meeting may be recorded and transcribed by Dettivo. Please inform all participants and comply with local laws and company policy before recording.";

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package_and_edges_resolve() {
        assert_eq!(super::CRATE_NAME, "dettivo-meeting");
        assert_eq!(
            super::UPSTREAM,
            &[
                "dettivo-proto",
                "dettivo-audio",
                "dettivo-storage",
                "dettivo-transcribe",
                "dettivo-speech",
                "dettivo-engine-proto"
            ]
        );
        assert_eq!(super::State::Partial.as_str(), "partial");
        assert_eq!(super::State::Transcribing.as_str(), "transcribing");
        assert_eq!(super::Track::System.prefix(), "system");
    }
}
