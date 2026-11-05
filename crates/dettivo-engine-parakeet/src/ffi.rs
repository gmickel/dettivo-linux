//! The safe wrapper over `parakeet-cpp-sys`: a loaded model, transcription
//! with word timestamps and confidence, the C API's error strings, and
//! the process-global backend (device name, threads, reset). Every
//! `unsafe` block here is the one place the engine touches raw pointers.

#![allow(unsafe_code, reason = "the FFI boundary to parakeet.cpp lives here")]

use std::ffi::{CStr, CString};
use std::path::Path;

use parakeet_cpp_sys as sys;
use serde::Deserialize;

/// A word as parakeet.cpp aligns it: seconds and a confidence in (0, 1].
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RawWord {
    /// The word.
    pub w: String,
    /// Start in seconds.
    pub start: f64,
    /// End in seconds.
    pub end: f64,
    /// Confidence (NeMo `max_prob`, the minimum over the word's tokens).
    pub conf: f64,
}

/// One clip's transcription from the JSON entry point.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RawTranscription {
    /// The text.
    pub text: String,
    /// The encoder frame stride in seconds.
    #[serde(default)]
    pub frame_sec: f64,
    /// Words in order.
    #[serde(default)]
    pub words: Vec<RawWord>,
}

/// Why the C API failed, with its own words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfiError(pub String);

impl std::fmt::Display for FfiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A loaded GGUF model.
pub struct Model {
    ctx: *mut sys::parakeet_ctx,
}

// SAFETY: the context is only ever used from the engine's one thread; the
// library serialises inference behind its own mutex besides.
unsafe impl Send for Model {}

impl Model {
    /// Loads the GGUF at `path`; a failure carries the library's reason
    /// (a missing file is reported by the caller before this is reached).
    pub fn load(path: &Path) -> Result<Self, FfiError> {
        let c_path = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| FfiError("model path contains a NUL byte".into()))?;
        // SAFETY: a valid NUL-terminated path; NULL is the documented failure.
        let ctx = unsafe { sys::parakeet_capi_load(c_path.as_ptr()) };
        if ctx.is_null() {
            return Err(FfiError(format!(
                "parakeet.cpp could not load {}: not a readable Parakeet GGUF",
                path.display()
            )));
        }
        Ok(Self { ctx })
    }

    /// The last error the library recorded on this context.
    fn last_error(&self) -> String {
        // SAFETY: the context is live; the pointer is owned by it and read
        // before any other call on it.
        unsafe {
            let p = sys::parakeet_capi_last_error(self.ctx);
            if p.is_null() {
                String::new()
            } else {
                CStr::from_ptr(p).to_string_lossy().into_owned()
            }
        }
    }

    /// Transcribes 16 kHz mono float samples with word timestamps.
    /// `language` is passed to the library's prompt selection (ignored by
    /// the TDT models, which detect the language themselves).
    pub fn transcribe(
        &self,
        samples: &[f32],
        language: &str,
    ) -> Result<RawTranscription, FfiError> {
        let n = i32::try_from(samples.len())
            .map_err(|_| FfiError("clip longer than the C API's sample count".into()))?;
        let lang = CString::new(language).map_err(|_| FfiError("language has a NUL".into()))?;
        let lengths = [n];
        // SAFETY: `samples` holds exactly `n` floats, `lengths` has one
        // entry summing to `n` (the API's precondition), the context is
        // live, and the returned string is freed below.
        let json = unsafe {
            let p = sys::parakeet_capi_transcribe_pcm_batch_json_lang(
                self.ctx,
                samples.as_ptr(),
                lengths.as_ptr(),
                1,
                16_000,
                0,
                lang.as_ptr(),
            );
            if p.is_null() {
                return Err(FfiError(self.last_error()));
            }
            let text = CStr::from_ptr(p).to_string_lossy().into_owned();
            sys::parakeet_capi_free_string(p);
            text
        };
        let mut clips: Vec<RawTranscription> = serde_json::from_str(&json)
            .map_err(|e| FfiError(format!("transcription JSON: {e}")))?;
        clips
            .pop()
            .ok_or_else(|| FfiError("transcription JSON held no clip".into()))
    }

    /// Transcribes like [`Model::transcribe`] and keeps the take for
    /// rescoring. Fails for a model rescoring cannot read (not TDT).
    pub fn take(&self, samples: &[f32]) -> Result<(Take<'_>, RawTranscription), FfiError> {
        let n = i32::try_from(samples.len())
            .map_err(|_| FfiError("clip longer than the C API's sample count".into()))?;
        let mut json: *mut std::os::raw::c_char = std::ptr::null_mut();
        // SAFETY: `samples` holds `n` floats, the context is live, `json`
        // is a valid out pointer; the string is freed below and the take
        // by `Take::drop`.
        let (take, text) = unsafe {
            let take =
                sys::parakeet_sys_take_open(self.ctx, samples.as_ptr(), n, c"".as_ptr(), &mut json);
            if take.is_null() {
                return Err(FfiError(self.last_error()));
            }
            let text = CStr::from_ptr(json).to_string_lossy().into_owned();
            sys::parakeet_capi_free_string(json);
            (take, text)
        };
        let take = Take {
            ptr: take,
            _model: std::marker::PhantomData,
        };
        let raw = serde_json::from_str(&text)
            .map_err(|e| FfiError(format!("transcription JSON: {e}")))?;
        Ok((take, raw))
    }

    /// The tokenizer's pieces, by id.
    pub fn pieces(&self) -> Vec<String> {
        // SAFETY: the context is live; each piece pointer is owned by the
        // model and copied at once.
        unsafe {
            let n = sys::parakeet_sys_vocab_size(self.ctx);
            (0..n)
                .map(|id| {
                    let p = sys::parakeet_sys_piece(self.ctx, id);
                    if p.is_null() {
                        String::new()
                    } else {
                        CStr::from_ptr(p).to_string_lossy().into_owned()
                    }
                })
                .collect()
        }
    }
}

