//! The chunked long-audio pipeline (ADR 0022): audio of any length is cut
//! into chunks at the macOS values (`chunker`), each chunk is transcribed
//! through a timestamp-capable engine, the overlaps are reconciled by
//! timestamp (`merger`), the known filler hallucinations and near-silent
//! chunks are kept out of the text (`filters`), and the whole run is one
//! cancellable job reporting per chunk (`job`). Imports, re-runs and a
//! meeting's finalisation run it; the meeting's live path (`live`, ADR
//! 0031) cuts each source into windows and folds the results through the
//! same merger, and `interleave` puts the two sources on one timeline.

pub mod chunker;
pub mod filters;
pub mod interleave;
pub mod job;
pub mod live;
pub mod merger;
pub mod source;

pub use dettivo_engine_proto::{Segment, Word};
pub use job::{JobError, Progress, Request, Stage, run};
pub use source::{AudioSource, WavSource};

/// Package name of this crate, as declared in its manifest.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Package names of the workspace crates this crate builds on.
pub const UPSTREAM: &[&str] = &[
    dettivo_proto::CRATE_NAME,
    dettivo_engine_proto::CRATE_NAME,
    dettivo_speech::CRATE_NAME,
];

/// The sample rate every source delivers.
pub const SAMPLE_RATE: u64 = 16_000;

/// The pipeline's values (`[transcribe]`); 30-second windows allow language
/// changes; overlap, quiet cuts and activation retain the existing defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Seconds of audio per chunk.
    pub chunk_seconds: u64,
    /// Seconds two consecutive chunks share.
    pub overlap_seconds: u64,
    /// Seconds before a chunk's nominal end inside which the cut moves to
    /// the quietest point.
    pub safety_margin_seconds: u64,
    /// RMS (0 to 1) under which a chunk never reaches the engine.
    pub silence_rms_floor: f64,
    /// Drop the known filler hallucinations.
    pub filler_filter: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            chunk_seconds: 30,
            overlap_seconds: 2,
            safety_margin_seconds: 5,
            silence_rms_floor: filters::SILENCE_RMS_FLOOR,
            filler_filter: true,
        }
    }
}

/// What a job produces.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Transcript {
    /// The merged text.
    pub text: String,
    /// The merged segments on absolute time, words included when the
    /// engine aligned them.
    pub segments: Vec<Segment>,
    /// The language the engine reported.
    pub language: String,
    /// Why an empty text is empty (`silent`).
    pub notice: Option<String>,
    /// The audio length in milliseconds.
    pub duration_ms: u64,
}

impl Transcript {
    /// The contract's segment shape for `transcripts.get`: indexed, with
    /// the microphone source and the words when there are any.
    pub fn contract_segments(&self) -> Vec<dettivo_proto::methods::meetings::Segment> {
        use dettivo_proto::methods::meetings::{Segment as Out, SegmentSource, Word as OutWord};
        self.segments
            .iter()
            .enumerate()
            .map(|(i, s)| Out {
                index: i as u32,
                start_ms: s.start_ms,
                end_ms: s.end_ms,
                text: s.text.clone(),
                speaker: None,
                source_type: SegmentSource::Microphone,
                words: s
                    .words
                    .iter()
                    .map(|w| OutWord {
                        start_ms: w.start_ms,
                        end_ms: w.end_ms,
                        text: w.text.clone(),
                        confidence: w.confidence,
                    })
                    .collect(),
                gap_before_ms: None,
                speaker_id: None,
                speaker_confidence: None,
                polished_text: None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "dettivo-transcribe");
    }

    #[test]
    fn upstream_edges_resolve_and_defaults_bound_language_detection() {
        assert_eq!(
            super::UPSTREAM,
            &["dettivo-proto", "dettivo-engine-proto", "dettivo-speech"]
        );
        let s = super::Settings::default();
        assert_eq!(
            (s.chunk_seconds, s.overlap_seconds, s.safety_margin_seconds),
            (30, 2, 5)
        );
        assert!((s.silence_rms_floor - 0.0065).abs() < 1e-9);
    }
}

#[cfg(test)]
mod dependency_tests {
    #[test]
    fn transcription_does_not_depend_on_language_processing() {
        assert!(!super::UPSTREAM.contains(&"dettivo-language"));
        assert!(!include_str!("../Cargo.toml").contains("dettivo-language.workspace"));
    }
}
