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
use whisper_rs::{
    DtwMode, DtwParameters, FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
    WhisperSegment,
};

use crate::words::{self, Spoken, Timing, Token};

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
    timing: Timing,
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
        let preset = words::dtw_preset(Path::new(model));
        let timing = if preset.is_some() {
            Timing::Dtw
        } else {
            Timing::Plain
        };
        let attempt = |use_gpu: bool| {
            let mut params = WhisperContextParameters::default();
            params.use_gpu(use_gpu);
            if let Some(model_preset) = preset.clone() {
                params.dtw_parameters(DtwParameters {
                    mode: DtwMode::ModelPreset { model_preset },
                    ..DtwParameters::default()
                });
            }
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
        tracing::info!(backend = ?backend_used, reason = %reason, timing = ?timing, "model loaded");
        Ok(Self {
            context,
            model: model.to_string(),
            backend: backend_used,
            reason,
            vad_model: vad_model.map(str::to_string),
            timing,
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
        // whisper.cpp maps segment times back from VAD-compressed audio but
        // not token times, so words are timed only without VAD.
        let timed = params.timestamps && self.vad_model.is_none();
        full.set_token_timestamps(timed && self.timing == Timing::Plain);
        let mut token_beg = self.context.token_beg();
        if self.timing == Timing::Dtw {
            #[allow(unsafe_code)]
            // SAFETY: the callback reads the arguments whisper.cpp passes and
            // `token_beg`, which outlives `full` and the `state.full` call.
            unsafe {
                full.set_filter_logits_callback(Some(dtw_safe_timestamps));
                full.set_filter_logits_callback_user_data(
                    (&raw mut token_beg).cast::<std::os::raw::c_void>(),
                );
            }
        }
        let mut state = self
            .context
            .create_state()
            .map_err(|e| EngineError::new("internal", format!("state: {e}")))?;
        state
            .full(full, &audio)
            .map_err(|e| EngineError::new("internal", format!("inference: {e}")))?;
        let mut spoken = Vec::new();
        let mut pieces = Vec::new();
        let mut text = String::new();
        let eot = self.context.token_eot();
        for i in 0..state.full_n_segments() {
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
            spoken.push(Spoken {
                span: (
                    segment.start_timestamp().max(0),
                    segment.end_timestamp().max(0),
                ),
                tokens: if timed {
                    tokens_of(&segment, eot)
                } else {
                    Vec::new()
                },
            });
            pieces.push(piece);
        }
        let audio_cs = (pcm.len() / 160) as i64;
        let timed_segments = if timed {
            words::timed(&spoken, self.timing, audio_cs)
        } else {
            spoken.iter().map(|s| (s.span, Vec::new())).collect()
        };
        let segments = if params.timestamps {
            pieces
                .into_iter()
                .zip(timed_segments)
                .map(|(text, ((start, end), words))| Segment {
                    start_ms: start as u64 * 10,
                    end_ms: end as u64 * 10,
                    text,
                    words,
                })
                .collect()
        } else {
            Vec::new()
        };
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

/// The earliest timestamp, in 20 ms steps, a decode may close a segment at
/// under DTW. whisper.cpp 1.8.3 median-filters the DTW alignment over a
/// 7-step window and aborts the process when a decode advanced 7 steps or
/// fewer, which a segment closed before 160 ms produces (`median_filter`).
const DTW_MIN_CLOSE_STEPS: i32 = 8;

/// Masks the timestamps before `DTW_MIN_CLOSE_STEPS` once the decode holds
/// a timestamp, so no segment closes early enough to abort the DTW pass;
/// the opening timestamp stays free.
#[allow(unsafe_code)]
unsafe extern "C" fn dtw_safe_timestamps(
    _ctx: *mut whisper_rs::WhisperSysContext,
    _state: *mut whisper_rs::WhisperSysState,
    tokens: *const whisper_rs::WhisperTokenData,
    n_tokens: std::os::raw::c_int,
    logits: *mut f32,
    token_beg: *mut std::os::raw::c_void,
) {
    let Ok(n) = usize::try_from(n_tokens) else {
        return;
    };
    if n == 0 || tokens.is_null() || logits.is_null() || token_beg.is_null() {
        return;
    }
    // SAFETY: whisper.cpp passes `n_tokens` initialised tokens and one logit
    // per vocabulary entry, which covers every timestamp token, and
    // `token_beg` points at the id `recognize` keeps alive for the call.
    unsafe {
        let beg = *token_beg.cast::<i32>();
        let tokens = std::slice::from_raw_parts(tokens, n);
        if tokens.iter().any(|t| t.id >= beg) {
            for step in 0..DTW_MIN_CLOSE_STEPS {
                *logits.add((beg + step) as usize) = f32::NEG_INFINITY;
            }
        }
    }
}

/// The segment's text tokens (ids below end-of-text; the rest are
/// timestamps and control tokens).
fn tokens_of(segment: &WhisperSegment<'_>, eot: i32) -> Vec<Token> {
    (0..segment.n_tokens())
        .filter_map(|i| segment.get_token(i))
        .filter(|t| t.token_id() < eot)
        .map(|t| {
            let data = t.token_data();
            Token {
                bytes: t.to_bytes().map(<[u8]>::to_vec).unwrap_or_default(),
                t0: data.t0,
                t1: data.t1,
                dtw: data.t_dtw,
                p: data.p,
            }
        })
        .collect()
}

fn available_threads() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get().min(8) as i32)
        .unwrap_or(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(id: i32) -> whisper_rs::WhisperTokenData {
        whisper_rs::WhisperTokenData {
            id,
            tid: 0,
            p: 1.0,
            plog: 0.0,
            pt: 0.0,
            ptsum: 0.0,
            t0: 0,
            t1: 0,
            t_dtw: 0,
            vlen: 0.0,
        }
    }

    /// Runs the mask over `ids` with timestamps starting at id 100.
    fn masked(ids: &[i32]) -> Vec<usize> {
        let mut beg = 100;
        let tokens: Vec<_> = ids.iter().copied().map(token).collect();
        let mut logits = vec![0.0f32; 200];
        #[allow(unsafe_code)]
        // SAFETY: the pointers cover `tokens`, `logits` and `beg`, all alive here.
        unsafe {
            dtw_safe_timestamps(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                tokens.as_ptr(),
                tokens.len() as i32,
                logits.as_mut_ptr(),
                (&raw mut beg).cast(),
            );
        }
        (0..logits.len())
            .filter(|&i| logits[i].is_infinite())
            .collect()
    }

    /// A decode that opened a segment may not close it before 160 ms, so
    /// whisper.cpp's DTW median filter never sees a window it aborts on.
    #[test]
    fn early_closing_timestamps_are_masked_once_a_segment_opened() {
        assert!(masked(&[]).is_empty(), "the opening timestamp stays free");
        assert!(masked(&[5, 6]).is_empty(), "text alone opens nothing");
        assert_eq!(masked(&[100, 5]), (100..108).collect::<Vec<_>>());
    }
}
