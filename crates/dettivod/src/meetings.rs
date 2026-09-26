//! The meeting service the `meetings.*` handlers drive (ADR 0027, ADR
//! 0030): the session machine from `dettivo-meeting`, the policy frozen
//! from the config in force, the two audio sources (PipeWire, or the
//! `DETTIVO_MOCK_MIC` and `DETTIVO_MOCK_SYSTEM_AUDIO` fixtures), the
//! publisher that feeds the event bus with `meeting.state`,
//! `meeting.segment`, `job.progress` and `audio.level` per source, the
//! disclosure acknowledgement in the state file, the completion hook that
//! starts the speaker pass (ADR 0035), the archive that polishes the
//! segments and starts the analysis (ADR 0036), and the recovery of a
//! meeting a killed daemon left recording.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use dettivo_audio::Target;
use dettivo_audio::mock::MockCapture;
use dettivo_audio::service::AudioService;
use dettivo_core::config::Loaded;
use dettivo_core::paths::Paths;
use dettivo_core::state::State as StateFile;
use dettivo_meeting::machine::{Finalizing, Session, Snapshot};
use dettivo_meeting::source::{Source, Sources};
use dettivo_meeting::{
    DISCLOSURE_MESSAGE, FinalizeProgress, Level, MeetingError, Policy, Publisher, State,
    StateChange, Track,
};
use dettivo_proto::events::{
    AudioLevelPayload, JobProgressPayload, LevelSource, MeetingLiveState, MeetingSegmentPayload,
    MeetingStatePayload, Topic,
};
use dettivo_proto::id::Id;
use dettivo_proto::methods::meetings::DisclosureResult;
use dettivo_speech::SttEngine;
use dettivo_storage::meetings::{MeetingArtifacts, MeetingRow, MeetingStatus};
use dettivo_storage::retention::ArtifactPolicy;
use dettivo_transcribe::live::{LiveSegment, LiveSettings};
use serde_json::json;

use crate::events::EventBus;
use crate::history::History;

/// The wire state of a session state; `idle` has no wire form.
fn live_state(state: State) -> Option<MeetingLiveState> {
    Some(match state {
        State::Idle => return None,
        State::Recording => MeetingLiveState::Recording,
        State::Stopping => MeetingLiveState::Stopping,
        State::Stopped => MeetingLiveState::Stopped,
        State::Transcribing => MeetingLiveState::Transcribing,
        State::Completed => MeetingLiveState::Completed,
        State::Cancelled => MeetingLiveState::Cancelled,
        State::Failed => MeetingLiveState::Failed,
        State::Partial => MeetingLiveState::Partial,
    })
}

/// Bridges session events onto the bus.
struct BusPublisher {
    bus: Arc<EventBus>,
}

impl Publisher for BusPublisher {
    fn state(&self, change: &StateChange) {
        tracing::info!(
            job = %change.job_id,
            state = change.state.as_str(),
            previous = change.previous.as_str(),
            reason = change.reason.as_deref().unwrap_or("-"),
            "meeting state"
        );
        let Some(state) = live_state(change.state) else {
            return;
        };
        let Ok(meeting_id) = Id::new(change.meeting_id.clone()) else {
            return;
        };
        self.bus.publish(
            Topic::MeetingState,
            json!(MeetingStatePayload {
                kind: "meeting".into(),
                state,
                meeting_id,
                live_segment_count: change.live_segment_count,
                live_last_end_ms: change.live_last_end_ms,
                is_finalizing: change.is_finalizing,
                previous_state: live_state(change.previous),
                reason: change.reason.clone(),
                duration_ms: Some(change.duration_ms),
                microphone_takes: Some(change.microphone_takes),
                system_audio: Some(change.system_audio),
                diarization_status: change.diarization_status,
                analysis_status: change.analysis_status,
            }),
        );
    }

