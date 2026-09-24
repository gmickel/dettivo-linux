//! The engine side of the protocol for the diarization binary. A `diarize`
//! pass runs on its own thread while this loop keeps reading frames, so a
//! `cancel` for the running request is answered at once (the pass ends in
//! the background and its result is dropped), a `status` meanwhile says
//! `busy`, and any other request waits its turn. Progress arrives from the
//! pass as chunk counts and goes out as `progress` events with
//! `completed` and `total` beside the fraction.

use std::collections::VecDeque;
use std::io::{self, BufWriter, Write};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::frame::{Frame, FrameError, bytes_to_pcm, read_frame, write_frame};
use crate::host::EngineError;
use crate::messages::{
    BackendPreference, CancelParams, DiarizeParams, DiarizeResult, Empty, ErrorEvent, LoadParams,
    LoadedResult, ProgressEvent, StatusResult,
};

/// What a diarization binary implements; the host loop does the rest.
pub trait DiarizeEngine: Sized + Send + Sync + 'static {
    /// Loads the model set named by `params` (a directory).
    fn load(params: &LoadParams, force_cpu: bool) -> Result<Self, EngineError>;
    /// The `loaded` payload (model, backend, reason).
    fn loaded(&self) -> LoadedResult;
    /// Diarizes 16 kHz mono samples, reporting `(completed, total)` chunks
    /// as the pass goes.
    fn diarize(
        &self,
        pcm: &[i16],
        params: &DiarizeParams,
        progress: &mut dyn FnMut(u32, u32),
    ) -> Result<DiarizeResult, EngineError>;
}

/// How often at most a progress event goes out.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// One decoded frame with its attachments, or the reader's failure
/// (`None` is end of stream).
type Incoming = Option<Result<(Frame, Vec<Vec<u8>>), String>>;

