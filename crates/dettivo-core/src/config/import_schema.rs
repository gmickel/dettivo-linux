//! The `[transcribe]` and `[transfer]` sections: the chunked long-audio
//! pipeline's values (ADR 0022) and the upload limit.

use serde::{Deserialize, Serialize};

/// `[transcribe]`: the chunked pipeline imports and re-runs go through.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Transcribe {
    /// Seconds of audio per chunk sent to the engine.
    pub chunk_seconds: u64,
    /// Seconds two consecutive chunks share; the merger reconciles them.
    pub overlap_seconds: u64,
    /// Seconds before a chunk's nominal end inside which the cut moves to
    /// the quietest point.
    pub safety_margin_seconds: u64,
    /// RMS (0 to 1) under which a chunk is treated as silence and never
    /// reaches the engine.
    pub silence_rms_floor: f64,
    /// Drop the known filler hallucinations before the text is stored.
    pub filler_filter: bool,
}

impl Default for Transcribe {
    fn default() -> Self {
        Self {
            chunk_seconds: 30,
            overlap_seconds: 2,
            safety_margin_seconds: 5,
            silence_rms_floor: 0.0065,
            filler_filter: true,
        }
    }
}

/// `[transfer]`: the chunked upload limits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Transfer {
    /// The largest upload `transfer.chunk` accumulates, in bytes.
    pub max_upload_bytes: u64,
}

impl Default for Transfer {
    fn default() -> Self {
        Self {
            max_upload_bytes: 1 << 30,
        }
    }
}