    fn level(&self, level: Level) {
        self.bus.publish(
            Topic::AudioLevel,
            json!(AudioLevelPayload {
                rms: level.rms,
                peak: level.peak,
                source: match level.track {
                    Track::Microphone => LevelSource::Microphone,
                    Track::System => LevelSource::System,
                },
            }),
        );
    }

    fn segment(&self, meeting_id: &str, segment: &LiveSegment) {
        let Ok(meeting_id) = Id::new(meeting_id.to_string()) else {
            return;
        };
        let contract = segment.contract(0);
        self.bus.publish(
            Topic::MeetingSegment,
            json!(MeetingSegmentPayload {
                meeting_id,
                source: segment.source.as_str().to_string(),
                segment_id: segment.id.clone(),
                provisional: segment.provisional,
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                text: segment.text.clone(),
                words: contract.words,
                gap_before_ms: segment.gap_before_ms,
            }),
        );
    }

    fn progress(&self, job_id: &str, _meeting_id: &str, progress: FinalizeProgress) {
        self.bus.publish(
            Topic::JobProgress,
            json!(JobProgressPayload {
                job_id: job_id.to_string(),
                progress: progress.fraction(),
                chunks_done: Some(progress.chunks_done),
                chunks_total: Some(progress.chunks_total),
                stage: Some(progress.stage.as_str().to_string()),
            }),
        );
    }
}

/// Opens the two tracks: the fixtures under QA mode, else PipeWire. The
/// pinned microphone is read from the configuration in force at every
/// open, so a device re-pinned while the meeting runs is what a reopen
/// after a loss picks up.
struct DaemonSources {
    audio: Option<AudioService>,
    mock_mic: Option<PathBuf>,
    mock_system: Option<PathBuf>,
    config: Arc<RwLock<Loaded>>,
    system_source: String,
    system_requested: bool,
}

impl DaemonSources {
    fn input_device(&self) -> String {
        self.config
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .config
            .audio
            .input_device
            .clone()
    }
}

