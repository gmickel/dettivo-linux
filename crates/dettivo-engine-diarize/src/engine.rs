//! The safe wrapper over sherpa-onnx's offline speaker diarization: a
//! model set (pyannote segmentation 3.0 for the turns, English ERes2Net
//! for the embeddings) loaded from one directory, one pass at
//! a time over 16 kHz samples with the pipeline's own chunk progress, and
//! turns labelled `SPEAKER_00`, `SPEAKER_01`, ... in start order. Every
//! `unsafe` block here is the one place the engine touches raw pointers.

#![allow(unsafe_code, reason = "the FFI boundary to sherpa-onnx lives here")]

use std::ffi::{CString, c_void};
use std::path::{Path, PathBuf};

use crate::provider::{self, Selection};
use dettivo_engine_proto::{
    Backend, DiarizeEngine, DiarizeParams, DiarizeResult, EngineError, LoadParams, LoadedResult,
    SpeakerTurn,
};
use sherpa_onnx_sys as ffi;

/// The segmentation model's file name inside the model directory.
pub const SEGMENTATION_FILE: &str = "segmentation.onnx";
/// The embedding model's file name inside the model directory.
pub const EMBEDDING_FILE: &str = "embedding.onnx";
/// The clustering distance threshold when no speaker count is known.
pub const DEFAULT_THRESHOLD: f32 = 0.6;
/// The most threads the engine takes by itself.
pub const MAX_DEFAULT_THREADS: u32 = 4;
/// A speaker turn shorter than this is dropped (the pipeline's own rule).
const MIN_DURATION_ON: f32 = 0.1;
/// A silence shorter than this joins the turns around it.
const MIN_DURATION_OFF: f32 = 0.1;

/// A loaded model set.
pub struct Diarizer {
    sd: *const ffi::SherpaOnnxOfflineSpeakerDiarization,
    dir: PathBuf,
    threads: u32,
    segmentation: CString,
    embedding: CString,
    provider: CString,
    selection: Selection,
}

// SAFETY: the diarizer is used from one thread at a time (the host runs
// one pass at a time and the CLI runs one); sherpa-onnx keeps no
// thread-local state behind the handle.
unsafe impl Send for Diarizer {}
unsafe impl Sync for Diarizer {}

/// The thread count a load without one gets: the cores, capped.
pub fn default_threads() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1)
        .clamp(1, MAX_DEFAULT_THREADS)
}

fn c_path(path: &Path) -> Result<CString, EngineError> {
    CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| EngineError::new("bad_request", "the model path holds a NUL byte"))
}

impl Diarizer {
    /// Loads the model set under `dir` with `threads` per model.
    pub fn open(dir: &Path, threads: u32, selection: Selection) -> Result<Self, EngineError> {
        for name in [SEGMENTATION_FILE, EMBEDDING_FILE] {
            let path = dir.join(name);
            if !path.is_file() {
                return Err(EngineError::new(
                    "model_missing",
                    format!("model file not found: {}", path.display()),
                ));
            }
        }
        let mut this = Self {
            sd: std::ptr::null(),
            dir: dir.to_path_buf(),
            threads: threads.max(1),
            segmentation: c_path(&dir.join(SEGMENTATION_FILE))?,
            embedding: c_path(&dir.join(EMBEDDING_FILE))?,
            provider: CString::new(if selection.backend == Backend::Cuda {
                "cuda"
            } else {
                "cpu"
            })
            .expect("static"),
            selection,
        };
        let config = this.config(-1, DEFAULT_THRESHOLD);
        // SAFETY: every pointer in `config` points at a CString this struct
        // owns for as long as the handle lives; NULL is the documented
        // answer for a model that does not load.
        this.sd = unsafe { ffi::SherpaOnnxCreateOfflineSpeakerDiarization(&config) };
        if this.sd.is_null() {
            return Err(EngineError::new(
                "load_failed",
                format!(
                    "sherpa-onnx could not load the model set under {}",
                    dir.display()
                ),
            ));
        }
        tracing::info!(
            model = %dir.display(),
            threads = this.threads,
            "model loaded backend={} reason={:?}", this.provider.to_string_lossy(), this.selection.reason
        );
        Ok(this)
    }

