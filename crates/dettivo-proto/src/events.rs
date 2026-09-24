//! `events.*`: subscribe and unsubscribe, the `events.notify` server
//! notification, and the typed payloads the contract documents
//! (`docs/api/dettivo-ipc-v1.md` section 8.8). The four Linux topics are
//! registered in `docs/api/linux-deltas.md`; their notifications are
//! pinned by the `*.event.json` snapshots under `fixtures/events/`.

use crate::id::Id;
use crate::runtime::{InsertionMethod, InsertionOutcome, TargetApp};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// A subscribable event topic, spelled on the wire in dotted form
/// (`"dictation.state"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Topic {
    /// Dictation lifecycle state changes.
    DictationState,
    /// Meeting lifecycle state changes, with bounded live metadata.
    MeetingState,
    /// Background job progress.
    JobProgress,
    /// The subscriber's buffer overflowed and events were dropped.
    EventsOverflow,
    /// Linux addition: input level meter for the OSD and the bar.
    AudioLevel,
    /// Linux addition: speech model download progress.
    ModelDownload,
    /// Linux addition: engine process lifecycle.
    EngineState,
    /// Linux addition: incremental meeting transcript segments.
    MeetingSegment,
    /// Linux addition: `config.toml` was reloaded after a change on disk
    /// or a `config.set`, so an open editor refreshes.
    ConfigChanged,
}

impl Topic {
    /// Every topic, macOS topics first.
    pub const ALL: &'static [Topic] = &[
        Self::DictationState,
        Self::MeetingState,
        Self::JobProgress,
        Self::EventsOverflow,
        Self::AudioLevel,
        Self::ModelDownload,
        Self::EngineState,
        Self::MeetingSegment,
        Self::ConfigChanged,
    ];

    /// The topics the macOS contract defines.
    pub const MACOS: &'static [Topic] = &[
        Self::DictationState,
        Self::MeetingState,
        Self::JobProgress,
        Self::EventsOverflow,
    ];

    /// The dotted wire form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DictationState => "dictation.state",
            Self::MeetingState => "meeting.state",
            Self::JobProgress => "job.progress",
            Self::EventsOverflow => "events.overflow",
            Self::AudioLevel => "audio.level",
            Self::ModelDownload => "model.download",
            Self::EngineState => "engine.state",
            Self::MeetingSegment => "meeting.segment",
            Self::ConfigChanged => "config.changed",
        }
    }

    /// Parses the dotted wire form.
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|t| t.as_str() == value)
    }

    /// Whether the topic is a Linux addition rather than a macOS topic.
    pub fn is_linux_addition(self) -> bool {
        !Self::MACOS.contains(&self)
    }
}

impl Serialize for Topic {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Topic {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Topic::parse(&raw).ok_or_else(|| D::Error::custom(format!("unknown event topic: {raw:?}")))
    }
}

/// `events.subscribe` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscribeParams {
    /// Topics to subscribe to.
    pub topics: Vec<Topic>,
    /// Requested per-subscription buffer depth.
    pub buffer: u32,
}

/// `events.subscribe` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscribeResult {
    /// Opaque subscription id, for example `"sub_1"`.
    pub subscription_id: String,
    /// Granted buffer depth.
    pub buffer: u32,
}

/// `events.unsubscribe` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnsubscribeParams {
    /// The subscription to cancel.
    pub subscription_id: String,
}

/// `events.unsubscribe` result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnsubscribeResult {
    /// Always `true` on success.
    pub unsubscribed: bool,
}

/// The method name of the server notification.
pub const NOTIFY_METHOD: &str = "events.notify";

/// The `params` object of an `events.notify` server notification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotifyParams {
    /// The subscription this event was delivered on.
    pub subscription_id: String,
    /// The topic.
    pub topic: Topic,
    /// ISO 8601 delivery timestamp.
    pub timestamp: String,
    /// Topic-specific payload; see the typed payloads below.
    pub payload: Value,
}

/// The payload of `config.changed` (Linux addition, ADR 0033): the file
/// that was reloaded, whether it parsed, and the keys whose effective
/// value or source changed, so an editor refreshes the rows it shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigChangedPayload {
    /// The configuration file.
    pub path: String,
    /// True when the file parses; false when the daemon fell back to
    /// defaults (`config.validate` names why).
    pub ok: bool,
    /// The dotted keys whose value or source changed with this reload.
    pub keys: Vec<String>,
}

/// Live meeting state carried by `meeting.state` notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeetingLiveState {
    /// Actively recording.
    Recording,
    /// Recording finished, transcription in progress.
    Transcribing,
    /// Finished successfully.
    Completed,
    /// Finished with an error.
    Failed,
    /// Cancelled by the client.
    Cancelled,
    /// Linux addition: the stop was asked for; the takes are being closed.
    Stopping,
    /// Linux addition: the capture ended; transcription has not run.
    Stopped,
    /// Linux addition: a daemon restart interrupted the meeting; its audio
    /// is preserved and waits for recover or discard.
    Partial,
}

