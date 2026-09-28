//! Voice embeddings per time span, from the same ERes2Net model the
//! diarization pass clusters with. The extractor is created on the first
//! `embed` with the model set's threads and provider, kept for the
//! diarizer's life and destroyed with it. A span is clamped to the
//! samples; one shorter than [`MIN_SPAN_MS`] afterwards, or one the model
//! does not call ready, gets no vector. Every vector comes back unit
//! length, so a dot product is the cosine similarity.

#![allow(unsafe_code, reason = "the FFI boundary to sherpa-onnx lives here")]

use dettivo_engine_proto::{EmbedParams, EmbedResult, EngineError, Span};
use sherpa_onnx_sys as ffi;

use crate::engine::{Diarizer, to_f32};

/// The shortest clamped span that gets a vector.
pub const MIN_SPAN_MS: u64 = 500;
/// The sample rate of every attachment.
const SAMPLE_RATE: u64 = 16_000;

/// A live speaker embedding extractor.
pub struct Extractor {
    handle: *const ffi::SherpaOnnxSpeakerEmbeddingExtractor,
    dim: usize,
}

// SAFETY: the extractor sits behind the diarizer's mutex, so one thread
// at a time uses it; sherpa-onnx keeps no thread-local state behind it.
unsafe impl Send for Extractor {}

impl Extractor {
    /// Loads the embedding model the diarizer's model set names.
    fn open(diarizer: &Diarizer) -> Result<Self, EngineError> {
        let config = ffi::SherpaOnnxSpeakerEmbeddingExtractorConfig {
            model: diarizer.embedding.as_ptr(),
            num_threads: diarizer.threads as i32,
            debug: 0,
            provider: diarizer.provider.as_ptr(),
        };
        // SAFETY: the config's pointers are the diarizer's CStrings, live
        // for the call; NULL is the documented answer for a failed load.
        let handle = unsafe { ffi::SherpaOnnxCreateSpeakerEmbeddingExtractor(&config) };
        if handle.is_null() {
            return Err(EngineError::new(
                "load_failed",
                "sherpa-onnx could not load the speaker embedding model",
            ));
        }
        // SAFETY: `handle` is the live extractor just created.
        let dim = unsafe { ffi::SherpaOnnxSpeakerEmbeddingExtractorDim(handle) };
        let this = Self {
            handle,
            dim: dim.max(0) as usize,
        };
        if this.dim == 0 {
            return Err(EngineError::new(
                "load_failed",
                "the speaker embedding model reports no dimension",
            ));
        }
        tracing::info!(dim = this.dim, "speaker embedding extractor loaded");
        Ok(this)
    }

    /// The unit-length vector for `samples`, or `None` when the model
    /// needs more audio than they hold.
    fn embed(&self, samples: &[f32]) -> Option<Vec<f32>> {
        // SAFETY: the extractor is live; the stream it returns is fed
        // `samples` (a valid slice for the call), read once and destroyed
        // here, and the embedding holds `dim` floats until it is freed.
        let raw = unsafe {
            let stream = ffi::SherpaOnnxSpeakerEmbeddingExtractorCreateStream(self.handle);
            if stream.is_null() {
                return None;
            }
            ffi::SherpaOnnxOnlineStreamAcceptWaveform(
                stream,
                SAMPLE_RATE as i32,
                samples.as_ptr(),
                samples.len() as i32,
            );
            ffi::SherpaOnnxOnlineStreamInputFinished(stream);
            let mut raw = None;
            if ffi::SherpaOnnxSpeakerEmbeddingExtractorIsReady(self.handle, stream) != 0 {
                let v =
                    ffi::SherpaOnnxSpeakerEmbeddingExtractorComputeEmbedding(self.handle, stream);
                if !v.is_null() {
                    raw = Some(std::slice::from_raw_parts(v, self.dim).to_vec());
                    ffi::SherpaOnnxSpeakerEmbeddingExtractorDestroyEmbedding(v);
                }
            }
            ffi::SherpaOnnxDestroyOnlineStream(stream);
            raw
        };
        raw.and_then(normalised)
    }
}

impl Drop for Extractor {
    fn drop(&mut self) {
        // SAFETY: the handle was created by `open` and is destroyed once.
        unsafe { ffi::SherpaOnnxDestroySpeakerEmbeddingExtractor(self.handle) };
    }
}

/// `v` scaled to unit length; `None` for a zero or non-finite vector.
fn normalised(mut v: Vec<f32>) -> Option<Vec<f32>> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if !norm.is_finite() || norm == 0.0 {
        return None;
    }
    v.iter_mut().for_each(|x| *x /= norm);
    Some(v)
}

/// The sample range `span` covers in `len` samples, when it is long enough.
fn range(span: &Span, len: usize) -> Option<std::ops::Range<usize>> {
    let at = |ms: u64| ((ms.saturating_mul(SAMPLE_RATE) / 1000) as usize).min(len);
    let (start, end) = (at(span.start_ms), at(span.end_ms));
    let min = (MIN_SPAN_MS * SAMPLE_RATE / 1000) as usize;
    (end.saturating_sub(start) >= min).then_some(start..end)
}

impl Diarizer {
    /// One unit-length embedding per span of `pcm` (16 kHz mono).
    pub fn embed_spans(
        &self,
        pcm: &[i16],
        params: &EmbedParams,
    ) -> Result<EmbedResult, EngineError> {
        let mut slot = self
            .embedder
            .lock()
            .map_err(|_| EngineError::new("internal", "the embedding extractor is poisoned"))?;
        if slot.is_none() {
            *slot = Some(Extractor::open(self)?);
        }
        let extractor = slot.as_ref().expect("created above");
        let embeddings = params
            .spans
            .iter()
            .map(|span| range(span, pcm.len()).and_then(|r| extractor.embed(&to_f32(&pcm[r]))))
            .collect();
        Ok(EmbedResult { embeddings })
    }
}

#[cfg(test)]
#[path = "embed_tests.rs"]
mod tests;
