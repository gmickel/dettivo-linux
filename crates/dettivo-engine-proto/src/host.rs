//! The engine side of the protocol, shared by every speech engine binary:
//! frames in on stdin, frames out on stdout, one request at a time (the
//! supervisor serialises). `cancel` is acknowledged and, since recognition
//! runs to completion synchronously, takes effect on the next request;
//! `status` and `unload` work in any state; an unknown request is answered
//! with the protocol's error shape. Also the WAV reader the CLI modes share.

use std::io::{self, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::ExitCode;

use serde_json::{Value, json};

use crate::frame::{Frame, FrameError, bytes_to_pcm, read_frame, write_frame};
use crate::messages::{
    CancelParams, Empty, ErrorEvent, LoadParams, LoadedResult, RecognizeParams, RecognizeResult,
    StatusResult,
};

/// Why loading or recognizing failed; `code` is a protocol error code.
#[derive(Debug)]
pub struct EngineError {
    /// `model_missing`, `load_failed`, `bad_request`, `internal`.
    pub code: &'static str,
    /// A message with no transcript, prompt or audio in it.
    pub message: String,
}

impl EngineError {
    /// An error with `code` and `message`.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for EngineError {}

/// What an engine binary implements; the host loop does the rest.
pub trait SpeechEngine: Sized {
    /// Loads the model named by `params` on the backend it asks for.
    fn load(params: &LoadParams, force_cpu: bool) -> Result<Self, EngineError>;
    /// The `loaded` payload (model, backend, reason).
    fn loaded(&self) -> LoadedResult;
    /// Recognizes 16 kHz mono samples, reporting progress in 0..1.
    fn recognize(
        &self,
        pcm: &[i16],
        params: &RecognizeParams,
        progress: &mut dyn FnMut(f64),
    ) -> Result<RecognizeResult, EngineError>;
}

fn error_frame(id: u64, code: &str, message: &str) -> Frame {
    Frame::response(
        id,
        "error",
        serde_json::to_value(ErrorEvent {
            request_id: id,
            code: code.to_string(),
            message: message.to_string(),
        })
        .unwrap_or(Value::Null),
    )
}

fn send<W: Write>(out: &mut W, frame: &Frame) -> bool {
    write_frame(out, frame, &[]).is_ok()
}

/// Runs the protocol on stdin/stdout until stdin closes.
pub fn serve<E: SpeechEngine>(force_cpu: bool) -> ExitCode {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut out = BufWriter::new(stdout.lock());
    let mut engine: Option<E> = None;
    tracing::info!("engine ready");
    loop {
        let (frame, attachments) = match read_frame(&mut reader) {
            Ok(x) => x,
            Err(FrameError::Eof) => break,
            Err(FrameError::Malformed(m)) => {
                tracing::warn!(detail = %m, "malformed frame");
                if !send(
                    &mut out,
                    &Frame::event(
                        0,
                        "error",
                        json!({"request_id": 0, "code": "bad_request", "message": m}),
                    ),
                ) {
                    break;
                }
                continue;
            }
            Err(FrameError::Io(e)) => {
                tracing::warn!(error = %e, "transport closed");
                break;
            }
        };
        let id = frame.id;
        let reply = match frame.name.as_str() {
            "load" => match serde_json::from_value::<LoadParams>(frame.payload.clone()) {
                Ok(p) => {
                    drop(engine.take());
                    match E::load(&p, force_cpu) {
                        Ok(e) => {
                            let loaded = e.loaded();
                            engine = Some(e);
                            let _ = send(
                                &mut out,
                                &Frame::event(
                                    0,
                                    "loaded",
                                    serde_json::to_value(&loaded).unwrap_or(Value::Null),
                                ),
                            );
                            Frame::response(
                                id,
                                "load",
                                serde_json::to_value(&loaded).unwrap_or(Value::Null),
                            )
                        }
                        Err(EngineError { code, message }) => error_frame(id, code, &message),
                    }
                }
                Err(e) => error_frame(id, "bad_request", &e.to_string()),
            },
            "unload" => {
                engine = None;
                tracing::info!("model unloaded");
                Frame::response(id, "unload", json!({}))
            }
            "status" => {
                let loaded = engine.as_ref().map(E::loaded);
                Frame::response(
                    id,
                    "status",
                    serde_json::to_value(StatusResult {
                        loaded: loaded.is_some(),
                        model: loaded.as_ref().map(|l| l.model.clone()),
                        backend: loaded.as_ref().map(|l| l.backend),
                        busy: false,
                        memory_bytes: None,
                    })
                    .unwrap_or(Value::Null),
                )
            }
            "cancel" => match serde_json::from_value::<CancelParams>(frame.payload.clone()) {
                Ok(_) => Frame::response(
                    id,
                    "cancel",
                    serde_json::to_value(Empty {}).unwrap_or(Value::Null),
                ),
                Err(e) => error_frame(id, "bad_request", &e.to_string()),
            },
            "recognize" => match (
                &engine,
                serde_json::from_value::<RecognizeParams>(frame.payload.clone()),
            ) {
                (None, _) => error_frame(id, "bad_request", "no model is loaded; send load first"),
                (_, Err(e)) => error_frame(id, "bad_request", &e.to_string()),
                (Some(e), Ok(p)) => {
                    let (Some(bytes), 1) = (attachments.first(), attachments.len()) else {
                        let f = error_frame(
                            id,
                            "bad_request",
                            "recognize needs exactly one pcm16k attachment",
                        );
                        if !send(&mut out, &f) {
                            break;
                        }
                        continue;
                    };
                    let pcm = bytes_to_pcm(bytes);
                    let mut progress = |fraction: f64| {
                        let _ = send(
                            &mut out,
                            &Frame::event(
                                id,
                                "progress",
                                json!({"request_id": id, "fraction": fraction}),
                            ),
                        );
                    };
                    progress(0.0);
                    match e.recognize(&pcm, &p, &mut progress) {
                        Ok(result) => {
                            progress(1.0);
                            Frame::response(
                                id,
                                "recognize",
                                serde_json::to_value(&result).unwrap_or(Value::Null),
                            )
                        }
                        Err(EngineError { code, message }) => error_frame(id, code, &message),
                    }
                }
            },
            "generate" | "diarize" => error_frame(
                id,
                "bad_request",
                "this engine does not implement that request",
            ),
            other => {
                tracing::warn!("unknown request");
                error_frame(id, "bad_request", &format!("unknown request {other:?}"))
            }
        };
        if !send(&mut out, &reply) {
            break;
        }
    }
    tracing::info!("engine exiting");
    ExitCode::SUCCESS
}

/// Reads a WAV into 16 kHz mono i16 (channels averaged, rate converted).
pub fn read_wav(path: &Path) -> Result<Vec<i16>, EngineError> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|e| EngineError::new("bad_request", format!("{}: {e}", path.display())))?;
    let spec = reader.spec();
    let channels = usize::from(spec.channels).max(1);
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .filter_map(Result::ok)
                .map(|s| s as f32 / scale)
                .collect()
        }
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(Result::ok).collect(),
    };
    let mono: Vec<f32> = samples
        .chunks(channels)
        .map(|c| c.iter().sum::<f32>() / channels as f32)
        .collect();
    let resampled: Vec<f32> = if spec.sample_rate == 16_000 {
        mono
    } else {
        let ratio = f64::from(spec.sample_rate) / 16_000.0;
        let out_len = (mono.len() as f64 / ratio) as usize;
        (0..out_len)
            .map(|i| {
                let pos = i as f64 * ratio;
                let a = pos.floor() as usize;
                let b = (a + 1).min(mono.len().saturating_sub(1));
                let t = (pos - a as f64) as f32;
                mono.get(a).copied().unwrap_or(0.0) * (1.0 - t)
                    + mono.get(b).copied().unwrap_or(0.0) * t
            })
            .collect()
    };
    Ok(resampled
        .into_iter()
        .map(|v| (v.clamp(-1.0, 1.0) * 32_767.0) as i16)
        .collect())
}

