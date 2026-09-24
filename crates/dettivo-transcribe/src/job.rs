//! The job: plan the chunks, run each through the engine (a crashed
//! engine is retried once), keep silent chunks and fillers out, merge,
//! and report after every chunk. Cancellation is checked between chunks;
//! a failure or a cancel hands back what the earlier chunks produced.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dettivo_engine_proto::Segment;
use dettivo_speech::{EngineError, RecognizeRequest, SttEngine};

use crate::chunker::{self, Chunk};
use crate::merger::{self, ChunkResult};
use crate::source::AudioSource;
use crate::{SAMPLE_RATE, Settings, Transcript, filters};

/// Longest wait for one chunk's recognition.
pub const CHUNK_TIMEOUT: Duration = Duration::from_secs(900);

/// Where a job stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The upload is being decoded.
    Decoding,
    /// Chunks are going through the engine.
    Transcribing,
    /// The overlaps are being reconciled.
    Merging,
    /// The item is stored.
    Done,
    /// The job failed; the item says why.
    Failed,
    /// The job was cancelled between chunks.
    Cancelled,
}

impl Stage {
    /// The wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Decoding => "decoding",
            Self::Transcribing => "transcribing",
            Self::Merging => "merging",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// One progress report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    /// The stage.
    pub stage: Stage,
    /// Chunks finished.
    pub chunks_done: u32,
    /// Chunks in total.
    pub chunks_total: u32,
}

impl Progress {
    /// `chunks_done / chunks_total`, 0 before the plan exists.
    pub fn fraction(&self) -> f64 {
        if self.chunks_total == 0 {
            0.0
        } else {
            f64::from(self.chunks_done) / f64::from(self.chunks_total)
        }
    }
}

/// What the engine is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Language code or `auto`.
    pub language: String,
    /// Vocabulary prompt.
    pub prompt: Option<String>,
    /// The audio is a system track (the other side of a meeting): the
    /// generic fillers Whisper writes over long stretches of it are
    /// dropped too.
    pub from_system: bool,
}

/// Why a job stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum JobError {
    /// The engine returns no timestamps; the provider is named.
    NoTimestamps {
        /// The provider.
        provider: String,
    },
    /// The chunk plan or the audio source failed.
    Audio(String),
    /// One chunk failed after its retry; earlier chunks are kept.
    Chunk {
        /// Zero-based index of the chunk.
        index: u32,
        /// Chunks in total.
        total: u32,
        /// The engine's reason, without any content.
        message: String,
        /// What the earlier chunks produced.
        partial: Box<Transcript>,
    },
    /// Cancelled between chunks.
    Cancelled {
        /// What the finished chunks produced.
        partial: Box<Transcript>,
    },
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoTimestamps { provider } => write!(
                f,
                "provider {provider} returns no timestamps (supports_timestamps is false); the chunked pipeline needs them"
            ),
            Self::Audio(m) => write!(f, "audio: {m}"),
            Self::Chunk {
                index,
                total,
                message,
                ..
            } => write!(f, "chunk {} of {total} failed: {message}", index + 1),
            Self::Cancelled { .. } => write!(f, "cancelled"),
        }
    }
}

/// Engine errors without any content.
pub fn redact(e: &EngineError) -> String {
    match e {
        EngineError::Crashed(_) => "engine crashed".into(),
        other => other
            .to_string()
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(120)
            .collect(),
    }
}

fn finish(results: &[ChunkResult], language: &str, silent: bool, duration_ms: u64) -> Transcript {
    let segments = merger::merge(results);
    let text = merger::text_of(&segments);
    Transcript {
        notice: (text.is_empty() && silent).then(|| "silent".to_string()),
        text,
        segments,
        language: language.to_string(),
        duration_ms,
    }
}

fn recognize_with_retry(
    engine: &dyn SttEngine,
    request: &Request,
    pcm: &[i16],
) -> Result<dettivo_engine_proto::RecognizeResult, EngineError> {
    let make = || RecognizeRequest {
        pcm: pcm.to_vec(),
        language: request.language.clone(),
        prompt: request.prompt.clone(),
        timestamps: true,
    };
    match engine.recognize(make(), CHUNK_TIMEOUT) {
        Err(EngineError::Crashed(_)) => {
            tracing::warn!("engine crashed on a chunk; retrying once");
            engine.recognize(make(), CHUNK_TIMEOUT)
        }
        other => other,
    }
}

/// Runs the pipeline over `source`.
pub fn run(
    source: &mut dyn AudioSource,
    engine: &dyn SttEngine,
    request: &Request,
    settings: &Settings,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Transcript, JobError> {
    if !engine.capabilities().supports_timestamps {
        return Err(JobError::NoTimestamps {
            provider: engine.provider().to_string(),
        });
    }
    let chunks: Vec<Chunk> = chunker::plan(source, settings).map_err(JobError::Audio)?;
    let total = chunks.len() as u32;
    let duration_ms = source.len() * 1000 / SAMPLE_RATE;
    let mut results: Vec<ChunkResult> = Vec::with_capacity(chunks.len());
    let mut language = request.language.clone();
    let mut all_silent = true;
    progress(Progress {
        stage: Stage::Transcribing,
        chunks_done: 0,
        chunks_total: total,
    });
    for chunk in &chunks {
        if cancel.load(Ordering::SeqCst) {
            return Err(JobError::Cancelled {
                partial: Box::new(finish(&results, &language, false, duration_ms)),
            });
        }
        let pcm = source
            .read(chunk.start, chunk.end)
            .map_err(JobError::Audio)?;
        let mut result = ChunkResult {
            start_ms: chunk.start_ms(),
            end_ms: chunk.end_ms(),
            segments: Vec::new(),
        };
        if let Some(speech) = filters::speech_bounds(&pcm, settings.silence_rms_floor) {
            result.start_ms = chunk.start_ms() + speech.start as u64 * 1000 / SAMPLE_RATE;
            result.end_ms = chunk.start_ms() + speech.end as u64 * 1000 / SAMPLE_RATE;
            all_silent = false;
            let recognized = recognize_with_retry(engine, request, &pcm[speech]).map_err(|e| {
                JobError::Chunk {
                    index: chunk.index,
                    total,
                    message: redact(&e),
                    partial: Box::new(finish(&results, &language, false, duration_ms)),
                }
            })?;
            if !recognized.text.trim().is_empty() && recognized.segments.is_empty() {
                return Err(JobError::NoTimestamps {
                    provider: engine.provider().to_string(),
                });
            }
            if !recognized.language.is_empty() && recognized.language != "auto" {
                language = recognized.language.clone();
            }
            result.segments = recognized
                .segments
                .into_iter()
                .filter(|s: &Segment| {
                    !settings.filler_filter
                        || !filters::should_drop(
                            &s.text,
                            s.end_ms.saturating_sub(s.start_ms),
                            request.from_system,
                        )
                })
                .collect();
        } else {
            tracing::info!(chunk = chunk.index, "near-silent chunk skipped");
        }
        results.push(result);
        progress(Progress {
            stage: Stage::Transcribing,
            chunks_done: chunk.index + 1,
            chunks_total: total,
        });
    }
    progress(Progress {
        stage: Stage::Merging,
        chunks_done: total,
        chunks_total: total,
    });
    let transcript = finish(&results, &language, all_silent, duration_ms);
    if cancel.load(Ordering::SeqCst) {
        Err(JobError::Cancelled {
            partial: Box::new(transcript),
        })
    } else {
        Ok(transcript)
    }
}