/// Frames from stdin, one thread ahead of the pass.
fn reader_thread() -> Receiver<Incoming> {
    let (tx, rx) = mpsc::channel::<Incoming>();
    std::thread::Builder::new()
        .name("dettivo-engine-stdin".into())
        .spawn(move || {
            let stdin = io::stdin();
            let mut reader = io::BufReader::new(stdin.lock());
            loop {
                let item = match read_frame(&mut reader) {
                    Ok(framed) => Some(Ok(framed)),
                    Err(FrameError::Eof) => None,
                    Err(FrameError::Malformed(m)) => Some(Err(m)),
                    Err(FrameError::Io(e)) => {
                        tracing::warn!(error = %e, "transport closed");
                        None
                    }
                };
                let stop = item.is_none();
                if tx.send(item).is_err() || stop {
                    break;
                }
            }
        })
        .expect("stdin reader thread");
    rx
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

/// What the pass thread sends back.
enum Update {
    Progress(u32, u32),
    Done(Result<DiarizeResult, EngineError>),
}

struct Host<E, W: Write> {
    out: W,
    engine: Option<Arc<E>>,
    /// Requests that arrived during a pass, answered after it.
    pending: VecDeque<(Frame, Vec<Vec<u8>>)>,
}

impl<E: DiarizeEngine, W: Write> Host<E, W> {
    fn send(&mut self, frame: &Frame) -> bool {
        write_frame(&mut self.out, frame, &[]).is_ok()
    }

    fn status(&self, id: u64, busy: bool) -> Frame {
        let loaded = self.engine.as_ref().map(|e| e.loaded());
        Frame::response(
            id,
            "status",
            serde_json::to_value(StatusResult {
                loaded: loaded.is_some(),
                model: loaded.as_ref().map(|l| l.model.clone()),
                backend: loaded.as_ref().map(|l| l.backend),
                busy,
                memory_bytes: None,
            })
            .unwrap_or(Value::Null),
        )
    }

    /// Answers one request. Returns false when stdout is gone.
    fn handle(
        &mut self,
        frame: Frame,
        attachments: Vec<Vec<u8>>,
        force_cpu: bool,
        rx: &Receiver<Incoming>,
    ) -> bool {
        let id = frame.id;
        let reply = match frame.name.as_str() {
            "load" => match serde_json::from_value::<LoadParams>(frame.payload.clone()) {
                Ok(p) => {
                    drop(self.engine.take());
                    match E::load(&p, force_cpu) {
                        Ok(e) => {
                            let loaded = e.loaded();
                            self.engine = Some(Arc::new(e));
                            let payload = serde_json::to_value(&loaded).unwrap_or(Value::Null);
                            if !self.send(&Frame::event(0, "loaded", payload.clone())) {
                                return false;
                            }
                            Frame::response(id, "load", payload)
                        }
                        Err(EngineError { code, message }) => error_frame(id, code, &message),
                    }
                }
                Err(e) => error_frame(id, "bad_request", &e.to_string()),
            },
            "unload" => {
                self.engine = None;
                tracing::info!("model unloaded");
                Frame::response(id, "unload", json!({}))
            }
            "status" => self.status(id, false),
            "cancel" => match serde_json::from_value::<CancelParams>(frame.payload.clone()) {
                Ok(_) => Frame::response(
                    id,
                    "cancel",
                    serde_json::to_value(Empty {}).unwrap_or(Value::Null),
                ),
                Err(e) => error_frame(id, "bad_request", &e.to_string()),
            },
            "diarize" => match serde_json::from_value::<DiarizeParams>(frame.payload.clone()) {
                Err(e) => error_frame(id, "bad_request", &e.to_string()),
                Ok(_) if self.engine.is_none() => {
                    error_frame(id, "bad_request", "no model is loaded; send load first")
                }
                Ok(_) if attachments.len() != 1 => error_frame(
                    id,
                    "bad_request",
                    "diarize needs exactly one pcm16k attachment",
                ),
                Ok(p) => return self.diarize(id, p, bytes_to_pcm(&attachments[0]), rx),
            },
            "recognize" | "generate" => error_frame(
                id,
                "bad_request",
                "this engine does not implement that request",
            ),
            other => {
                tracing::warn!("unknown request");
                error_frame(id, "bad_request", &format!("unknown request {other:?}"))
            }
        };
        self.send(&reply)
    }

    /// Runs one pass on its own thread, answering `cancel` (for this
    /// request) and `status` from the channel meanwhile and queueing the
    /// rest. Returns false when stdout is gone.
    fn diarize(
        &mut self,
        id: u64,
        params: DiarizeParams,
        pcm: Vec<i16>,
        rx: &Receiver<Incoming>,
    ) -> bool {
        let engine = self.engine.clone().expect("checked by the caller");
        let (tx, updates) = mpsc::channel::<Update>();
        let worker = std::thread::Builder::new()
            .name("dettivo-engine-diarize-pass".into())
            .spawn(move || {
                let progress_tx = tx.clone();
                let mut progress = |done: u32, total: u32| {
                    let _ = progress_tx.send(Update::Progress(done, total));
                };
                let result = engine.diarize(&pcm, &params, &mut progress);
                let _ = tx.send(Update::Done(result));
            });
        if let Err(e) = worker {
            return self.send(&error_frame(id, "internal", &e.to_string()));
        }
        let mut ok = true;
        let mut cancelled = false;
        let mut closed = false;
        let mut last_progress = Instant::now() - PROGRESS_INTERVAL;
        if !self.send(&progress_frame(id, 0, 0)) {
            ok = false;
        }
        loop {
            // Frames that arrived meanwhile: a cancel for this request ends
            // it now, a status answers busy, the rest wait.
            loop {
                match rx.try_recv() {
                    Ok(Some(Ok((f, _)))) if f.name == "cancel" => {
                        let target = serde_json::from_value::<CancelParams>(f.payload.clone())
                            .map(|c| c.request_id)
                            .unwrap_or(0);
                        if target == id && !cancelled {
                            cancelled = true;
                            tracing::info!(
                                "diarization cancelled; the pass ends in the background"
                            );
                            ok &= self.send(&error_frame(id, "cancelled", "cancelled"));
                        }
                        self.pending.push_back((f, Vec::new()));
                    }
                    Ok(Some(Ok((f, _)))) if f.name == "status" => {
                        let s = self.status(f.id, true);
                        ok &= self.send(&s);
                    }
                    Ok(Some(Ok((f, a)))) => self.pending.push_back((f, a)),
                    Ok(Some(Err(m))) => {
                        ok &= self.send(&Frame::event(
                            0,
                            "error",
                            json!({"request_id": 0, "code": "bad_request", "message": m}),
                        ));
                    }
                    Ok(None) | Err(TryRecvError::Disconnected) => {
                        closed = true;
                        cancelled = true;
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
            match updates.recv_timeout(Duration::from_millis(20)) {
                Ok(Update::Progress(done, total)) => {
                    if !cancelled && (done == total || last_progress.elapsed() >= PROGRESS_INTERVAL)
                    {
                        last_progress = Instant::now();
                        ok &= self.send(&progress_frame(id, done, total));
                    }
                }
                Ok(Update::Done(result)) => {
                    if !cancelled {
                        let reply = match result {
                            Ok(r) => Frame::response(
                                id,
                                "diarize",
                                serde_json::to_value(&r).unwrap_or(Value::Null),
                            ),
                            Err(EngineError { code, message }) => error_frame(id, code, &message),
                        };
                        ok &= self.send(&reply);
                    }
                    break;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    if !cancelled {
                        ok &= self.send(&error_frame(id, "internal", "the pass thread died"));
                    }
                    break;
                }
            }
        }
        ok && !closed
    }
}

fn progress_frame(id: u64, done: u32, total: u32) -> Frame {
    let fraction = if total == 0 {
        0.0
    } else {
        f64::from(done) / f64::from(total)
    };
    Frame::event(
        id,
        "progress",
        serde_json::to_value(ProgressEvent {
            request_id: id,
            fraction,
            completed: Some(done),
            total: Some(total),
        })
        .unwrap_or(Value::Null),
    )
}

/// Runs the protocol on stdin/stdout until stdin closes.
pub fn serve<E: DiarizeEngine>(force_cpu: bool, provider: Option<BackendPreference>) -> ExitCode {
    let stdout = io::stdout();
    let rx = reader_thread();
    let mut host: Host<E, _> = Host {
        out: BufWriter::new(stdout.lock()),
        engine: None,
        pending: VecDeque::new(),
    };
    tracing::info!("engine ready");
    loop {
        let (mut frame, attachments) = match host.pending.pop_front() {
            Some(framed) => framed,
            None => match rx.recv() {
                Ok(Some(Ok(f))) => f,
                Ok(Some(Err(m))) => {
                    tracing::warn!(detail = %m, "malformed frame");
                    if !host.send(&Frame::event(
                        0,
                        "error",
                        json!({"request_id": 0, "code": "bad_request", "message": m}),
                    )) {
                        break;
                    }
                    continue;
                }
                Ok(None) | Err(_) => break,
            },
        };
        if frame.name == "load"
            && let Some(provider) = provider
            && let Some(payload) = frame.payload.as_object_mut()
        {
            payload.remove("provider");
            payload.insert("backend_preference".into(), json!(provider));
        }
        if !host.handle(frame, attachments, force_cpu, &rx) {
            break;
        }
    }
    tracing::info!("engine exiting");
    ExitCode::SUCCESS
}

/// CLI mode: loads the model set, diarizes the samples and returns the
/// same JSON the protocol's `diarize` response carries; `progress` hears
/// the chunk counts.
pub fn run_cli<E: DiarizeEngine>(
    pcm: &[i16],
    load: &LoadParams,
    params: &DiarizeParams,
    force_cpu: bool,
    progress: &mut dyn FnMut(u32, u32),
) -> Result<String, EngineError> {
    let engine = E::load(load, force_cpu)?;
    let result = engine.diarize(pcm, params, progress)?;
    let loaded = engine.loaded();
    let mut value = json!({"turns": result.turns, "backend": loaded.backend});
    if let Some(reason) = loaded.fallback_reason {
        value["fallback_reason"] = json!(reason);
    }
    serde_json::to_string_pretty(&value).map_err(|e| EngineError::new("internal", e.to_string()))
}

#[cfg(test)]
#[path = "host_diarize_tests.rs"]
mod tests;
