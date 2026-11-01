//! parakeet.cpp through the safe wrapper: load a GGUF model on the chosen
//! backend (Vulkan first, CPU on failure), recognize 16 kHz mono PCM in
//! overlapping chunks, and return segments with word timestamps and
//! confidence. A vocabulary rescores each chunk's transcript towards its
//! terms (ADR 0088); the Whisper prompt is accepted and ignored.

/// The smallest confidence a word reports; the contract's range is (0, 1].
pub const MIN_CONFIDENCE: f64 = 0.001;

use std::cell::{Cell, OnceCell, RefCell};
use std::path::Path;
use std::time::Instant;

use dettivo_engine_proto::{
    Backend, BackendPreference, EngineError, LoadParams, LoadedResult, RecognizeParams,
    RecognizeResult, SpeechEngine, Word, backend,
};

use crate::align;
use crate::bias::{self, Pieces, Rescorer};
use crate::ffi::{self, FfiError, Model, RawTranscription};
use crate::gguf;

/// True when the binary was built with ggml's Vulkan backend.
pub const VULKAN_BUILT_IN: bool = cfg!(feature = "vulkan");

/// A loaded model.
pub struct Engine {
    /// `None` only while a GPU model moves to the CPU.
    model: RefCell<Option<Model>>,
    /// The model path.
    pub path: String,
    /// The backend that runs it.
    backend: Cell<Backend>,
    /// Why that backend.
    reason: RefCell<String>,
    /// Loaded on the GPU by the `auto` preference, so a GPU that cannot
    /// run an inference may move it to the CPU.
    gpu_fallback: Cell<bool>,
    /// The languages the model accepts, when known.
    languages: Option<&'static [&'static str]>,
    /// The tokenizer's pieces, read on the first vocabulary request.
    pieces: OnceCell<Pieces>,
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
            model: RefCell::new(Some(model)),
            path: path.to_string(),
            backend: Cell::new(backend_used),
            reason: RefCell::new(reason),
            gpu_fallback: Cell::new(
                backend_used == Backend::Vulkan && preference == BackendPreference::Auto,
            ),
            languages,
            pieces: OnceCell::new(),
        })
    }

    /// Reloads the model on the CPU for good: a GPU another program holds
    /// (a game) has no room for an inference's buffers, and the engine
    /// would otherwise fail every request.
    fn move_to_cpu(&self, why: &str) -> Result<(), EngineError> {
        // Dropping the model frees the GPU backend its weights live in.
        self.model.replace(None);
        let (model, _) = attempt(&self.path, true)
            .map_err(|e| EngineError::new("load_failed", format!("CPU reload failed: {e}")))?;
        self.model.replace(Some(model));
        self.backend.set(Backend::Cpu);
        self.gpu_fallback.set(false);
        let reason = format!(
            "{}; the GPU could not run it ({why}), CPU fallback",
            self.reason.borrow()
        );
        self.reason.replace(reason);
        Ok(())
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
            backend: self.backend.get(),
            reason: self.reason.borrow().clone(),
            fallback_reason: None,
        }
    }

    /// An inference the GPU fails (out of memory while another program
    /// holds it) reloads the model on the CPU under `auto` and runs again.
    fn recognize(
        &self,
        pcm: &[i16],
        params: &RecognizeParams,
        progress: &mut dyn FnMut(f64),
    ) -> Result<RecognizeResult, EngineError> {
        match self.recognize_once(pcm, params, progress) {
            Err(e) if self.gpu_fallback.get() && e.message.starts_with("inference:") => {
                tracing::warn!(error = %e.message, "inference failed on the GPU; moving the model to the CPU");
                self.move_to_cpu(&e.message)?;
                self.recognize_once(pcm, params, progress)
            }
            other => other,
        }
    }
}

impl Engine {
    fn recognize_once(
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
                backend: self.backend.get(),
            });
        }
        let audio: Vec<f32> = pcm.iter().map(|s| f32::from(*s) / 32_768.0).collect();
        let ranges = align::chunks(audio.len());
        let rescorer = params.vocabulary.as_ref().and_then(|bias| {
            let pieces = self.pieces.get_or_init(|| {
                Pieces::new(
                    self.model
                        .borrow()
                        .as_ref()
                        .map(Model::pieces)
                        .unwrap_or_default(),
                )
            });
            Rescorer::new(bias, pieces)
        });
        let mut rescored = (bias::Outcome::default(), 0.0f64);
        let mut chunks = Vec::with_capacity(ranges.len());
        let mut single_text = None;
        for (i, (start, end)) in ranges.iter().enumerate() {
            let lang = if params.language == "auto" {
                ""
            } else {
                params.language.as_str()
            };
            let model = self.model.borrow();
            let model = model
                .as_ref()
                .ok_or_else(|| EngineError::new("internal", "the model is not loaded"))?;
            let clip = &audio[*start..*end];
            let raw = match &rescorer {
                Some(r) => self.rescored(model, r, clip, lang, &mut rescored)?,
                None => model.transcribe(clip, lang).map_err(inference)?,
            };
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
        if rescorer.is_some() {
            // Counts and time only: never the terms or the transcript.
            tracing::info!(
                candidates = rescored.0.candidates,
                replaced = rescored.0.replaced,
                rescore_ms = (rescored.1 * 10.0).round() / 10.0,
                "vocabulary rescoring"
            );
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
            backend: self.backend.get(),
        })
    }

    /// One chunk recognized and rescored; adds what rescoring did and its
    /// time to `total`. A model rescoring cannot read is transcribed as is.
    fn rescored(
        &self,
        model: &Model,
        rescorer: &Rescorer,
        clip: &[f32],
        lang: &str,
        total: &mut (bias::Outcome, f64),
    ) -> Result<RawTranscription, EngineError> {
        let (take, mut raw) = match model.take(clip) {
            Ok(v) => v,
            Err(e) if e.0.starts_with("vocabulary rescoring needs") => {
                return model.transcribe(clip, lang).map_err(inference);
            }
            Err(e) => return Err(inference(e)),
        };
        let started = Instant::now();
        let pieces = self
            .pieces
            .get()
            .expect("pieces are read before a rescorer exists");
        let outcome = rescorer
            .rescore(&take, pieces, &mut raw)
            .map_err(inference)?;
        total.0.candidates += outcome.candidates;
        total.0.replaced += outcome.replaced;
        total.1 += started.elapsed().as_secs_f64() * 1000.0;
        Ok(raw)
    }
}

/// A failed inference, worded so a GPU that cannot run it moves the model
/// to the CPU.
fn inference(e: FfiError) -> EngineError {
    EngineError::new("internal", format!("inference: {e}"))
}
