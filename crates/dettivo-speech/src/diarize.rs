//! The diarization engines over the supervisor (ADR 0035, ADR 0073): the
//! sherpa-onnx engine or the Nemotron engine, chosen by the model set, one
//! `diarize` per pass with the speaker count when known, chunk progress relayed
//! from the engine's `progress` events, a cancel flag the caller may
//! raise from another thread (the engine answers `cancelled` and the pass
//! ends in its background), and the same slot mechanics as the speech
//! engines: spawn on first need, `stt_idle_seconds` reaping, crashes
//! recorded with backoff, degraded after three. `embed` asks the same
//! engine for one voice embedding per time span of a track; only the
//! sherpa-onnx engine has an embedding model (Nemotron refuses it).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use dettivo_engine_proto::{
    BackendPreference, DiarizeParams, DiarizeResult, EmbedParams, EmbedResult, LoadParams, Span,
};
use serde_json::Value;

use crate::EngineError;
use crate::engines::DIARIZE_BINARY;
use crate::supervisor::{EngineStatus, Supervisor};

/// The Nemotron diarization engine binary (ADR 0073).
pub const NEMOTRON_BINARY: &str = "dettivo-engine-nemotron";
/// The catalogue id of NVIDIA's Nemotron 3 Diarization under `diarize`.
pub const NEMOTRON_MODEL: &str = "nemotron-3-diarization";
/// The most speakers Nemotron 3 Diarization tracks.
pub const NEMOTRON_MAX_SPEAKERS: u32 = 8;
/// Spans per `embed` request: at 192 values each, a batch answers in a few
/// hundred kilobytes, far under the 1 MiB frame header.
const EMBED_BATCH: usize = 128;
/// The sherpa-onnx model set a Nemotron pass falls back to.
pub const FALLBACK_MODEL: &str = "diarization-en";

/// The engine binary that loads the catalogue model set `model`.
pub fn binary_for(model: &str) -> &'static str {
    if model == NEMOTRON_MODEL {
        NEMOTRON_BINARY
    } else {
        DIARIZE_BINARY
    }
}

/// One pass.
#[derive(Debug, Clone, PartialEq)]
pub struct DiarizeRequest {
    /// 16 kHz mono samples of the whole track.
    pub pcm: Vec<i16>,
    /// The speaker count when known; the clustering decides otherwise.
    pub speakers: Option<u32>,
    /// The clustering distance threshold without a count.
    pub clustering_threshold: Option<f64>,
}

/// A diarization engine binary for one model directory.
pub struct DiarizeEngine {
    supervisor: Arc<Supervisor>,
    binary: &'static str,
    model_dir: String,
    threads: Option<u32>,
    backend: BackendPreference,
}

impl DiarizeEngine {
    /// `binary` over `supervisor` for the model set under `model_dir`,
    /// loading with `threads` per model (`None` is the engine's default).
    pub fn new(
        supervisor: Arc<Supervisor>,
        binary: &'static str,
        model_dir: String,
        threads: Option<u32>,
    ) -> Self {
        Self {
            supervisor,
            binary,
            model_dir,
            threads,
            backend: BackendPreference::Auto,
        }
    }

    /// Selects the configured diarization provider for each load.
    pub fn with_backend(mut self, backend: BackendPreference) -> Self {
        self.backend = backend;
        self
    }

    /// The model directory this engine loads.
    pub fn model_dir(&self) -> &str {
        &self.model_dir
    }