    fn config(
        &self,
        clusters: i32,
        threshold: f32,
    ) -> ffi::SherpaOnnxOfflineSpeakerDiarizationConfig {
        ffi::SherpaOnnxOfflineSpeakerDiarizationConfig {
            segmentation: ffi::SherpaOnnxOfflineSpeakerSegmentationModelConfig {
                pyannote: ffi::SherpaOnnxOfflineSpeakerSegmentationPyannoteModelConfig {
                    model: self.segmentation.as_ptr(),
                    window_shift_ratio: 0.1,
                },
                num_threads: self.threads as i32,
                debug: 0,
                provider: self.provider.as_ptr(),
            },
            embedding: ffi::SherpaOnnxSpeakerEmbeddingExtractorConfig {
                model: self.embedding.as_ptr(),
                num_threads: self.threads as i32,
                debug: 0,
                provider: self.provider.as_ptr(),
            },
            clustering: ffi::SherpaOnnxFastClusteringConfig {
                num_clusters: clusters,
                threshold,
            },
            min_duration_on: if clusters > 0 { 0.3 } else { MIN_DURATION_ON },
            min_duration_off: if clusters > 0 { 0.5 } else { MIN_DURATION_OFF },
        }
    }

    /// Diarizes `samples` (16 kHz mono, -1..1): `clusters` fixes the
    /// speaker count when known, `threshold` tunes the clustering
    /// otherwise, and `progress` hears `(completed, total)` chunks.
    pub fn run(
        &self,
        samples: &[f32],
        clusters: Option<u32>,
        threshold: Option<f32>,
        progress: &mut dyn FnMut(u32, u32),
    ) -> Result<Vec<SpeakerTurn>, EngineError> {
        if samples.is_empty() {
            return Err(EngineError::new("bad_request", "no audio to diarize"));
        }
        let clusters = clusters.map(|n| n as i32).filter(|n| *n > 0).unwrap_or(-1);
        let config = self.config(clusters, threshold.unwrap_or(DEFAULT_THRESHOLD));
        let mut callback: &mut dyn FnMut(u32, u32) = progress;
        let arg: *mut &mut dyn FnMut(u32, u32) = &mut callback;
        // SAFETY: the handle is live, the config's pointers are this
        // struct's CStrings, `samples` is a valid slice for the call, and
        // `arg` outlives the call that invokes the trampoline with it.
        let result = unsafe {
            ffi::SherpaOnnxOfflineSpeakerDiarizationSetConfig(self.sd, &config);
            ffi::SherpaOnnxOfflineSpeakerDiarizationProcessWithCallback(
                self.sd,
                samples.as_ptr(),
                samples.len() as i32,
                Some(trampoline),
                arg.cast::<c_void>(),
            )
        };
        if result.is_null() {
            return Err(EngineError::new(
                "internal",
                "sherpa-onnx returned no diarization result",
            ));
        }
        // SAFETY: `result` is a live result pointer; the segment array is
        // read for `n` entries and destroyed with the result afterwards.
        let turns = unsafe {
            let n = ffi::SherpaOnnxOfflineSpeakerDiarizationResultGetNumSegments(result);
            let segments = ffi::SherpaOnnxOfflineSpeakerDiarizationResultSortByStartTime(result);
            let mut turns = Vec::with_capacity(n.max(0) as usize);
            if !segments.is_null() && n > 0 {
                for s in std::slice::from_raw_parts(segments, n as usize) {
                    turns.push(SpeakerTurn {
                        start_ms: (f64::from(s.start) * 1000.0).round().max(0.0) as u64,
                        end_ms: (f64::from(s.end) * 1000.0).round().max(0.0) as u64,
                        speaker: format!("SPEAKER_{:02}", s.speaker.max(0)),
                    });
                }
                ffi::SherpaOnnxOfflineSpeakerDiarizationDestroySegment(segments);
            }
            ffi::SherpaOnnxOfflineSpeakerDiarizationDestroyResult(result);
            turns
        };
        tracing::info!(turns = turns.len(), "diarization done");
        Ok(turns)
    }
}

