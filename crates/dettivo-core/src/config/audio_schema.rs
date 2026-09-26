//! The `[audio]` and `[meetings]` sections: capture devices, the
//! two-stream meeting recorder, its post-meeting speaker pass
//! (`[meetings.diarization]`, ADR 0035) and the analysis that follows the
//! finalisation (`[meetings.analysis]`, ADR 0036).

use serde::{Deserialize, Serialize};

/// `[audio]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Audio {
    /// PipeWire node name to capture from; empty follows the default source.
    pub input_device: String,
    /// Meter sample interval in milliseconds.
    pub level_interval_ms: u64,
    /// A meeting's system track: `default_monitor` (the default sink's
    /// monitor, followed when the default changes) or a sink's node name
    /// (its monitor, pinned).
    pub system_source: String,
}

impl Default for Audio {
    fn default() -> Self {
        Self {
            input_device: String::new(),
            level_interval_ms: 50,
            system_source: "default_monitor".into(),
        }
    }
}

/// `[meetings]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Meetings {
    /// Keep both tracks after the meeting; off removes them at stop.
    pub keep_audio: bool,
    /// Seconds between two live checkpoints while recording.
    pub checkpoint_interval_seconds: u64,
    /// `keep` (audio, journal, sidecars and `metadata.json`), `audio_only`
    /// (audio, journal and sidecars) or `none` (the row alone once stopped).
    pub artifacts: String,
    /// Transcribe while the meeting records (ADR 0031); off runs the
    /// finalisation alone at stop.
    pub live: bool,
    /// The longest live window sent to the engine, in milliseconds.
    pub live_window_ms: u64,
    /// Milliseconds two consecutive live windows share.
    pub live_overlap_ms: u64,
    /// Milliseconds of new audio that cut the next live window.
    pub live_tick_ms: u64,
    /// A live segment this far behind the newest audio is final.
    pub boundary_merge_gap_ms: u64,
    /// The span around a remote segment inside which a microphone filler
    /// is its echo and is dropped.
    pub cross_source_padding_ms: u64,
    /// RMS (0 to 1) a live window must reach to be sent to the engine.
    pub speech_floor_rms: f64,
    /// The artifact policy `dettivo meetings delete` and the app use when
    /// none is named: `transcript_only`, `transcript_and_audio` or `all`.
    pub delete_artifact_policy: String,
    /// `[meetings.analysis]`: the analysis after finalisation (ADR 0036).
    pub analysis: MeetingAnalysis,
    /// `[meetings.diarization]`: the post-meeting speaker pass (ADR 0035).
    pub diarization: Diarization,
}

impl Default for Meetings {
    fn default() -> Self {
        Self {
            keep_audio: true,
            checkpoint_interval_seconds: 15,
            artifacts: "keep".into(),
            live: true,
            live_window_ms: 3000,
            live_overlap_ms: 450,
            live_tick_ms: 900,
            boundary_merge_gap_ms: 1200,
            cross_source_padding_ms: 800,
            speech_floor_rms: 0.0065,
            delete_artifact_policy: "all".into(),
            analysis: MeetingAnalysis::default(),
            diarization: Diarization::default(),
        }
    }
}

/// `[meetings.analysis]`: the summary, decisions and action items a
/// meeting gets from the language model once it finalised (ADR 0036).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MeetingAnalysis {
    /// Run the analysis after every finalisation; off leaves it to
    /// `meetings.analyze`.
    pub auto: bool,
    /// The whole analysis must finish within this; then it fails with
    /// `provider_unavailable` and can be run again.
    pub timeout_ms: u64,
    /// Characters of transcript per model call; a longer transcript is
    /// analysed in parts and the parts merged in one final call.
    pub chunk_chars: u64,
    /// The provider the analysis uses (`local`, `ollama`,
    /// `openai_compatible`, `auto`); empty follows `[llm] provider`.
    pub provider: String,
}

impl Default for MeetingAnalysis {
    fn default() -> Self {
        Self {
            auto: true,
            timeout_ms: 60_000,
            chunk_chars: 12_000,
            provider: String::new(),
        }
    }
}

/// `[meetings.diarization]`: the post-meeting speaker pass over the
/// system track (ADR 0035) and the sentence rule that labels a segment
/// (ADR 0072).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Diarization {
    /// The pass is available at all (`meetings.diarize` and the auto run).
    pub enabled: bool,
    /// Run the pass by itself when a finalisation completes.
    pub auto: bool,
    /// The catalogue model set under `<models>/diarize/`.
    pub model: String,
    /// A pause between aligned words at least this long ends a sentence
    /// unit where the diarized speaker differs across it.
    pub pause_ms: u64,
    /// A unit no speaker turn overlaps takes the nearest turn within this.
    pub nearest_turn_ms: u64,
    /// The least share of a unit's diarized speech its winner must hold;
    /// 0 labels every unit a turn overlaps.
    pub min_speaker_share: f64,
    /// Retired by ADR 0072: read and ignored, so a file that still sets
    /// it loads; never written or listed.
    #[serde(skip_serializing)]
    pub min_coverage: serde::de::IgnoredAny,
    /// The speaker count the clustering is told; 0 lets it decide.
    pub max_speakers: u32,
    /// The clustering distance threshold when the count is not fixed.
    pub clustering_threshold: f64,
}

impl Default for Diarization {
    fn default() -> Self {
        Self {
            enabled: true,
            auto: true,
            model: "diarization-en".into(),
            pause_ms: 250,
            nearest_turn_ms: 10_000,
            min_speaker_share: 0.0,
            min_coverage: serde::de::IgnoredAny,
            max_speakers: 0,
            clustering_threshold: 0.6,
        }
    }
}