    /// The engine binary.
    pub fn binary(&self) -> &'static str {
        self.binary
    }

    fn load_params(&self) -> LoadParams {
        LoadParams {
            model: self.model_dir.clone(),
            backend_preference: self.backend,
            vad_model: None,
            context_length: None,
            lora: None,
            threads: self.threads.filter(|t| *t > 0),
        }
    }

    /// Runs one pass within `timeout`; `progress` hears `(completed,
    /// total)` chunks, and `cancel` raised from another thread ends the
    /// pass with `EngineError::Cancelled`.
    pub fn diarize(
        &self,
        request: &DiarizeRequest,
        timeout: Duration,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u32, u32),
    ) -> Result<DiarizeResult, EngineError> {
        dettivo_engine_proto::validate_pcm_samples(request.pcm.len() as u64).map_err(|e| {
            EngineError::Engine {
                code: "bad_request".into(),
                message: e.to_string(),
            }
        })?;
        let params = DiarizeParams {
            speakers: request.speakers,
            clustering_threshold: request.clustering_threshold,
            frame_probabilities: false,
        };
        let pcm = dettivo_engine_proto::pcm_to_bytes(&request.pcm);
        self.supervisor
            .with_engine_load(self.binary, self.load_params(), |process, _| {
                let value = process.call_cancellable(
                    "diarize",
                    serde_json::to_value(&params).unwrap_or(Value::Null),
                    &[&pcm],
                    timeout,
                    cancel,
                    |event| {
                        if event.name == "progress" {
                            let done = event.payload["completed"].as_u64().unwrap_or(0) as u32;
                            let total = event.payload["total"].as_u64().unwrap_or(0) as u32;
                            progress(done, total);
                        }
                    },
                )?;
                serde_json::from_value(value)
                    .map_err(|e| EngineError::Transport(format!("diarize response: {e}")))
            })
    }

    /// One embedding per span of `pcm` (16 kHz mono), within `timeout`
    /// per request. The spans go in batches, each with only the audio it
    /// covers: one reply for a long meeting's spans outgrew the protocol's
    /// header limit (`MAX_HEADER_BYTES`), and the engine could not send it.
    pub fn embed(
        &self,
        pcm: &[i16],
        spans: &[(u64, u64)],
        timeout: Duration,
    ) -> Result<Vec<Option<Vec<f32>>>, EngineError> {
        dettivo_engine_proto::validate_pcm_samples(pcm.len() as u64).map_err(|e| {
            EngineError::Engine {
                code: "bad_request".into(),
                message: e.to_string(),
            }
        })?;
        let mut out = Vec::with_capacity(spans.len());
        for batch in spans.chunks(EMBED_BATCH) {
            out.extend(self.embed_batch(pcm, batch, timeout)?);
        }
        Ok(out)
    }

    fn embed_batch(
        &self,
        pcm: &[i16],
        spans: &[(u64, u64)],
        timeout: Duration,
    ) -> Result<Vec<Option<Vec<f32>>>, EngineError> {
        let from_ms = spans.iter().map(|s| s.0).min().unwrap_or(0);
        let to_ms = spans.iter().map(|s| s.1).max().unwrap_or(0);
        let from = usize::try_from(from_ms * 16)
            .unwrap_or(usize::MAX)
            .min(pcm.len());
        let to = usize::try_from(to_ms * 16)
            .unwrap_or(usize::MAX)
            .clamp(from, pcm.len());
        let params = EmbedParams {
            spans: spans
                .iter()
                .map(|&(start_ms, end_ms)| Span {
                    start_ms: start_ms.saturating_sub(from_ms),
                    end_ms: end_ms.saturating_sub(from_ms),
                })
                .collect(),
        };
        let bytes = dettivo_engine_proto::pcm_to_bytes(&pcm[from..to]);
        self.supervisor
            .with_engine_load(self.binary, self.load_params(), |process, _| {
                let value = process.call(
                    "embed",
                    serde_json::to_value(&params).unwrap_or(Value::Null),
                    &[&bytes],
                    timeout,
                    |_| {},
                )?;
                let result = serde_json::from_value::<EmbedResult>(value)
                    .map_err(|e| EngineError::Transport(format!("embed response: {e}")))?;
                if result.embeddings.len() != spans.len() {
                    return Err(EngineError::Transport(format!(
                        "embed response: {} vectors for {} spans",
                        result.embeddings.len(),
                        spans.len()
                    )));
                }
                Ok(result.embeddings)
            })
    }

    /// The supervisor's row for the engine (found, running, crashes).
    pub fn status(&self) -> EngineStatus {
        self.supervisor
            .status(&[self.binary])
            .into_iter()
            .next()
            .expect("one row per binary asked for")
    }
}

/// How long a pass over `audio_ms` of audio may take before it counts
/// as hung: a minute plus twice the audio's length (the CPU pass runs
/// several times faster than real time, ADR 0035).
pub fn timeout_for(audio_ms: u64) -> Duration {
    Duration::from_secs(60) + Duration::from_millis(audio_ms.saturating_mul(2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_load_names_the_directory_the_provider_and_the_threads() {
        let supervisor = Supervisor::new(Default::default());
        let engine = DiarizeEngine::new(
            supervisor,
            DIARIZE_BINARY,
            "/models/diarize/diarization".into(),
            Some(0),
        );
        let load = engine.load_params();
        assert_eq!(load.model, "/models/diarize/diarization");
        assert_eq!(load.backend_preference, BackendPreference::Auto);
        assert_eq!(load.threads, None, "0 is the engine's default");
        assert_eq!(engine.model_dir(), "/models/diarize/diarization");
        assert_eq!(timeout_for(60_000), Duration::from_secs(180));
        assert_eq!(engine.status().binary, DIARIZE_BINARY);
        let pinned = engine.with_backend(BackendPreference::Cuda);
        assert_eq!(
            pinned.load_params().backend_preference,
            BackendPreference::Cuda
        );
        assert_eq!(binary_for(NEMOTRON_MODEL), NEMOTRON_BINARY);
        assert_eq!(binary_for(FALLBACK_MODEL), DIARIZE_BINARY);
    }
}
