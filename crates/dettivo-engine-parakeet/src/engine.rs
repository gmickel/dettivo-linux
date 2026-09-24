//! parakeet.cpp through the safe wrapper: load a GGUF model on the chosen
//! backend (Vulkan first, CPU on failure), recognize 16 kHz mono PCM in
//! overlapping chunks, and return segments with word timestamps and
//! confidence. The prompt is accepted and ignored: Parakeet takes no
//! vocabulary.

/// The smallest confidence a word reports; the contract's range is (0, 1].
pub const MIN_CONFIDENCE: f64 = 0.001;

use std::path::Path;

use dettivo_engine_proto::{
    Backend, BackendPreference, EngineError, LoadParams, LoadedResult, RecognizeParams,
    RecognizeResult, SpeechEngine, Word, backend,
};

use crate::align;
use crate::ffi::{self, FfiError, Model};
use crate::gguf;

/// True when the binary was built with ggml's Vulkan backend.
pub const VULKAN_BUILT_IN: bool = cfg!(feature = "vulkan");

/// A loaded model.
pub struct Engine {
    model: Model,
    /// The model path.
    pub path: String,
    /// The backend that loaded it.
    pub backend: Backend,
    /// Why that backend.
    pub reason: String,
    /// The languages the model accepts, when known.
    languages: Option<&'static [&'static str]>,
}

impl Engine {
    /// Loads the model with a preference and the CPU switch.
    pub fn load_with(
        path: &str,
        preference: BackendPreference,
        force_cpu: bool,
    ) -> Result<Self, EngineError> {
        if !Path::new(path).is_file() {
            return Err(EngineError::new(
                "model_missing",
                format!("model file not found: {path}"),
            ));
        }
        let header = gguf::read_header(Path::new(path))
            .map_err(|e| EngineError::new("load_failed", format!("{path}: {e}")))?;
        let languages = header.name.as_deref().and_then(align::languages_for);
        let (first, why) = backend::choose(preference, force_cpu, VULKAN_BUILT_IN)
            .map_err(|why| EngineError::new("load_failed", backend::strict_vulkan_refused(&why)))?;
        ffi::set_num_threads(available_threads());
        let (model, backend_used, reason) = match first {
            Backend::Vulkan => match attempt(path, false) {
                Ok((model, device)) if device != "cpu" => {
                    (model, Backend::Vulkan, format!("{why} ({device})"))
                }
                // The backend the native engine initialised is the truth:
                // `cpu` under a strict preference is a refusal.
                Ok((model, _)) if preference == BackendPreference::Vulkan => {
                    drop(model);
                    return Err(EngineError::new(
                        "load_failed",
                        backend::strict_vulkan_refused(
                            "parakeet.cpp initialised no Vulkan device and opened the model on the CPU",
                        ),
                    ));
                }
                Ok((model, _)) => (
                    model,
                    Backend::Cpu,
                    "no Vulkan device initialised; parakeet.cpp fell back to CPU".into(),
                ),
                Err(e) => {
                    if preference == BackendPreference::Vulkan {
                        return Err(EngineError::new(
                            "load_failed",
                            backend::strict_vulkan_refused(&format!("Vulkan load failed: {e}")),
                        ));
                    }
                    tracing::warn!(error = %e, "Vulkan load failed, falling back to CPU");
                    ffi::shutdown_backend();
                    let (model, _) = attempt(path, true).map_err(|e| {
                        EngineError::new(
                            "load_failed",
                            format!("CPU load failed after Vulkan failed: {e}"),
                        )
                    })?;
                    (
                        model,
                        Backend::Cpu,
                        format!("Vulkan load failed ({e}); CPU fallback"),
                    )
                }
            },
            Backend::Cpu => {
                let (model, _) = attempt(path, true)
                    .map_err(|e| EngineError::new("load_failed", format!("load failed: {e}")))?;
                (model, Backend::Cpu, why)
            }
            Backend::Cuda => {
                return Err(EngineError::new(
                    "load_failed",
                    "Parakeet has no CUDA provider",
                ));
            }
        };
        tracing::info!(
            backend = ?backend_used,
            reason = %reason,
            model_name = header.name.as_deref().unwrap_or("unknown"),
            "model loaded"
        );
        Ok(Self {
            model,
            path: path.to_string(),
            backend: backend_used,
            reason,
            languages,
        })
    }
}

