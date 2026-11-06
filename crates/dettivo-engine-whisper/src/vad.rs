//! The Silero voice activity model on its own: a load that names only a
//! VAD model holds this instead of a Whisper model, and answers
//! `detect_speech` with how much speech a take carries. It runs on the
//! CPU, so it never competes with a recognizer for the GPU.
//!
//! A take can also be judged as it is captured (ADR 0097): `feed_speech`
//! runs the model over each whole 512-sample window as it arrives, carrying
//! the model's state, and `detect_speech` with `continues` runs the rest
//! and finds the speech over every window. The windows, the state and so
//! the answer are exactly those of one `detect_speech` over the whole take.
//! A feed that names a `threshold` also answers where the last window whose
//! probability reached it ends, so the release can end a take early whose
//! last stretch holds no voice (ADR 0093).

use std::cell::RefCell;
use std::ffi::{CString, c_int};
use std::path::Path;

use dettivo_engine_proto::{
    DetectSpeechParams, DetectSpeechResult, EngineError, FeedSpeechParams, FeedSpeechResult,
};
use whisper_rs::whisper_rs_sys as sys;

/// Samples per Silero window.
const WINDOW: usize = 512;

#[allow(unsafe_code)]
unsafe extern "C" {
    /// The patched detection (third_party/whisper-rs-sys): `reset` starts a
    /// stream, otherwise the samples continue it.
    fn dettivo_vad_detect_speech_more(
        vctx: *mut sys::whisper_vad_context,
        samples: *const f32,
        n_samples: c_int,
        reset: bool,
    ) -> bool;
}

/// A loaded voice activity model.
pub struct Vad {
    ctx: Context,
    stream: RefCell<Option<Stream>>,
}

/// A take being judged as it is captured.
struct Stream {
    /// The samples received so far.
    samples: u64,
    /// Received samples short of a whole window, held for the next piece.
    pending: Vec<f32>,
    /// No window has run yet, so the next one starts the model afresh.
    fresh: bool,
}

struct Context(*mut sys::whisper_vad_context);

impl Drop for Context {
    fn drop(&mut self) {
        #[allow(unsafe_code)]
        // SAFETY: the pointer came from `whisper_vad_init_from_file_with_params`
        // and is freed once, here.
        unsafe {
            sys::whisper_vad_free(self.0)
        }
    }
}

fn internal(what: &str) -> EngineError {
    EngineError::new("internal", format!("voice activity: {what} failed"))
}

/// `pcm` as whisper.cpp's samples: each one over 32768.
fn to_float(pcm: &[i16]) -> impl Iterator<Item = f32> + '_ {
    pcm.iter().map(|&s| f32::from(s) / 32768.0)
}

impl Vad {
    /// Loads the model at `path` on the CPU.
    pub fn load(path: &str) -> Result<Self, EngineError> {
        if !Path::new(path).is_file() {
            return Err(EngineError::new(
                "model_missing",
                format!("VAD model file not found: {path}"),
            ));
        }
        let c_path = CString::new(path)
            .map_err(|_| EngineError::new("bad_request", "VAD model path holds a NUL byte"))?;
        let params = sys::whisper_vad_context_params {
            n_threads: 4,
            use_gpu: false,
            gpu_device: 0,
        };
        #[allow(unsafe_code)]
        // SAFETY: `c_path` is a valid C string for the duration of the call.
        let ptr = unsafe { sys::whisper_vad_init_from_file_with_params(c_path.as_ptr(), params) };
        if ptr.is_null() {
            return Err(EngineError::new("load_failed", "VAD model load failed"));
        }
        Ok(Self {
            ctx: Context(ptr),
            stream: RefCell::new(None),
        })
    }

    /// The milliseconds of speech in `pcm` under `params`; with
    /// `continues`, in the take fed so far followed by `pcm`.
    pub fn detect(
        &self,
        pcm: &[i16],
        params: &DetectSpeechParams,
    ) -> Result<DetectSpeechResult, EngineError> {
        let stream = self.stream.borrow_mut().take();
        let (samples, fresh) = match (params.continues, stream) {
            (None, _) => (to_float(pcm).collect::<Vec<f32>>(), true),
            (Some(fed), Some(s)) if s.samples == fed => {
                let mut rest = s.pending;
                rest.extend(to_float(pcm));
                (rest, s.fresh)
            }
            (Some(fed), Some(s)) => {
                return Err(EngineError::new(
                    "bad_request",
                    format!("the speech stream holds {} samples, not {fed}", s.samples),
                ));
            }
            (Some(_), None) => {
                return Err(EngineError::new("bad_request", "no speech stream is open"));
            }
        };
        if fresh && samples.is_empty() {
            return Ok(DetectSpeechResult { speech_ms: 0 });
        }
        self.run(&samples, fresh)?;
        self.speech_ms(params)
    }

