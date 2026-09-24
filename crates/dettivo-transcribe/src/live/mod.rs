//! The live path of a meeting (ADR 0030): each captured source is cut
//! into windows at the macOS live tuning (`windower`), every window's
//! engine result is folded into that source's segments as provisional
//! text that hardens into final segments once the boundary merge gap has
//! passed (`merger`), and the two sources meet on one absolute timeline
//! in `crate::interleave`. The values are the macOS
//! `LiveMeetingTranscriptionConfig` ones.

pub mod merger;
pub mod windower;

pub use merger::{Merger, Update};
pub use windower::{Cut, Window, Windower};

use dettivo_engine_proto::Word;
use dettivo_proto::methods::meetings::{
    Segment as ContractSegment, SegmentSource, Word as OutWord,
};
use serde::{Deserialize, Serialize};

/// Which side of the call a segment came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// The microphone: the person at this machine.
    You,
    /// The system track: the other side of the call.
    Remote,
}

impl Source {
    /// The wire spelling (`you`, `remote`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::You => "you",
            Self::Remote => "remote",
        }
    }

    /// The contract's `source_type` for the source.
    pub fn contract(self) -> SegmentSource {
        match self {
            Self::You => SegmentSource::Microphone,
            Self::Remote => SegmentSource::System,
        }
    }

    /// The source a contract `source_type` names (`merged` counts as the
    /// microphone).
    pub fn from_contract(source: SegmentSource) -> Self {
        match source {
            SegmentSource::System => Self::Remote,
            SegmentSource::Microphone | SegmentSource::Merged => Self::You,
        }
    }
}

/// The live tuning (`[meetings]`); the defaults are the macOS values.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveSettings {
    /// The longest window sent to the engine, in milliseconds.
    pub window_ms: u64,
    /// Milliseconds two consecutive windows share.
    pub overlap_ms: u64,
    /// Milliseconds of new audio that cut the next window.
    pub tick_ms: u64,
    /// A segment this far behind the newest audio is final.
    pub boundary_merge_gap_ms: u64,
    /// The span around a remote segment inside which a microphone filler
    /// is its echo.
    pub cross_source_padding_ms: u64,
    /// RMS (0 to 1) a window must reach to be sent while nobody spoke.
    pub speech_floor_rms: f64,
}

impl Default for LiveSettings {
    fn default() -> Self {
        Self {
            window_ms: 3000,
            overlap_ms: 450,
            tick_ms: 900,
            boundary_merge_gap_ms: 1200,
            cross_source_padding_ms: 800,
            speech_floor_rms: crate::filters::SILENCE_RMS_FLOOR,
        }
    }
}

impl LiveSettings {
    /// The floor a window must reach while the previous one was speech:
    /// the macOS `speechContinuationFloor` (0.0045) relative to its
    /// activation floor (0.0065).
    pub fn continuation_floor_rms(&self) -> f64 {
        self.speech_floor_rms * (0.0045 / 0.0065)
    }

    /// The settings must cut at least one tick per window and keep the
    /// overlap inside it; the error names the keys.
    pub fn validate(&self) -> Result<(), String> {
        if self.tick_ms == 0 || self.window_ms == 0 {
            return Err("meetings.live_tick_ms and live_window_ms must be positive".into());
        }
        if self.overlap_ms + self.tick_ms > self.window_ms {
            return Err(format!(
                "meetings.live_window_ms ({}) must hold live_overlap_ms ({}) plus live_tick_ms ({})",
                self.window_ms, self.overlap_ms, self.tick_ms
            ));
        }
        Ok(())
    }
}

/// One segment on the meeting's clock with its source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveSegment {
    /// Which side.
    pub source: Source,
    /// `you-12` for a final, `you-p1` for a provisional.
    pub id: String,
    /// True while a later window may still replace it.
    pub provisional: bool,
    /// Start on the meeting clock.
    pub start_ms: u64,
    /// End on the meeting clock.
    pub end_ms: u64,
    /// The text.
    pub text: String,
    /// The words when the engine aligned them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<Word>,
    /// Milliseconds of capture missing before this segment (a device
    /// switch, a skipped window).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_before_ms: Option<u64>,
}

impl LiveSegment {
    /// The contract's shape at `index`.
    pub fn contract(&self, index: u32) -> ContractSegment {
        ContractSegment {
            index,
            start_ms: self.start_ms,
            end_ms: self.end_ms,
            text: self.text.clone(),
            speaker: None,
            source_type: self.source.contract(),
            words: self
                .words
                .iter()
                .map(|w| OutWord {
                    start_ms: w.start_ms,
                    end_ms: w.end_ms,
                    text: w.text.clone(),
                    confidence: w.confidence,
                })
                .collect(),
            gap_before_ms: self.gap_before_ms,
            speaker_id: None,
            speaker_confidence: None,
            polished_text: None,
        }
    }

    /// A final segment back from the contract's shape (a checkpoint tail).
    pub fn from_contract(segment: &ContractSegment) -> Self {
        Self {
            source: Source::from_contract(segment.source_type),
            id: format!(
                "{}-{}",
                Source::from_contract(segment.source_type).as_str(),
                segment.index
            ),
            provisional: false,
            start_ms: segment.start_ms,
            end_ms: segment.end_ms,
            text: segment.text.clone(),
            words: segment
                .words
                .iter()
                .map(|w| Word {
                    start_ms: w.start_ms,
                    end_ms: w.end_ms,
                    text: w.text.clone(),
                    confidence: w.confidence,
                })
                .collect(),
            gap_before_ms: segment.gap_before_ms,
        }
    }
}

/// The contract's segments of a list, indexed in order.
pub fn contract_segments(segments: &[LiveSegment]) -> Vec<ContractSegment> {
    segments
        .iter()
        .enumerate()
        .map(|(i, s)| s.contract(i as u32))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_macos_values_and_the_contract_shape_round_trips() {
        let s = LiveSettings::default();
        assert_eq!(
            (
                s.window_ms,
                s.overlap_ms,
                s.tick_ms,
                s.boundary_merge_gap_ms,
                s.cross_source_padding_ms
            ),
            (3000, 450, 900, 1200, 800)
        );
        assert!((s.speech_floor_rms - 0.0065).abs() < 1e-9);
        assert!((s.continuation_floor_rms() - 0.0045).abs() < 1e-9);
        assert!(s.validate().is_ok());
        let bad = LiveSettings {
            window_ms: 1000,
            ..s
        };
        assert!(
            bad.validate()
                .unwrap_err()
                .contains("live_window_ms (1000)")
        );
        let seg = LiveSegment {
            source: Source::Remote,
            id: "remote-3".into(),
            provisional: false,
            start_ms: 100,
            end_ms: 900,
            text: "hello".into(),
            words: Vec::new(),
            gap_before_ms: Some(40),
        };
        let out = seg.contract(3);
        assert_eq!(out.source_type, SegmentSource::System);
        assert_eq!(out.gap_before_ms, Some(40));
        assert_eq!(LiveSegment::from_contract(&out), seg);
        assert_eq!(Source::from_contract(SegmentSource::Merged), Source::You);
        assert_eq!(Source::You.as_str(), "you");
    }
}
