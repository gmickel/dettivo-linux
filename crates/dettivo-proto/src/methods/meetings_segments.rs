//! `meetings.segments`, a Linux addition (`docs/api/linux-deltas.md`):
//! the transcript of a meeting so far. While a meeting records, stops
//! and finalises it answers the live segments (finals in the order they
//! became final, then the provisional tail); once the finalisation has
//! stored the settled row it answers the stored segments, the same ones
//! `meetings.get` carries. The `cursor` of one answer passed back as
//! `since` returns only the finals after it; a cursor from the other
//! transcript answers the whole transcript with `reset = true`.

use crate::id::Id;
use crate::methods::meetings::{Segment, SegmentSource, Word};
use crate::runtime::HistoryStatus;
use serde::{Deserialize, Serialize};

/// `meetings.segments` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SegmentsParams {
    /// The meeting.
    pub meeting_id: Id,
    /// The `cursor` of an earlier answer; absent reads from the start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
}

/// Which transcript an answer reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptSource {
    /// The live segments the capture worker holds, from the start of the
    /// recording until the finalisation stores the settled row.
    Live,
    /// The segments stored on the row.
    Stored,
}

impl TranscriptSource {
    /// The wire spelling, which is also the cursor's prefix.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Stored => "stored",
        }
    }
}

/// One segment of the answer: the contract segment's fields plus the
/// id, the side and the provisional flag the `meeting.segment` event
/// carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptSegment {
    /// `you-12` or `remote-3` for a live final, `you-p1` for a live
    /// provisional segment (the event's `segment_id`), `stored-<index>`
    /// for a stored one.
    pub segment_id: String,
    /// The position in the transcript: finals count from 0 in order, the
    /// provisional tail follows them.
    pub index: u32,
    /// `you` (the microphone, or the whole room when the meeting records
    /// room audio only) or `remote` (the system track).
    pub source: String,
    /// The contract's source: `microphone`, `system` or `merged`.
    pub source_type: SegmentSource,
    /// True while a later window may still replace the segment.
    pub provisional: bool,
    /// Start on the meeting clock, in milliseconds.
    pub start_ms: u64,
    /// End on the meeting clock, in milliseconds.
    pub end_ms: u64,
    /// The text.
    pub text: String,
    /// The speaker's name once the speaker pass assigned one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
    /// The speaker's stable id once assigned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<String>,
    /// The winning speaker's share of the segment once assigned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_confidence: Option<f64>,
    /// The words when the engine aligned them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<Word>,
    /// Milliseconds of capture missing before this segment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_before_ms: Option<u64>,
    /// The polished text once the meeting finalised.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polished_text: Option<String>,
}

impl TranscriptSegment {
    /// The side a contract source names (`merged` counts as `you`).
    pub fn side(source: SegmentSource) -> &'static str {
        match source {
            SegmentSource::System => "remote",
            SegmentSource::Microphone | SegmentSource::Merged => "you",
        }
    }

    /// A stored segment, every field as `meetings.get` carries it.
    pub fn stored(segment: &Segment) -> Self {
        Self {
            segment_id: format!("stored-{}", segment.index),
            index: segment.index,
            source: Self::side(segment.source_type).to_string(),
            source_type: segment.source_type,
            provisional: false,
            start_ms: segment.start_ms,
            end_ms: segment.end_ms,
            text: segment.text.clone(),
            speaker: segment.speaker.clone(),
            speaker_id: segment.speaker_id.clone(),
            speaker_confidence: segment.speaker_confidence,
            words: segment.words.clone(),
            gap_before_ms: segment.gap_before_ms,
            polished_text: segment.polished_text.clone(),
        }
    }
}

/// `meetings.segments` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SegmentsResult {
    /// The meeting.
    pub meeting_id: Id,
    /// Where the meeting stands.
    pub status: HistoryStatus,
    /// Which transcript this answer reads.
    pub transcript: TranscriptSource,
    /// Pass back as `since` to read only what is new: `live:<n>` or
    /// `stored:<n>`, `n` the number of finals the transcript holds.
    pub cursor: String,
    /// True when `since` belonged to the other transcript (or to none
    /// this meeting has): `segments` is then the whole transcript and
    /// replaces what the client holds.
    pub reset: bool,
    /// The finals after `since`, in order.
    pub segments: Vec<TranscriptSegment>,
    /// The provisional tail as it stands now; it replaces the previous
    /// tail whole. Empty on the stored transcript.
    pub provisional: Vec<TranscriptSegment>,
}

/// A parsed cursor: the transcript and the number of finals read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// The transcript the cursor belongs to.
    pub transcript: TranscriptSource,
    /// Finals read so far.
    pub finals: usize,
}

impl Cursor {
    /// `live:<n>` or `stored:<n>`; anything else names the shape.
    pub fn parse(text: &str) -> Result<Self, String> {
        let bad = || {
            format!(
                "since {text:?} is not a cursor; pass the cursor of an earlier answer (live:<n> or stored:<n>)"
            )
        };
        let (prefix, n) = text.split_once(':').ok_or_else(bad)?;
        let transcript = match prefix {
            "live" => TranscriptSource::Live,
            "stored" => TranscriptSource::Stored,
            _ => return Err(bad()),
        };
        let finals = n.parse().map_err(|_| bad())?;
        Ok(Self { transcript, finals })
    }

    /// The wire spelling.
    pub fn render(&self) -> String {
        format!("{}:{}", self.transcript.as_str(), self.finals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursors_round_trip_and_a_malformed_one_names_the_shape() {
        for text in ["live:0", "live:42", "stored:7"] {
            assert_eq!(Cursor::parse(text).unwrap().render(), text);
        }
        for bad in ["", "live", "live:-1", "later:3", "live:x"] {
            assert!(
                Cursor::parse(bad).unwrap_err().contains("live:<n>"),
                "{bad}"
            );
        }
    }
}
