//! whisper.cpp through `whisper-rs`: load a ggml model on the chosen
//! backend (Vulkan first, CPU on failure), recognize 16 kHz mono PCM with
//! optional VAD, a language and a vocabulary prompt, and return timestamped
//! segments. The protocol loop and the CLI mode come from
//! `dettivo-engine-proto`'s host.

use std::path::Path;

use dettivo_engine_proto::{
    Backend, BackendPreference, EngineError, LoadParams, LoadedResult, RecognizeParams,
    RecognizeResult, Segment, SpeechEngine, backend,
};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// True when the binary was built with ggml's Vulkan backend.
pub const VULKAN_BUILT_IN: bool = cfg!(feature = "vulkan");

/// How many Vulkan devices ggml's Vulkan backend enumerates: the count
/// whisper.cpp itself consults when it picks a GPU backend, so a load
/// with `use_gpu` runs there only when this is above zero.
#[cfg(feature = "vulkan")]
fn vulkan_devices() -> i32 {
    unsafe extern "C" {
        fn ggml_backend_vk_get_device_count() -> std::os::raw::c_int;
    }
    // SAFETY: the ggml Vulkan backend is linked into this binary under
    // the `vulkan` feature and the function takes no arguments.
    unsafe { ggml_backend_vk_get_device_count() }
}

/// Without the backend nothing enumerates.
#[cfg(not(feature = "vulkan"))]
fn vulkan_devices() -> i32 {
    0
}

/// A loaded model.
pub struct Engine {
    context: WhisperContext,
    /// The model path.
    pub model: String,
    /// The backend that loaded it.
    pub backend: Backend,
    /// Why that backend.
    pub reason: String,
    vad_model: Option<String>,
}

impl Engine {
    /// Loads `model` on the preferred backend, falling back to CPU when the
    /// GPU load fails, and says which happened.
    pub fn load_with(
        model: &str,
        preference: BackendPreference,
        force_cpu: bool,
        vad_model: Option<&str>,
    ) -> Result<Self, EngineError> {
        if !Path::new(model).is_file() {
            return Err(EngineError::new(
                "model_missing",
                format!("model file not found: {model}"),
            ));
        }
        if let Some(vad) = vad_model {
            if !Path::new(vad).is_file() {
                return Err(EngineError::new(
                    "model_missing",
                    format!("VAD model file not found: {vad}"),
                ));
            }
        }
        let (mut first, mut why) = backend::choose(preference, force_cpu, VULKAN_BUILT_IN)
            .map_err(|why| EngineError::new("load_failed", backend::strict_vulkan_refused(&why)))?;
        // An installed ICD is not a usable device: ggml has to enumerate
        // one, else whisper.cpp would open the model on the CPU while the
        // load was asked for and reported as Vulkan.
        if first == Backend::Vulkan {
            match vulkan_devices() {
                0 => {
                    let none = "no Vulkan device enumerates in ggml".to_string();
                    if preference == BackendPreference::Vulkan {
                        return Err(EngineError::new(
                            "load_failed",
                            backend::strict_vulkan_refused(&none),
                        ));
                    }
                    first = Backend::Cpu;
                    why = none;
                }
                n => why = format!("{why}; ggml enumerates {n} Vulkan device(s)"),
            }
        }
        let attempt = |use_gpu: bool| {
            let mut params = WhisperContextParameters::default();
            params.use_gpu(use_gpu);
            WhisperContext::new_with_params(model, params)
        };
        let (context, backend_used, reason) = match first {
            Backend::Vulkan => match attempt(true) {
                Ok(ctx) => (ctx, Backend::Vulkan, why),
                Err(e) => {
                    if preference == BackendPreference::Vulkan {
                        return Err(EngineError::new(
                            "load_failed",
                            backend::strict_vulkan_refused(&format!("Vulkan load failed: {e}")),
                        ));
                    }
                    tracing::warn!(error = %e, "Vulkan load failed, falling back to CPU");
                    let ctx = attempt(false).map_err(|e| {
                        EngineError::new(
                            "load_failed",
                            format!("CPU load failed after Vulkan failed: {e}"),
                        )
                    })?;
                    (
                        ctx,
                        Backend::Cpu,
                        format!("Vulkan load failed ({e}); CPU fallback"),
                    )
                }
            },
            Backend::Cpu => {
                let ctx = attempt(false)
                    .map_err(|e| EngineError::new("load_failed", format!("load failed: {e}")))?;
                (ctx, Backend::Cpu, why)
            }
            Backend::Cuda => {
                return Err(EngineError::new(
                    "load_failed",
                    "Whisper has no CUDA provider",
                ));
            }
        };
        tracing::info!(backend = ?backend_used, reason = %reason, "model loaded");
        Ok(Self {
            context,
            model: model.to_string(),
            backend: backend_used,
            reason,
            vad_model: vad_model.map(str::to_string),
        })
    }
}