/// CLI mode: loads the model, recognizes the WAV and returns the same JSON
/// the protocol's `recognize` response carries.
pub fn run_cli<E: SpeechEngine>(
    wav: &Path,
    load: &LoadParams,
    params: &RecognizeParams,
    force_cpu: bool,
) -> Result<String, EngineError> {
    let pcm = read_wav(wav)?;
    let engine = E::load(load, force_cpu)?;
    let result = engine.recognize(&pcm, params, &mut |_| {})?;
    serde_json::to_string_pretty(&result).map_err(|e| EngineError::new("internal", e.to_string()))
}

/// Whether an environment flag such as `DETTIVO_FORCE_CPU` is on.
pub fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| matches!(v.trim(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stereo_wav_at_another_rate_lands_as_16k_mono() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 32_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for _ in 0..3200 {
            w.write_sample(1000i16).unwrap();
            w.write_sample(3000i16).unwrap();
        }
        w.finalize().unwrap();
        let pcm = read_wav(&path).unwrap();
        assert_eq!(pcm.len(), 1600);
        assert!((pcm[10] - 2000).abs() <= 1, "{}", pcm[10]);
        let err = read_wav(&dir.path().join("missing.wav")).unwrap_err();
        assert_eq!(err.code, "bad_request");
        assert!(err.message.contains("missing.wav"));
    }
}
