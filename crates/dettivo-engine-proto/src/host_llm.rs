//! The engine side of the protocol for a language model binary. The
//! difference from [`crate::host`] is that `generate` streams: frames
//! keep arriving while a generation runs, so a `cancel` for the running
//! request stops it between two tokens (the answer so far comes back with
//! `finish_reason = cancelled`), `status` answers `busy` meanwhile, and any
//! other request waits its turn. One thread owns the model; a reader
//! thread feeds frames through a channel.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::io::{self, BufReader, BufWriter, Write};
use std::process::ExitCode;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use serde_json::{Value, json};

use crate::frame::{Frame, FrameError, read_frame, write_frame};
use crate::host::EngineError;
use crate::messages::{
    CancelParams, Empty, ErrorEvent, GenerateParams, GenerateResult, LoadParams, LoadedResult,
    StatusResult,
};

/// What a language model binary implements; the host loop does the rest.
pub trait LanguageEngine: Sized {
    /// Loads the model named by `params` on the backend it asks for.
    fn load(params: &LoadParams, force_cpu: bool) -> Result<Self, EngineError>;
    /// The `loaded` payload (model, backend, reason).
    fn loaded(&self) -> LoadedResult;
    /// Bytes in use on the backend device, when it reports them.
    fn memory_bytes(&self) -> Option<u64>;
    /// Generates an answer, handing every piece of text to `partial` as it
    /// lands and asking `cancelled` between tokens whether to stop.
    fn generate(
        &mut self,
        params: &GenerateParams,
        partial: &mut dyn FnMut(&str),
        cancelled: &mut dyn FnMut() -> bool,
    ) -> Result<GenerateResult, EngineError>;
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

/// One decoded frame, or the reader's failure (`None` is end of stream).
type Incoming = Option<Result<Frame, String>>;

/// Frames from stdin, one thread ahead of the model.
fn reader_thread() -> Receiver<Incoming> {
    let (tx, rx) = mpsc::channel::<Incoming>();
    std::thread::Builder::new()
        .name("dettivo-engine-stdin".into())
        .spawn(move || {
            let stdin = io::stdin();
            let mut reader = BufReader::new(stdin.lock());
            loop {
                let item = match read_frame(&mut reader) {
                    Ok((frame, _)) => Some(Ok(frame)),
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

struct Host<E, W: Write> {
    out: W,
    engine: Option<E>,
    /// Requests that arrived during a generation, answered after it.
    pending: VecDeque<Frame>,
    unloaded_memory: fn() -> Option<u64>,
}

impl<E: LanguageEngine, W: Write> Host<E, W> {
    fn send(&mut self, frame: &Frame) -> bool {
        write_frame(&mut self.out, frame, &[]).is_ok()
    }

    fn status(&self, busy: bool) -> Frame {
        let loaded = self.engine.as_ref().map(E::loaded);
        let memory_bytes = match self.engine.as_ref() {
            Some(e) => e.memory_bytes(),
            None => (self.unloaded_memory)(),
        };
        Frame::response(
            0,
            "status",
            serde_json::to_value(StatusResult {
                loaded: loaded.is_some(),
                model: loaded.as_ref().map(|l| l.model.clone()),
                backend: loaded.as_ref().map(|l| l.backend),
                busy,
                memory_bytes,
            })
            .unwrap_or(Value::Null),
        )
    }

    /// Answers one request; `generate` runs here with the channel polled
    /// between tokens. Returns false when stdout is gone.
    fn handle(&mut self, frame: Frame, force_cpu: bool, rx: &Receiver<Incoming>) -> bool {
        let id = frame.id;
        let reply = match frame.name.as_str() {
            "load" => match serde_json::from_value::<LoadParams>(frame.payload.clone()) {
                Ok(p) => {
                    drop(self.engine.take());
                    match E::load(&p, force_cpu) {
                        Ok(e) => {
                            let loaded = e.loaded();
                            self.engine = Some(e);
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
            "status" => {
                let mut s = self.status(false);
                s.id = id;
                s
            }
            "cancel" => match serde_json::from_value::<CancelParams>(frame.payload.clone()) {
                Ok(_) => Frame::response(
                    id,
                    "cancel",
                    serde_json::to_value(Empty {}).unwrap_or(Value::Null),
                ),
                Err(e) => error_frame(id, "bad_request", &e.to_string()),
            },
            "generate" => match serde_json::from_value::<GenerateParams>(frame.payload.clone()) {
                Err(e) => error_frame(id, "bad_request", &e.to_string()),
                Ok(_) if self.engine.is_none() => {
                    error_frame(id, "bad_request", "no model is loaded; send load first")
                }
                Ok(p) => return self.generate(id, &p, rx),
            },
            "recognize" | "diarize" => error_frame(
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

    /// Runs one generation, answering `cancel` (for this request) and
    /// `status` from the channel between tokens and queueing the rest.
    fn generate(&mut self, id: u64, params: &GenerateParams, rx: &Receiver<Incoming>) -> bool {
        let mut engine = self.engine.take().expect("checked by the caller");
        let loaded = engine.loaded();
        let memory = engine.memory_bytes();
        let out = RefCell::new(&mut self.out);
        let ok = Cell::new(true);
        let emit = |frame: &Frame| {
            if ok.get() {
                ok.set(write_frame(&mut **out.borrow_mut(), frame, &[]).is_ok());
            }
        };
        let mut cancelled = false;
        let mut closed = false;
        let mut pending: VecDeque<Frame> = VecDeque::new();
        let result = {
            let mut partial = |text: &str| {
                emit(&Frame::event(
                    id,
                    "partial",
                    json!({"request_id": id, "text": text}),
                ));
            };
            let mut check = || {
                loop {
                    match rx.try_recv() {
                        Ok(Some(Ok(f))) if f.name == "cancel" => {
                            let target = serde_json::from_value::<CancelParams>(f.payload.clone())
                                .map(|c| c.request_id)
                                .unwrap_or(0);
                            if target == id {
                                cancelled = true;
                                tracing::info!("generation cancelled");
                            }
                            pending.push_back(f);
                        }
                        Ok(Some(Ok(f))) if f.name == "status" => {
                            emit(&Frame::response(
                                f.id,
                                "status",
                                serde_json::to_value(StatusResult {
                                    loaded: true,
                                    model: Some(loaded.model.clone()),
                                    backend: Some(loaded.backend),
                                    busy: true,
                                    memory_bytes: memory,
                                })
                                .unwrap_or(Value::Null),
                            ));
                        }
                        Ok(Some(Ok(f))) => pending.push_back(f),
                        Ok(Some(Err(m))) => emit(&Frame::event(
                            0,
                            "error",
                            json!({"request_id": 0, "code": "bad_request", "message": m}),
                        )),
                        Ok(None) | Err(TryRecvError::Disconnected) => {
                            closed = true;
                            cancelled = true;
                            break;
                        }
                        Err(TryRecvError::Empty) => break,
                    }
                }
                cancelled || !ok.get()
            };
            engine.generate(params, &mut partial, &mut check)
        };
        self.engine = Some(engine);
        let reply = match result {
            Ok(r) => Frame::response(
                id,
                "generate",
                serde_json::to_value(&r).unwrap_or(Value::Null),
            ),
            Err(EngineError { code, message }) => error_frame(id, code, &message),
        };
        emit(&reply);
        // Everything that arrived meanwhile is answered now, in order; the
        // cancel that stopped this generation is acknowledged like any other.
        self.pending.extend(pending);
        ok.get() && !closed
    }
}

/// Runs the protocol on stdin/stdout until stdin closes. `unloaded_memory`
/// reports the backend device's usage once no model is loaded.
pub fn serve<E: LanguageEngine>(force_cpu: bool, unloaded_memory: fn() -> Option<u64>) -> ExitCode {
    let stdout = io::stdout();
    let rx = reader_thread();
    let mut host: Host<E, _> = Host {
        out: BufWriter::new(stdout.lock()),
        engine: None,
        pending: VecDeque::new(),
        unloaded_memory,
    };
    tracing::info!("engine ready");
    loop {
        let frame = match host.pending.pop_front() {
            Some(f) => f,
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
        if !host.handle(frame, force_cpu, &rx) {
            break;
        }
    }
    tracing::info!("engine exiting");
    ExitCode::SUCCESS
}

/// CLI mode: loads the model, generates once and returns the same JSON the
/// protocol's `generate` response carries.
pub fn run_cli<E: LanguageEngine>(
    load: &LoadParams,
    params: &GenerateParams,
    force_cpu: bool,
) -> Result<String, EngineError> {
    let mut engine = E::load(load, force_cpu)?;
    let result = engine.generate(params, &mut |_| {}, &mut || false)?;
    serde_json::to_string_pretty(&result).map_err(|e| EngineError::new("internal", e.to_string()))
}

#[cfg(test)]
#[path = "host_llm_tests.rs"]
mod tests;