    /// Runs the next samples of a take; `start` begins a new one.
    pub fn feed(
        &self,
        pcm: &[i16],
        params: &FeedSpeechParams,
    ) -> Result<FeedSpeechResult, EngineError> {
        let mut slot = self.stream.borrow_mut();
        if params.start {
            *slot = Some(Stream {
                samples: 0,
                pending: Vec::new(),
                fresh: true,
            });
        }
        let Some(stream) = slot.as_mut() else {
            return Err(EngineError::new(
                "bad_request",
                "no speech stream is open; feed with start first",
            ));
        };
        stream.pending.extend(to_float(pcm));
        stream.samples += pcm.len() as u64;
        let whole = stream.pending.len() / WINDOW * WINDOW;
        if whole > 0 {
            if let Err(e) = self.run(&stream.pending[..whole], stream.fresh) {
                *slot = None;
                return Err(e);
            }
            stream.fresh = false;
            stream.pending.drain(..whole);
        }
        let voiced_until = params.threshold.map(|t| {
            if stream.fresh {
                0
            } else {
                self.voiced_until(t)
            }
        });
        Ok(FeedSpeechResult {
            samples: stream.samples,
            voiced_until,
        })
    }

    /// The end, in samples, of the last window run whose speech
    /// probability reached `threshold`; 0 when none did.
    fn voiced_until(&self, threshold: f64) -> u64 {
        #[allow(unsafe_code)]
        // SAFETY: the context is live; its probabilities are read in place.
        let probs = unsafe {
            let n = usize::try_from(sys::whisper_vad_n_probs(self.ctx.0)).unwrap_or(0);
            let p = sys::whisper_vad_probs(self.ctx.0);
            if p.is_null() || n == 0 {
                return 0;
            }
            std::slice::from_raw_parts(p, n)
        };
        probs
            .iter()
            .rposition(|&p| f64::from(p) >= threshold)
            .map_or(0, |i| ((i + 1) * WINDOW) as u64)
    }

    /// Runs the model over `samples`, starting afresh when `reset`.
    fn run(&self, samples: &[f32], reset: bool) -> Result<(), EngineError> {
        let n = c_int::try_from(samples.len()).map_err(|_| internal("a take that long"))?;
        #[allow(unsafe_code)]
        // SAFETY: the context is live and `samples` holds `n` floats.
        let ok = unsafe { dettivo_vad_detect_speech_more(self.ctx.0, samples.as_ptr(), n, reset) };
        ok.then_some(()).ok_or_else(|| internal("detection"))
    }

    /// The speech in every window run since the last reset.
    fn speech_ms(&self, params: &DetectSpeechParams) -> Result<DetectSpeechResult, EngineError> {
        let vad = sys::whisper_vad_params {
            threshold: params.threshold as f32,
            min_speech_duration_ms: c_int::try_from(params.min_speech_ms).unwrap_or(c_int::MAX),
            min_silence_duration_ms: 100,
            max_speech_duration_s: f32::MAX,
            speech_pad_ms: 30,
            samples_overlap: 0.1,
        };
        #[allow(unsafe_code)]
        // SAFETY: the context is live; the segments are read and freed here.
        let centiseconds = unsafe {
            let segments = sys::whisper_vad_segments_from_probs(self.ctx.0, vad);
            if segments.is_null() {
                return Err(internal("segmentation"));
            }
            let total: f32 = (0..sys::whisper_vad_segments_n_segments(segments))
                .map(|i| {
                    let start = sys::whisper_vad_segments_get_segment_t0(segments, i);
                    let end = sys::whisper_vad_segments_get_segment_t1(segments, i);
                    (end - start).max(0.0)
                })
                .sum();
            sys::whisper_vad_free_segments(segments);
            total
        };
        // whisper.cpp reports segment bounds in centiseconds.
        Ok(DetectSpeechResult {
            speech_ms: (centiseconds * 10.0).round() as u64,
        })
    }
}
