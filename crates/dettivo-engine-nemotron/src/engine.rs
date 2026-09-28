//! The engine: NVIDIA Nemotron 3 Diarization through NeMo-Speech.cpp,
//! loaded from its catalogue directory (or the GGUF itself), one pass at a
//! time over 16 kHz samples, with the recorder post-processing fn-64
//! measured turning the probabilities into turns. The model tracks at
//! most eight speakers; a larger expected count is refused so the daemon
//! can run the sherpa-onnx engine instead. It has no speaker-embedding
//! model, so an `embed` is refused; the daemon asks the sherpa-onnx set
//! for the voiceprint's embeddings (ADR 0075).

use std::path::{Path, PathBuf};

use crate::backend::{self, Selection};
use crate::model::{self, Model};
use crate::turns;
use dettivo_engine_proto::{
    Backend, DiarizeEngine, DiarizeParams, DiarizeResult, EmbedParams, EmbedResult, EngineError,
    FrameProbabilities, LoadParams, LoadedResult,
};

/// The model file inside its catalogue directory.
pub const MODEL_FILE: &str = "Nemotron-3-Diarization.q8_0.gguf";

/// A loaded model.
pub struct Nemotron {
    model: Model,
    path: PathBuf,
    selection: Selection,
}

/// The GGUF a load names: the file itself, or [`MODEL_FILE`] inside a
/// directory.
pub fn model_file(path: &Path) -> Result<PathBuf, EngineError> {
    let file = if path.is_dir() {
        path.join(MODEL_FILE)
    } else {
        path.to_path_buf()
    };
    if file.is_file() {
        Ok(file)
    } else {
        Err(EngineError::new(
            "model_missing",
            format!("model file not found: {}", file.display()),
        ))
    }
}

/// Converts 16-bit samples to the floats the library reads.
pub fn to_f32(pcm: &[i16]) -> Vec<f32> {
    pcm.iter().map(|s| f32::from(*s) / 32_768.0).collect()
}

impl Nemotron {
    /// The frame probabilities and the turns of one pass.
    pub fn run(
        &self,
        pcm: &[i16],
        speakers: Option<u32>,
        progress: &mut dyn FnMut(u32, u32),
    ) -> Result<(Vec<f32>, Vec<dettivo_engine_proto::SpeakerTurn>), EngineError> {
        if pcm.is_empty() {
            return Err(EngineError::new("bad_request", "no audio to diarize"));
        }
        let capacity = self.model.speakers();
        let speakers = speakers.filter(|n| *n > 0);
        if let Some(n) = speakers.filter(|n| *n > capacity) {
            return Err(EngineError::new(
                "bad_request",
                format!("Nemotron 3 Diarization tracks at most {capacity} speakers; {n} expected"),
            ));
        }
        let probs = self
            .model
            .probabilities(&to_f32(pcm), progress)
            .map_err(|why| EngineError::new("internal", why))?;
        let turns = turns::turns(
            &probs,
            capacity as usize,
            u64::from(self.model.frame_ms()),
            speakers,
        );
        tracing::info!(turns = turns.len(), "diarization done");
        Ok((probs, turns))
    }
}

impl DiarizeEngine for Nemotron {
    fn load(params: &LoadParams, force_cpu: bool) -> Result<Self, EngineError> {
        let path = model_file(Path::new(&params.model))?;
        let (model, mut selection) = backend::load(
            params.backend_preference,
            force_cpu,
            backend::BUILT,
            |gpu| Model::open(&path, gpu),
        )?;
        if selection.backend != Backend::Cpu
            && let Some(name) = model::gpu_name(0)
        {
            selection.reason = format!("{} ({name})", selection.reason);
        }
        tracing::info!(
            model = %path.display(),
            speakers = model.speakers(),
            "model loaded backend={:?} reason={:?}", selection.backend, selection.reason
        );
        Ok(Self {
            model,
            path,
            selection,
        })
    }

    fn loaded(&self) -> LoadedResult {
        LoadedResult {
            model: self.path.to_string_lossy().into_owned(),
            backend: self.selection.backend,
            reason: self.selection.reason.clone(),
            fallback_reason: self.selection.fallback_reason.clone(),
        }
    }

    fn diarize(
        &self,
        pcm: &[i16],
        params: &DiarizeParams,
        progress: &mut dyn FnMut(u32, u32),
    ) -> Result<DiarizeResult, EngineError> {
        let (probs, turns) = self.run(pcm, params.speakers, progress)?;
        if !params.frame_probabilities {
            return Ok(DiarizeResult {
                turns,
                ..Default::default()
            });
        }
        let speakers = self.model.speakers();
        Ok(DiarizeResult {
            turns,
            frames: Some(FrameProbabilities {
                count: (probs.len() / speakers.max(1) as usize) as u64,
                speakers,
                frame_ms: self.model.frame_ms(),
            }),
            probabilities: probs.iter().flat_map(|p| p.to_le_bytes()).collect(),
        })
    }

    fn embed(&self, _pcm: &[i16], _params: &EmbedParams) -> Result<EmbedResult, EngineError> {
        Err(embed_refused())
    }
}

/// The refusal of an `embed`: the model has no speaker-embedding model.
fn embed_refused() -> EngineError {
    EngineError::new(
        "bad_request",
        "Nemotron 3 Diarization has no speaker-embedding model; \
         embeddings come from the sherpa-onnx diarization-en set",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_model_names_its_path_and_samples_convert() {
        let dir = tempfile::tempdir().unwrap();
        let err = model_file(dir.path()).err().unwrap();
        assert_eq!(err.code, "model_missing");
        assert!(err.message.contains(MODEL_FILE), "{err}");
        let file = dir.path().join("other.gguf");
        std::fs::write(&file, b"x").unwrap();
        assert_eq!(model_file(&file).unwrap(), file);
        assert_eq!(to_f32(&[0, 16_384, -32_768]), [0.0, 0.5, -1.0]);
    }

    #[test]
    fn embed_is_refused_as_a_bad_request() {
        let err = embed_refused();
        assert_eq!(err.code, "bad_request");
        assert!(err.message.contains("no speaker-embedding model"), "{err}");
    }
}
