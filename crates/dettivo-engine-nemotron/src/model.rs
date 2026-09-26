//! The safe wrapper over NeMo-Speech.cpp's diarization C ABI: a model
//! created on the CPU or the build's GPU backend, and one streaming pass
//! over 16 kHz samples that returns every frame's speaker probabilities.
//! Every `unsafe` block here is the one place the engine touches raw
//! pointers.

#![allow(unsafe_code, reason = "the FFI boundary to NeMo-Speech.cpp lives here")]

use std::ffi::{CStr, CString};
use std::path::Path;

use nemo_speech_cpp_sys as ffi;

/// The streaming geometry, in 80 ms encoder frames: transformers'
/// `Nemotron3DiarizationSpeakerCache` as fn-64's reference port runs it
/// (340-frame chunks, 40 frames of look-ahead, a 40-frame FIFO, the
/// 264-frame speaker cache refreshed every 300 frames, no left context).
const CHUNK: i32 = 340;
const RIGHT_CONTEXT: i32 = 40;
const LEFT_CONTEXT: i32 = 0;
const FIFO: i32 = 40;
const SPEAKER_CACHE: i32 = 264;
const UPDATE_PERIOD: i32 = 300;

/// Audio pushed per step (60 s): the pass reports progress and reads the
/// new frames between pushes, before the stream's compaction (after about
/// 20 minutes) drops the oldest raw probabilities.
pub const PUSH_SAMPLES: usize = 60 * 16_000;

/// The library's reason for the last failed call on this thread.
fn last_error() -> String {
    // SAFETY: the library returns a thread-local NUL-terminated string (or
    // NULL), valid until the next call on this thread.
    unsafe {
        let error = ffi::nemo_speech_asr_last_error();
        if error.is_null() {
            "unknown NeMo-Speech.cpp error".into()
        } else {
            CStr::from_ptr(error).to_string_lossy().into_owned()
        }
    }
}

fn check(status: ffi::nemo_speech_asr_status) -> Result<(), String> {
    if status == ffi::nemo_speech_asr_status_NEMO_SPEECH_ASR_OK {
        Ok(())
    } else {
        Err(last_error())
    }
}

/// The description of the GPU a model created with `gpu = index` loads on:
/// the `index`-th GPU or integrated GPU in ggml's device registry.
pub fn gpu_name(index: i32) -> Option<String> {
    let mut seen = 0;
    // SAFETY: the registry calls take no pointers but the device handles
    // they return, which live as long as the process; the description is
    // a NUL-terminated string the device owns.
    unsafe {
        for i in 0..ffi::ggml_backend_dev_count() {
            let device = ffi::ggml_backend_dev_get(i);
            let kind = ffi::ggml_backend_dev_type(device);
            if kind != ffi::ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_GPU
                && kind != ffi::ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_IGPU
            {
                continue;
            }
            if seen == index {
                let description = ffi::ggml_backend_dev_description(device);
                return (!description.is_null())
                    .then(|| CStr::from_ptr(description).to_string_lossy().into_owned());
            }
            seen += 1;
        }
    }
    None
}

/// A loaded model.
pub struct Model {
    raw: *mut ffi::nemo_speech_diar_model,
    speakers: u32,
    frame_ms: u32,
}

// SAFETY: the model is used from one thread at a time (the host runs one
// pass at a time, the CLI one); the library serialises compute internally
// and documents independent streams as usable from different threads.
unsafe impl Send for Model {}
unsafe impl Sync for Model {}

/// One stream, closed when dropped.
struct Stream(*mut ffi::nemo_speech_diar_stream);

impl Drop for Stream {
    fn drop(&mut self) {
        // SAFETY: the stream was opened by `probabilities` and is closed once.
        unsafe { ffi::nemo_speech_diar_stream_close(self.0) };
    }
}

