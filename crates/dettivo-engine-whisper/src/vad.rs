//! The Silero voice activity model on its own: a load that names only a
//! VAD model holds this instead of a Whisper model, and answers
//! `detect_speech` with how much speech a take carries. It runs on the
//! CPU, so it never competes with a recognizer for the GPU.

use std::cell::RefCell;
use std::path::Path;

use dettivo_engine_proto::{DetectSpeechParams, DetectSpeechResult, EngineError};
use whisper_rs::{WhisperVadContext, WhisperVadContextParams, WhisperVadParams};

/// A loaded voice activity model.
pub struct Vad(RefCell<WhisperVadContext>);

impl Vad {
    /// Loads the model at `path` on the CPU.
    pub fn load(path: &str) -> Result<Self, EngineError> {
        if !Path::new(path).is_file() {
            return Err(EngineError::new(
                "model_missing",
                format!("VAD model file not found: {path}"),
            ));
        }
        let mut params = WhisperVadContextParams::new();
        params.set_use_gpu(false);
        WhisperVadContext::new(path, params)
            .map(|ctx| Self(RefCell::new(ctx)))
            .map_err(|e| EngineError::new("load_failed", format!("VAD model load failed: {e}")))
    }

    /// The milliseconds of speech in `pcm` under `params`.
    pub fn detect(
        &self,
        pcm: &[i16],
        params: &DetectSpeechParams,
    ) -> Result<DetectSpeechResult, EngineError> {
        if pcm.is_empty() {
            return Ok(DetectSpeechResult { speech_ms: 0 });
        }
        let mut audio = vec![0.0f32; pcm.len()];
        whisper_rs::convert_integer_to_float_audio(pcm, &mut audio)
            .map_err(|e| EngineError::new("bad_request", format!("pcm conversion: {e}")))?;
        let mut vad = WhisperVadParams::new();
        vad.set_threshold(params.threshold as f32);
        vad.set_min_speech_duration(i32::try_from(params.min_speech_ms).unwrap_or(i32::MAX));
        let segments = self
            .0
            .borrow_mut()
            .segments_from_samples(vad, &audio)
            .map_err(|e| EngineError::new("internal", format!("voice activity: {e}")))?;
        // whisper.cpp reports segment bounds in centiseconds.
        let centiseconds: f32 = segments.map(|s| (s.end - s.start).max(0.0)).sum();
        Ok(DetectSpeechResult {
            speech_ms: (centiseconds * 10.0).round() as u64,
        })
    }
}
