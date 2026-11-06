//! The speech check's payloads: `detect_speech` judges a dictation take
//! with the Silero voice model, and `feed_speech` hands it the take while
//! it is captured (ADR 0097). Re-exported from `messages`.

use serde::{Deserialize, Serialize};

/// `detect_speech` (with one `pcm16k` attachment): the Silero voice
/// activity pass a dictation take goes through before a recognizer hears it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectSpeechParams {
    /// The probability from which a frame is speech.
    pub threshold: f64,
    /// The shortest stretch that counts as speech, in milliseconds.
    pub min_speech_ms: u32,
    /// The attachment ends a take fed so far by `feed_speech`, which must
    /// hold exactly this many samples; the whole take is judged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continues: Option<u64>,
}

/// `feed_speech` (with one `pcm16k` attachment): the next samples of a take
/// judged as it is captured (ADR 0097); `start` begins a new take.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedSpeechParams {
    /// The samples are the first of a new take.
    pub start: bool,
    /// Asks where the take's last voiced window ends: the answer's
    /// `voiced_until` counts windows whose speech probability reached this
    /// (ADR 0093).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
}

/// `feed_speech` result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedSpeechResult {
    /// The samples of the take fed so far.
    pub samples: u64,
    /// With `threshold`: the end, in samples from the take's start, of the
    /// last window run whose speech probability reached it; 0 when none did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voiced_until: Option<u64>,
}

/// `detect_speech` result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectSpeechResult {
    /// Milliseconds of speech found; 0 means none.
    pub speech_ms: u64,
}