/// Payload of a `meeting.state` notification. The Linux fields
/// (`previous_state`, `reason`, `duration_ms`, `microphone_takes`,
/// `system_audio`) are registered in `docs/api/linux-deltas.md` and omitted
/// on the wire when absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingStatePayload {
    /// Always `"meeting"`.
    pub kind: String,
    /// Current state.
    pub state: MeetingLiveState,
    /// The meeting's id.
    pub meeting_id: Id,
    /// Live segment count so far.
    pub live_segment_count: u64,
    /// End timestamp in milliseconds of the last live segment.
    pub live_last_end_ms: u64,
    /// Whether the meeting is in its finalizing phase.
    pub is_finalizing: bool,
    /// Linux addition: the state left.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_state: Option<MeetingLiveState>,
    /// Linux addition: why, when it matters (`device lost`, `cancelled`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Linux addition: milliseconds captured so far.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Linux addition: microphone takes written so far.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub microphone_takes: Option<u32>,
    /// Linux addition: whether the system track records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_audio: Option<bool>,
    /// Linux addition (ADR 0035): where the diarization pass stands
    /// (`queued` on the `completed` transition that planned it, then
    /// `running`, `ready`, `failed`, `unavailable`); carried on the
    /// transitions the pass makes and omitted otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diarization_status: Option<crate::methods::speakers::DiarizationStatus>,
    /// Linux addition: where the analysis stands, on the transitions the
    /// analysis job makes (`queued` on the `completed` transition that
    /// planned it, then `running`, `ready`, `failed`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis_status: Option<crate::methods::meetings_notes::AnalysisStatus>,
}

/// Payload of the Linux `meeting.segment` topic: one live transcript
/// segment of the meeting that is recording (ADR 0031). Provisional
/// segments come as a tail per window numbered `<source>-p1`, `-p2`, ...:
/// `-p1` opens a fresh provisional tail for its source and drops the
/// earlier one, each later `-pN` extends it. A final segment is appended
/// for good, numbered in `segment_id`, and retires the provisional
/// segments it hardens out of (those starting before its end).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingSegmentPayload {
    /// The meeting.
    pub meeting_id: Id,
    /// `you` (the microphone) or `remote` (the system track).
    pub source: String,
    /// `you-12` for a final segment, `you-p1` for a provisional one.
    pub segment_id: String,
    /// True while a later window may still replace the segment.
    pub provisional: bool,
    /// Start on the meeting clock, in milliseconds.
    pub start_ms: u64,
    /// End on the meeting clock, in milliseconds.
    pub end_ms: u64,
    /// The text.
    pub text: String,
    /// The words when the engine aligned them; empty otherwise.
    #[serde(default)]
    pub words: Vec<crate::methods::meetings::Word>,
    /// Milliseconds of capture missing before this segment (a device
    /// switch, a window the engine skipped); absent when none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_before_ms: Option<u64>,
}

/// Payload of a `job.progress` notification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobProgressPayload {
    /// The job reporting progress.
    pub job_id: String,
    /// Fractional progress in `[0.0, 1.0]`.
    pub progress: f64,
    /// Linux addition (registered in `docs/api/linux-deltas.md`): chunks
    /// of a long-audio job finished so far; absent on other jobs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunks_done: Option<u32>,
    /// Linux addition: chunks the job has in total.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunks_total: Option<u32>,
    /// Linux addition: `decoding`, `transcribing`, `merging`, `done`,
    /// `failed` or `cancelled`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
}

/// Which capture the level describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelSource {
    /// The microphone (or the fixture standing in for it).
    Microphone,
    /// The system-audio track of a meeting.
    System,
}

/// Payload of the Linux `audio.level` topic: one meter sample.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioLevelPayload {
    /// Root mean square over the window, 0..1.
    pub rms: f32,
    /// Peak absolute sample over the window, 0..1.
    pub peak: f32,
    /// Which capture.
    pub source: LevelSource,
}

