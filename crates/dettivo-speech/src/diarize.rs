//! The diarization engine over the supervisor (ADR 0035): one `diarize`
//! per pass with the speaker count when known, chunk progress relayed
//! from the engine's `progress` events, a cancel flag the caller may
//! raise from another thread (the engine answers `cancelled` and the pass
//! ends in its background), and the same slot mechanics as the speech
//! engines: spawn on first need, `stt_idle_seconds` reaping, crashes
//! recorded with backoff, degraded after three.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use dettivo_engine_proto::{BackendPreference, DiarizeParams, DiarizeResult, LoadParams};
use serde_json::Value;

use crate::EngineError;
use crate::engines::DIARIZE_BINARY;
use crate::supervisor::{EngineStatus, Supervisor};

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

/// The diarization engine for one model directory.
pub struct DiarizeEngine {
    supervisor: Arc<Supervisor>,
    model_dir: String,
    threads: Option<u32>,
    backend: BackendPreference,
}

impl DiarizeEngine {
    /// An engine over `supervisor` for the model set under `model_dir`,
    /// loading with `threads` per model (`None` is the engine's default).
    pub fn new(supervisor: Arc<Supervisor>, model_dir: String, threads: Option<u32>) -> Self {
        Self {
            supervisor,
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
        };
        let pcm = dettivo_engine_proto::pcm_to_bytes(&request.pcm);
        self.supervisor
            .with_engine_load(DIARIZE_BINARY, self.load_params(), |process, _| {
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

    /// The supervisor's row for the engine (found, running, crashes).
    pub fn status(&self) -> EngineStatus {
        self.supervisor
            .status(&[DIARIZE_BINARY])
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
        let engine = DiarizeEngine::new(supervisor, "/models/diarize/diarization".into(), Some(0));
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
    }
}