impl Model {
    /// Loads the GGUF at `path` on GPU device `gpu` of the build's backend,
    /// or on the CPU for `-1`.
    pub fn open(path: &Path, gpu: i32) -> Result<Self, String> {
        let c_path = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| "the model path holds a NUL byte".to_string())?;
        let config = ffi::nemo_speech_diar_model_config {
            size: std::mem::size_of::<ffi::nemo_speech_diar_model_config>(),
            model_path: c_path.as_ptr(),
            gpu,
            preset: std::ptr::null(),
            chunk_frames: CHUNK,
            right_context_frames: RIGHT_CONTEXT,
            left_context_frames: LEFT_CONTEXT,
            fifo_frames: FIFO,
            spkcache_frames: SPEAKER_CACHE,
            update_period_frames: UPDATE_PERIOD,
        };
        let mut raw = std::ptr::null_mut();
        // SAFETY: `config` and the path it points at outlive the call, and
        // `raw` is a valid out pointer the library sets on success.
        check(unsafe { ffi::nemo_speech_diar_create(&config, &mut raw) })?;
        // SAFETY: `raw` is the live model just created.
        let (speakers, seconds) = unsafe {
            (
                ffi::nemo_speech_diar_num_speakers(raw),
                ffi::nemo_speech_diar_seconds_per_frame(raw),
            )
        };
        Ok(Self {
            raw,
            speakers: speakers.max(0) as u32,
            frame_ms: (seconds * 1000.0).round() as u32,
        })
    }

    /// Speaker channels (eight for Nemotron 3 Diarization).
    pub fn speakers(&self) -> u32 {
        self.speakers
    }

    /// Milliseconds per output frame (10).
    pub fn frame_ms(&self) -> u32 {
        self.frame_ms
    }

    /// One streaming pass over `samples` (16 kHz mono, -1..1): every
    /// frame's probabilities, frame-major, `speakers` per frame.
    /// `progress` hears `(pushed, total)` pieces of [`PUSH_SAMPLES`].
    pub fn probabilities(
        &self,
        samples: &[f32],
        progress: &mut dyn FnMut(u32, u32),
    ) -> Result<Vec<f32>, String> {
        let mut raw = std::ptr::null_mut();
        // SAFETY: the model is live and `raw` is a valid out pointer.
        check(unsafe { ffi::nemo_speech_diar_stream_open(self.raw, &mut raw) })?;
        let stream = Stream(raw);
        let total = samples.len().div_ceil(PUSH_SAMPLES).max(1) as u32;
        let mut out = Vec::new();
        let mut read = 0i64;
        progress(0, total);
        for (i, piece) in samples.chunks(PUSH_SAMPLES).enumerate() {
            // SAFETY: the stream is live and `piece` is valid for the call.
            check(unsafe {
                ffi::nemo_speech_diar_stream_push_f32(stream.0, piece.as_ptr(), piece.len(), 16_000)
            })?;
            self.collect(&stream, &mut read, &mut out)?;
            progress(i as u32 + 1, total);
        }
        // SAFETY: the stream is live.
        check(unsafe { ffi::nemo_speech_diar_stream_finish(stream.0) })?;
        self.collect(&stream, &mut read, &mut out)?;
        Ok(out)
    }

    /// Appends the frames labelled since `read` to `out`.
    fn collect(&self, stream: &Stream, read: &mut i64, out: &mut Vec<f32>) -> Result<(), String> {
        let n = self.speakers as usize;
        // SAFETY: the stream is live; the two counters take no pointers.
        let (count, start) = unsafe {
            (
                ffi::nemo_speech_diar_frame_count(stream.0),
                ffi::nemo_speech_diar_frame_probs_start(stream.0),
            )
        };
        if start > *read {
            return Err(format!(
                "frames {read}..{start} were compacted before they were read"
            ));
        }
        if count <= *read {
            return Ok(());
        }
        let mut retained = vec![0f32; (count - start) as usize * n];
        // SAFETY: `retained` holds exactly the retained window's floats.
        check(unsafe {
            ffi::nemo_speech_diar_frame_probs(stream.0, retained.as_mut_ptr(), retained.len())
        })?;
        out.extend_from_slice(&retained[(*read - start) as usize * n..]);
        *read = count;
        Ok(())
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        // SAFETY: the model was created by `open` and is destroyed once.
        unsafe { ffi::nemo_speech_diar_destroy(self.raw) };
    }
}