/// One greedy token of a take.
pub type Token = sys::parakeet_sys_token;

/// A recognized clip kept for rescoring; lives no longer than its model.
pub struct Take<'m> {
    ptr: *mut sys::parakeet_sys_take,
    _model: std::marker::PhantomData<&'m Model>,
}

impl Take<'_> {
    /// The greedy tokens.
    pub fn tokens(&self) -> &[Token] {
        // SAFETY: the array belongs to the live take and holds `n` tokens.
        unsafe {
            let mut p: *const Token = std::ptr::null();
            let n = sys::parakeet_sys_take_tokens(self.ptr, &mut p);
            if p.is_null() || n <= 0 {
                &[]
            } else {
                std::slice::from_raw_parts(p, n as usize)
            }
        }
    }

    /// The encoder frame count.
    pub fn frames(&self) -> i32 {
        // SAFETY: a plain getter on the live take.
        unsafe { sys::parakeet_sys_take_frames(self.ptr) }
    }

    /// The log-probability of reading `tokens` over frames `[lo, hi)` from
    /// where greedy decoding stood before token `start`.
    pub fn score(&self, start: usize, lo: i32, hi: i32, tokens: &[i32]) -> Result<f32, FfiError> {
        let start = i32::try_from(start).map_err(|_| FfiError("token index".into()))?;
        let mut out = 0.0f32;
        // SAFETY: `tokens` holds its length in ids; the take is live and
        // is only used from this thread.
        let rc = unsafe {
            sys::parakeet_sys_take_score(
                self.ptr,
                start,
                lo,
                hi,
                tokens.as_ptr(),
                tokens.len() as i32,
                &mut out,
            )
        };
        if rc == 0 {
            Ok(out)
        } else {
            Err(FfiError("rescoring a span failed".into()))
        }
    }
}

impl Drop for Take<'_> {
    fn drop(&mut self) {
        // SAFETY: the take came from parakeet_sys_take_open and is freed once.
        unsafe { sys::parakeet_sys_take_free(self.ptr) };
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        // SAFETY: the context came from parakeet_capi_load and is freed once.
        unsafe { sys::parakeet_capi_free(self.ctx) };
        shutdown_backend();
    }
}

/// The ggml device the process-global backend runs on (`cpu`, `Vulkan0`,
/// ...); creating it on first call.
pub fn backend_device_name() -> String {
    // SAFETY: the pointer belongs to the backend and is copied at once.
    unsafe { CStr::from_ptr(sys::parakeet_sys_backend_device_name()) }
        .to_string_lossy()
        .into_owned()
}

/// Sets the compute thread count for every graph.
pub fn set_num_threads(n: i32) {
    // SAFETY: a plain integer setter.
    unsafe { sys::parakeet_sys_set_num_threads(n) };
}

/// Frees the process-global backend so the next inference creates it
/// afresh (after `PARAKEET_DEVICE` changed). Every `Model` must be dropped
/// first: their weights live in the backend's buffers.
pub fn shutdown_backend() {
    // SAFETY: called only once every model is dropped (see the doc).
    unsafe { sys::parakeet_sys_shutdown_backend() };
}

/// Pins the ggml device parakeet.cpp picks when the backend is created:
/// `cpu`, or unset for the first GPU the registry reports.
pub fn set_device(force_cpu: bool) {
    // SAFETY: the engine is single-threaded when this runs (before any
    // inference thread exists), which is the condition set_var asks for.
    unsafe {
        if force_cpu {
            std::env::set_var("PARAKEET_DEVICE", "cpu");
        } else {
            std::env::remove_var("PARAKEET_DEVICE");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transcription_document_parses_with_its_words() {
        let json = r#"[{"text":"hello there","frame_sec":0.080000,
            "words":[{"w":"hello","start":0.480,"end":0.640,"conf":0.9100},
                     {"w":"there","start":0.720,"end":1.040,"conf":0.8000}],
            "tokens":[{"id":1,"t":0.48,"conf":0.91}]}]"#;
        let mut clips: Vec<RawTranscription> = serde_json::from_str(json).unwrap();
        let clip = clips.pop().unwrap();
        assert_eq!(clip.text, "hello there");
        assert_eq!(clip.words.len(), 2);
        assert_eq!(clip.words[1].w, "there");
        assert!((clip.frame_sec - 0.08).abs() < 1e-9);
    }

    #[test]
    fn a_missing_model_is_an_error_naming_the_path() {
        let err = match Model::load(Path::new("/nonexistent/x.gguf")) {
            Ok(_) => panic!("a missing file loaded"),
            Err(e) => e,
        };
        assert!(err.0.contains("/nonexistent/x.gguf"), "{err}");
    }
}