/// Payload of an `events.overflow` notification: the macOS server sends
/// the number of events dropped since the last delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverflowPayload {
    /// Events dropped from this subscription's buffer.
    pub dropped: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn topics_round_trip_in_dotted_form() {
        for topic in Topic::ALL {
            let json = serde_json::to_string(topic).unwrap();
            assert_eq!(json, format!("\"{}\"", topic.as_str()));
            let back: Topic = serde_json::from_str(&json).unwrap();
            assert_eq!(back, *topic);
        }
        assert!(Topic::AudioLevel.is_linux_addition());
        assert!(!Topic::JobProgress.is_linux_addition());
    }

    #[test]
    fn dictation_state_carries_the_completion_fields_only_when_present() {
        let plain: DictationStatePayload = serde_json::from_value(json!({
            "job_id": "job_dict_1", "state": "recording", "previous_state": "idle", "reason": null
        }))
        .unwrap();
        assert_eq!(plain.insertion, None);
        assert_eq!(plain.timings, None);
        let out = serde_json::to_value(&plain).unwrap();
        assert!(out.get("insertion").is_none() && out.get("first_words").is_none());
        assert!(out.get("timings").is_none());
        let done = json!({
            "job_id": "job_dict_1", "state": "idle", "previous_state": "inserting", "reason": null,
            "insertion": {
                "outcome": "inserted", "method": "paste", "backend": "mock",
                "target_app": {"bundle_id": "ghostty", "name": "ghostty"}, "reason": null
            },
            "first_words": "Add a regression test for the merger",
            "timings": {"capture_ms": 12, "transcribe_ms": 640, "insert_ms": 35}
        });
        let payload: DictationStatePayload = serde_json::from_value(done.clone()).unwrap();
        assert_eq!(payload.insertion.as_ref().unwrap().backend, "mock");
        assert_eq!(payload.timings.unwrap().transcribe_ms, 640);
        assert_eq!(serde_json::to_value(&payload).unwrap(), done);
    }

    #[test]
    fn unknown_topic_is_rejected_by_name() {
        let err = serde_json::from_str::<Topic>("\"bogus.topic\"").unwrap_err();
        assert!(err.to_string().contains("bogus.topic"));
    }

    #[test]
    fn notify_params_and_typed_payloads_round_trip() {
        let v = json!({
            "subscription_id": "sub_1",
            "topic": "meeting.state",
            "timestamp": "2026-02-13T17:00:00Z",
            "payload": {
                "kind": "meeting",
                "state": "recording",
                "meeting_id": "0f8fad5b-d9cb-469f-a165-70867728950e",
                "live_segment_count": 24,
                "live_last_end_ms": 91234,
                "is_finalizing": false
            }
        });
        let params: NotifyParams = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(params.topic, Topic::MeetingState);
        let payload: MeetingStatePayload = serde_json::from_value(params.payload.clone()).unwrap();
        assert_eq!(payload.state, MeetingLiveState::Recording);
        assert_eq!(serde_json::to_value(&params).unwrap(), v);
        let overflow: OverflowPayload = serde_json::from_value(json!({"dropped": 3})).unwrap();
        assert_eq!(overflow.dropped, 3);
    }
}

/// `model.download` (Linux addition): progress of one model download.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelDownloadPayload {
    /// The provider.
    pub provider: String,
    /// The model id.
    pub model: String,
    /// `running`, `done`, `cancelled`, `failed`, `quarantined`.
    pub state: String,
    /// Bytes on disk.
    pub bytes_done: u64,
    /// Bytes expected.
    pub bytes_total: u64,
    /// The failure reason, when any.
    pub error: Option<String>,
}

/// `dictation.state`: one session transition. The transition into `idle`
/// after `inserting` carries the Linux completion fields (`insertion` and
/// `first_words`) so the OSD shows the outcome without a second request;
/// every other transition leaves them absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DictationStatePayload {
    /// The session's job id.
    pub job_id: String,
    /// `recording`, `transcribing`, `inserting`, `idle`, `cancelled`, `failed`.
    pub state: String,
    /// The state left.
    pub previous_state: String,
    /// Why, when it matters; never transcript text.
    pub reason: Option<String>,
    /// How the transcript reached the desktop (Linux addition).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insertion: Option<DictationInsertion>,
    /// The first sentence of the transcript, bounded to one line (Linux
    /// addition); only on the completion transition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_words: Option<String>,
    /// Where the time after the key release went (Linux addition); only on
    /// the completion transition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timings: Option<DictationTimings>,
    /// The mode the session ran in (`raw`, `deterministic_polish`,
    /// `enhanced`); Linux addition, only on the completion transition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Why the Enhanced pass inserted the deterministic result instead of
    /// a rewrite (Linux addition); absent when a rewrite was inserted or
    /// the mode never asked for one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<crate::methods::polish::PolishNotice>,
    /// The hash of the Polish policy the session froze at its start
    /// (Linux addition), for log correlation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_hash: Option<String>,
}

/// The split of the release-to-inserted path carried by the completion
/// `dictation.state`, in milliseconds. The QA rig's `first_insert_timing`
/// scenario reads it for the NFR-1 measurement (`docs/qa.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DictationTimings {
    /// From the stop being observed to the take being final.
    pub capture_ms: u64,
    /// The engine's recognition; 0 for a silent take.
    pub transcribe_ms: u64,
    /// The inserter's call; 0 when there was nothing to insert.
    pub insert_ms: u64,
}

/// The insertion outcome carried by the completion `dictation.state`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DictationInsertion {
    /// `inserted`, `copied_to_clipboard` or `failed`.
    pub outcome: InsertionOutcome,
    /// `paste`, `fallback_copy` or `clipboard_only`.
    pub method: InsertionMethod,
    /// The backend that performed it (`clipboard`, `mock`, later
    /// `virtual_keyboard`, `portal`, `uinput`).
    pub backend: String,
    /// The focused application at the time of insertion.
    pub target_app: TargetApp,
    /// Why, on `failed` or `copied_to_clipboard`.
    pub reason: Option<String>,
}

/// `engine.state` (Linux addition): an engine process transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineStatePayload {
    /// The engine binary.
    pub binary: String,
    /// `spawned`, `loaded`, `unloaded`, `crashed`, `degraded`.
    pub state: String,
    /// The model, when loaded.
    pub model: Option<String>,
    /// The backend, when loaded.
    pub backend: Option<String>,
    /// The reason, when any.
    pub reason: Option<String>,
}