impl Drop for Diarizer {
    fn drop(&mut self) {
        if !self.sd.is_null() {
            // SAFETY: the handle was created by this struct and is destroyed once.
            unsafe { ffi::SherpaOnnxDestroyOfflineSpeakerDiarization(self.sd) };
        }
    }
}

unsafe extern "C" fn trampoline(done: i32, total: i32, arg: *mut c_void) -> i32 {
    // SAFETY: `arg` is the `&mut &mut dyn FnMut` `run` passed, live for the call.
    let callback = unsafe { &mut *arg.cast::<&mut dyn FnMut(u32, u32)>() };
    callback(done.max(0) as u32, total.max(0) as u32);
    0
}

/// Converts 16-bit samples to the floats sherpa-onnx reads.
pub fn to_f32(pcm: &[i16]) -> Vec<f32> {
    pcm.iter().map(|s| f32::from(*s) / 32_768.0).collect()
}

impl DiarizeEngine for Diarizer {
    fn load(params: &LoadParams, force_cpu: bool) -> Result<Self, EngineError> {
        let threads = params
            .threads
            .filter(|t| *t > 0)
            .unwrap_or_else(default_threads);
        let selection = provider::choose(params.backend_preference, force_cpu)?;
        Self::open(Path::new(&params.model), threads, selection)
    }

    fn loaded(&self) -> LoadedResult {
        LoadedResult {
            model: self.dir.to_string_lossy().into_owned(),
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
        let turns = self.run(
            &to_f32(pcm),
            params.speakers,
            params.clustering_threshold.map(|t| t as f32),
            progress,
        )?;
        Ok(DiarizeResult { turns })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_count_mode_restores_its_duration_settings() {
        let model = std::env::var_os("DETTIVO_TEST_DIARIZATION_MODEL")
            .map(PathBuf::from)
            .or_else(|| {
                let data = std::env::var_os("XDG_DATA_HOME")
                    .map(PathBuf::from)
                    .or_else(|| {
                        std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share"))
                    })?;
                Some(data.join("dettivo/models/diarize/diarization-en"))
            });
        let Some(model) = model.filter(|p| p.join(EMBEDDING_FILE).is_file()) else {
            eprintln!("skip: the calibrated diarization model is not downloaded");
            return;
        };
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../dettivo-qa/fixtures/diarization/two-speakers.wav");
        let pcm = dettivo_engine_proto::host::read_wav(&fixture).unwrap();
        let phrase = &pcm[700 * 16..3991 * 16];
        let mut repeated = Vec::new();
        for i in 0..4 {
            if i > 0 {
                repeated.extend(std::iter::repeat_n(0, 350 * 16));
            }
            repeated.extend_from_slice(phrase);
        }
        let samples = to_f32(&repeated);
        let diarizer = Diarizer::open(&model, 1, Selection::cpu("test")).unwrap();
        let automatic = diarizer.run(&samples, None, None, &mut |_, _| {}).unwrap();
        let fixed = diarizer
            .run(&samples, Some(1), None, &mut |_, _| {})
            .unwrap();
        assert_eq!(automatic.len(), 4);
        assert_eq!(fixed.len(), 1);
        assert_eq!(
            diarizer.run(&samples, None, None, &mut |_, _| {}).unwrap(),
            automatic
        );
        assert_eq!(
            diarizer
                .run(&samples, Some(1), None, &mut |_, _| {})
                .unwrap(),
            fixed
        );
    }

    #[test]
    fn a_missing_file_names_its_path_and_samples_convert() {
        let dir = tempfile::tempdir().unwrap();
        let err = Diarizer::open(dir.path(), 1, Selection::cpu("test"))
            .err()
            .unwrap();
        assert_eq!(err.code, "model_missing");
        assert!(err.message.contains(SEGMENTATION_FILE), "{err}");
        std::fs::write(dir.path().join(SEGMENTATION_FILE), b"x").unwrap();
        let err = Diarizer::open(dir.path(), 1, Selection::cpu("test"))
            .err()
            .unwrap();
        assert!(err.message.contains(EMBEDDING_FILE), "{err}");
        assert_eq!(to_f32(&[0, 16_384, -32_768]), [0.0, 0.5, -1.0]);
        assert!((1..=MAX_DEFAULT_THREADS).contains(&default_threads()));
    }
}