/// Loads the model and runs half a second of silence through it, so the
/// backend is created on the requested device and the weights are on it
/// before `load` answers; returns the device parakeet.cpp reports.
fn attempt(path: &str, force_cpu: bool) -> Result<(Model, String), FfiError> {
    ffi::set_device(force_cpu);
    let model = Model::load(Path::new(path))?;
    model.transcribe(&vec![0.0f32; 8_000], "")?;
    let device = ffi::backend_device_name();
    Ok((model, device))
}

fn available_threads() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get().min(8) as i32)
        .unwrap_or(4)
}

impl SpeechEngine for Engine {
    fn load(params: &LoadParams, force_cpu: bool) -> Result<Self, EngineError> {
        Self::load_with(&params.model, params.backend_preference, force_cpu)
    }

    fn loaded(&self) -> LoadedResult {
        LoadedResult {
            model: self.path.clone(),
            backend: self.backend,
            reason: self.reason.clone(),
            fallback_reason: None,
        }
    }

    fn recognize(
        &self,
        pcm: &[i16],
        params: &RecognizeParams,
        progress: &mut dyn FnMut(f64),
    ) -> Result<RecognizeResult, EngineError> {
        align::check_language(&params.language, self.languages)
            .map_err(|m| EngineError::new("bad_request", m))?;
        let duration_ms = (pcm.len() as u64 * 1000) / 16_000;
        let language = if params.language != "auto" {
            params.language.clone()
        } else if self.languages == Some(&["en"]) {
            "en".to_string()
        } else {
            "auto".to_string()
        };
        if pcm.is_empty() {
            return Ok(RecognizeResult {
                text: String::new(),
                language,
                segments: Vec::new(),
                duration_ms: 0,
                backend: self.backend,
            });
        }
        let audio: Vec<f32> = pcm.iter().map(|s| f32::from(*s) / 32_768.0).collect();
        let ranges = align::chunks(audio.len());
        let mut chunks = Vec::with_capacity(ranges.len());
        let mut single_text = None;
        for (i, (start, end)) in ranges.iter().enumerate() {
            let lang = if params.language == "auto" {
                ""
            } else {
                params.language.as_str()
            };
            let raw = self
                .model
                .transcribe(&audio[*start..*end], lang)
                .map_err(|e| EngineError::new("internal", format!("inference: {e}")))?;
            let offset_ms = (*start as u64 * 1000) / 16_000;
            let words: Vec<Word> = raw
                .words
                .iter()
                .filter(|w| !w.w.trim().is_empty())
                .map(|w| Word {
                    start_ms: offset_ms + (w.start.max(0.0) * 1000.0).round() as u64,
                    end_ms: offset_ms + (w.end.max(0.0) * 1000.0).round() as u64,
                    text: w.w.trim().to_string(),
                    // The contract's range is (0, 1]: a zero or negative
                    // value from the model becomes the smallest positive.
                    confidence: w.conf.clamp(0.0, 1.0).max(MIN_CONFIDENCE),
                })
                .collect();
            if ranges.len() == 1 {
                single_text = Some(raw.text.trim().to_string());
            }
            chunks.push((*start, *end, words));
            progress((i + 1) as f64 / (ranges.len() + 1) as f64);
        }
        let words = align::stitch(&chunks);
        let segments = if params.timestamps {
            align::segments(&words)
        } else {
            Vec::new()
        };
        // One chunk keeps the library's own text; stitched chunks are
        // rejoined from their words.
        let text = single_text.unwrap_or_else(|| {
            words
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        });
        Ok(RecognizeResult {
            text,
            language,
            segments,
            duration_ms,
            backend: self.backend,
        })
    }
}