impl SpeechEngine for Engine {
    fn load(params: &LoadParams, force_cpu: bool) -> Result<Self, EngineError> {
        Self::load_with(
            &params.model,
            params.backend_preference,
            force_cpu,
            params.vad_model.as_deref(),
        )
    }

    fn loaded(&self) -> LoadedResult {
        LoadedResult {
            model: self.model.clone(),
            backend: self.backend,
            reason: self.reason.clone(),
            fallback_reason: None,
        }
    }

    fn recognize(
        &self,
        pcm: &[i16],
        params: &RecognizeParams,
        _progress: &mut dyn FnMut(f64),
    ) -> Result<RecognizeResult, EngineError> {
        let language = params.language.as_str();
        let duration_ms = (pcm.len() as u64 * 1000) / 16_000;
        if pcm.is_empty() {
            return Ok(RecognizeResult {
                text: String::new(),
                language: language.to_string(),
                segments: Vec::new(),
                duration_ms: 0,
                backend: self.backend,
            });
        }
        let mut audio: Vec<f32> = vec![0.0; pcm.len()];
        whisper_rs::convert_integer_to_float_audio(pcm, &mut audio)
            .map_err(|e| EngineError::new("bad_request", format!("pcm conversion: {e}")))?;
        let mut full = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        full.set_print_progress(false);
        full.set_print_special(false);
        full.set_print_realtime(false);
        full.set_print_timestamps(false);
        full.set_suppress_blank(true);
        full.set_n_threads(available_threads());
        if language != "auto" {
            full.set_language(Some(language));
        } else {
            full.set_language(None);
        }
        if let Some(p) = params.prompt.as_deref().filter(|p| !p.is_empty()) {
            full.set_initial_prompt(p);
        }
        if let Some(vad) = &self.vad_model {
            full.set_vad_model_path(Some(vad.as_str()));
        }
        let mut state = self
            .context
            .create_state()
            .map_err(|e| EngineError::new("internal", format!("state: {e}")))?;
        state
            .full(full, &audio)
            .map_err(|e| EngineError::new("internal", format!("inference: {e}")))?;
        let mut segments = Vec::new();
        let mut text = String::new();
        let n = state.full_n_segments();
        for i in 0..n {
            let Some(segment) = state.get_segment(i) else {
                continue;
            };
            let piece = segment
                .to_str_lossy()
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if piece.is_empty() {
                continue;
            }
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(&piece);
            if params.timestamps {
                segments.push(Segment {
                    start_ms: (segment.start_timestamp().max(0) as u64) * 10,
                    end_ms: (segment.end_timestamp().max(0) as u64) * 10,
                    text: piece,
                    words: Vec::new(),
                });
            }
        }
        // An English-only model (`*.en`) has no language detector worth
        // reading; everything else reports what whisper detected.
        let english_only = self.model.contains(".en.") || self.model.ends_with(".en");
        let detected = if language != "auto" {
            language.to_string()
        } else if english_only {
            "en".to_string()
        } else {
            whisper_rs::get_lang_str(state.full_lang_id_from_state())
                .map(str::to_string)
                .unwrap_or_else(|| "auto".into())
        };
        Ok(RecognizeResult {
            text,
            language: detected,
            segments,
            duration_ms,
            backend: self.backend,
        })
    }
}

fn available_threads() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get().min(8) as i32)
        .unwrap_or(4)
}