impl Sources for DaemonSources {
    fn open_microphone(&self, level_interval_ms: u64, reopen: bool) -> Result<Source, String> {
        if let Some(path) = &self.mock_mic {
            return MockCapture::open(path, level_interval_ms)
                .map(Source::Mock)
                .map_err(|e| format!("mock microphone: {e}"));
        }
        let Some(audio) = &self.audio else {
            return Err("PipeWire is not available".into());
        };
        let input_device = self.input_device();
        if !input_device.is_empty() {
            match audio.open(Target::Node(input_device), level_interval_ms) {
                Ok(c) => return Ok(Source::Capture(c)),
                // A pinned device that vanished mid-meeting: the meeting
                // carries on with the default source rather than losing
                // the room, and the journal's gap marker says so.
                Err(dettivo_audio::CaptureError::UnknownDevice(name)) if reopen => {
                    tracing::warn!(device = %name, "meeting: pinned device gone; the default source takes over");
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        audio
            .open(Target::Default, level_interval_ms)
            .map(Source::Capture)
            .map_err(|e| e.to_string())
    }

    fn open_system(&self, level_interval_ms: u64) -> Result<Source, String> {
        if !self.system_requested {
            return Err("system audio not requested".into());
        }
        if let Some(path) = &self.mock_system {
            return MockCapture::open(path, level_interval_ms)
                .map(Source::Mock)
                .map_err(|e| format!("mock system audio: {e}"));
        }
        let Some(audio) = &self.audio else {
            return Err("PipeWire is not available".into());
        };
        let target = if self.system_source == "default_monitor" {
            Target::SystemMonitor
        } else {
            Target::Node(self.system_source.clone())
        };
        audio
            .open(target, level_interval_ms)
            .map(Source::Capture)
            .map_err(|e| e.to_string())
    }
}

/// The service.
pub struct Meetings {
    session: Session,
    state_file: PathBuf,
}

impl Meetings {
    /// Meetings live under `<data>/meetings/<id>/`; rows go through the
    /// archive (the history service behind it); the disclosure
    /// acknowledgement lives in the state file.
    pub fn new(
        paths: &Paths,
        bus: Arc<EventBus>,
        artifacts: MeetingArtifacts,
        archive: Arc<dyn dettivo_meeting::Archive>,
    ) -> Self {
        Self {
            session: Session::new(artifacts, Arc::new(BusPublisher { bus }), archive),
            state_file: paths.state_file.clone(),
        }
    }

    /// Seeds or clears the acknowledgement in the state file
    /// (`DETTIVO_E2E_DISCLOSURE`, QA mode), so a drive shows the dialog
    /// on a fresh profile or skips it.
    pub fn seed_disclosure(&self, acknowledged: bool) -> Result<(), String> {
        let mut state = StateFile::load(&self.state_file).map_err(|e| e.to_string())?;
        state.acknowledgements.meeting_disclosure = acknowledged;
        state.acknowledgements.meeting_disclosure_at =
            acknowledged.then(dettivo_storage::time::now_iso);
        state.save(&self.state_file).map_err(|e| e.to_string())
    }

    /// The meeting directories.
    pub fn artifacts(&self) -> &MeetingArtifacts {
        self.session.artifacts()
    }

    /// The policy a meeting starting now would freeze: `[meetings]` with
    /// the live tuning (ADR 0030), `[transcribe]` for the finalisation and
    /// the `[dictation]` vocabulary as the prompt.
    pub fn policy(loaded: &Loaded) -> Policy {
        let m = &loaded.config.meetings;
        let d = &loaded.config.dictation;
        Policy {
            keep_audio: m.keep_audio,
            artifacts: ArtifactPolicy::parse(&m.artifacts).unwrap_or_else(|| {
                tracing::warn!(
                    "meetings.artifacts is not keep, audio_only or none; keeping artifacts"
                );
                ArtifactPolicy::Keep
            }),
            checkpoint_interval: Duration::from_secs(m.checkpoint_interval_seconds.max(1)),
            level_interval_ms: loaded.config.audio.level_interval_ms.max(1),
            live: m.live,
            tuning: LiveSettings {
                window_ms: m.live_window_ms,
                overlap_ms: m.live_overlap_ms,
                tick_ms: m.live_tick_ms,
                boundary_merge_gap_ms: m.boundary_merge_gap_ms,
                cross_source_padding_ms: m.cross_source_padding_ms,
                speech_floor_rms: m.speech_floor_rms,
            },
            transcribe: crate::jobs::settings_of(loaded),
            prompt: (!d.vocabulary.is_empty()).then(|| d.vocabulary.join(", ")),
        }
    }

    /// Starts a meeting on `row` with the sources the configuration and the
    /// QA environment select and `engine` for its transcription.
    pub fn start(
        &self,
        config: Arc<RwLock<Loaded>>,
        audio: Option<AudioService>,
        engine: Arc<dyn SttEngine>,
        system_requested: bool,
        row: MeetingRow,
    ) -> Result<Snapshot, MeetingError> {
        let fixture = |var: &str| {
            std::env::var_os(var)
                .map(PathBuf::from)
                .filter(|p| !p.as_os_str().is_empty())
        };
        let loaded = config.read().unwrap_or_else(|p| p.into_inner()).clone();
        let sources = Arc::new(DaemonSources {
            audio,
            mock_mic: fixture("DETTIVO_MOCK_MIC"),
            mock_system: fixture("DETTIVO_MOCK_SYSTEM_AUDIO"),
            config,
            system_source: loaded.config.audio.system_source.clone(),
            system_requested,
        });
        self.session
            .start(Self::policy(&loaded), sources, engine, row)
    }

    /// Asks the meeting to stop; answers the stopping snapshot at once.
    pub fn stop(&self) -> Result<Snapshot, MeetingError> {
        self.session.stop()
    }

    /// Cancels the meeting and waits for its worker.
    pub fn cancel(&self) -> Result<Snapshot, MeetingError> {
        self.session.cancel()
    }

    /// The active meeting.
    pub fn snapshot(&self) -> Option<Snapshot> {
        self.session.snapshot()
    }

    /// The finalisation of `meeting_id`, while one runs.
    pub fn finalizing(&self, meeting_id: &str) -> Option<Finalizing> {
        self.session.finalizing(meeting_id)
    }

    /// The transcript of `meeting_id` so far (`meetings.segments`, ADR
    /// 0071): the live tail while one is held, else what `stored` reads.
    pub fn read_segments<E>(
        &self,
        meeting_id: &str,
        since: Option<dettivo_proto::methods::meetings_segments::Cursor>,
        stored: impl FnOnce() -> Result<Vec<dettivo_proto::methods::meetings::Segment>, E>,
    ) -> Result<dettivo_meeting::transcript::Read, E> {
        self.session.read_segments(meeting_id, since, stored)
    }

    /// Stops the finalisation of `meeting_id` between chunks; true when
    /// one ran.
    pub fn cancel_finalize(&self, meeting_id: &str) -> bool {
        self.session.cancel_finalize(meeting_id)
    }

    /// Finalises a recovered meeting from its takes; answers the job id.
    pub fn finalize_recovered(
        &self,
        loaded: &Loaded,
        engine: Arc<dyn SttEngine>,
        row: MeetingRow,
    ) -> Result<String, MeetingError> {
        self.session
            .finalize_recovered(Self::policy(loaded), engine, row)
    }

    /// The disclosure and whether it was acknowledged.
    pub fn disclosure(&self) -> DisclosureResult {
        let state = StateFile::load(&self.state_file).unwrap_or_default();
        DisclosureResult {
            acknowledged: state.acknowledgements.meeting_disclosure,
            acknowledged_at: state.acknowledgements.meeting_disclosure_at,
            message: DISCLOSURE_MESSAGE.to_string(),
        }
    }

    /// Records the acknowledgement with the time, once.
    pub fn acknowledge(&self) -> Result<DisclosureResult, String> {
        let mut state = StateFile::load(&self.state_file).map_err(|e| e.to_string())?;
        if !state.acknowledgements.meeting_disclosure {
            state.acknowledgements.meeting_disclosure = true;
            state.acknowledgements.meeting_disclosure_at = Some(dettivo_storage::time::now_iso());
            state.save(&self.state_file).map_err(|e| e.to_string())?;
            tracing::info!("meeting disclosure acknowledged");
        }
        Ok(self.disclosure())
    }

    /// Promotes every meeting a previous daemon left `recording` or
    /// `stopping` to `partial` from its directory (FR-G7); returns how
    /// many. Runs before the socket listens.
    pub fn recover_stale(&self, history: &History) -> usize {
        let stale = match history.with_store(|s| {
            s.meetings_with_status(&[MeetingStatus::Recording, MeetingStatus::Stopping])
        }) {
            Ok(rows) => rows,
            Err(e) => {
                tracing::warn!(error = %e, "meetings: stale rows not read");
                return 0;
            }
        };
        let mut promoted = 0;
        for mut row in stale {
            let dir = self.artifacts().dir(&row.id);
            let found = dettivo_meeting::recovery::promote(&dir, &mut row);
            match history.with_store(|s| s.update_meeting(&row)) {
                Ok(()) => {
                    promoted += 1;
                    tracing::warn!(
                        id = %row.id,
                        duration_ms = found.duration_ms,
                        takes = found.microphone_takes,
                        checkpoint = found.reason.as_deref().unwrap_or("read"),
                        "meeting promoted to partial after a restart"
                    );
                }
                Err(e) => tracing::warn!(error = %e, "meeting: partial row not stored"),
            }
        }
        promoted
    }
}
