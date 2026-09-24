//! R1 over the protocol: `status` before any load, an unknown request
//! answered with the protocol's error shape, `diarize` before `load`
//! refused, a bad attachment count refused, a missing model named, and,
//! with the model set on disk, a `load`, a `diarize` with `progress`
//! events carrying the chunk counts, a `cancel` that is answered at once
//! while the pass finishes in the background, `status` reporting `busy`
//! meanwhile, and an `unload`; then a clean exit when stdin closes.

use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use dettivo_engine_proto::host::read_wav;
use dettivo_engine_proto::{Frame, Kind, pcm_to_bytes, read_frame, write_frame};
use serde_json::json;

const ENGINE: &str = env!("CARGO_BIN_EXE_dettivo-engine-diarize");

fn model_dir() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("DETTIVO_TEST_DIARIZATION_MODEL") {
        let path = PathBuf::from(path);
        assert!(path.join("segmentation.onnx").is_file() && path.join("embedding.onnx").is_file());
        return Some(path);
    }
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let dir = data.join("dettivo/models/diarize/diarization-en");
    (dir.join("segmentation.onnx").is_file() && dir.join("embedding.onnx").is_file()).then_some(dir)
}

struct Engine {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Engine {
    fn spawn() -> Self {
        let mut child = Command::new(ENGINE)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin: Some(stdin),
            stdout,
        }
    }

    fn send(&mut self, frame: &Frame, attachments: &[&[u8]]) {
        let stdin = self.stdin.as_mut().unwrap();
        write_frame(stdin, frame, attachments).unwrap();
        stdin.flush().unwrap();
    }

    /// Frames until the response to `id`, the response last.
    fn until_response(&mut self, id: u64) -> Vec<Frame> {
        let mut out = Vec::new();
        loop {
            let (reply, _) = read_frame(&mut self.stdout).unwrap();
            let done = reply.kind == Kind::Response && reply.id == id;
            out.push(reply);
            if done {
                return out;
            }
        }
    }

    fn call(&mut self, frame: Frame) -> Frame {
        let id = frame.id;
        self.send(&frame, &[]);
        self.until_response(id).pop().unwrap()
    }

    fn finish(mut self) {
        drop(self.stdin.take());
        let status = self.child.wait().unwrap();
        assert!(status.success(), "{status}");
    }
}

#[test]
fn the_protocol_answers_every_request_shape_without_a_model() {
    let mut engine = Engine::spawn();
    let status = engine.call(Frame::request(1, "status", json!({})));
    assert_eq!(status.name, "status");
    assert_eq!(status.payload["loaded"], false);
    let unknown = engine.call(Frame::request(2, "frobnicate", json!({})));
    assert_eq!(unknown.payload["code"], "bad_request");
    assert_eq!(unknown.payload["request_id"], 2);
    let early = engine.call(Frame::request(3, "diarize", json!({})));
    assert_eq!(early.payload["code"], "bad_request");
    let cancel = engine.call(Frame::request(4, "cancel", json!({"request_id": 3})));
    assert_eq!(cancel.name, "cancel");
    let missing = engine.call(Frame::request(
        5,
        "load",
        json!({"model": "/nonexistent/diarization"}),
    ));
    assert_eq!(missing.payload["code"], "model_missing");
    assert!(
        missing.payload["message"]
            .as_str()
            .unwrap()
            .contains("/nonexistent/diarization/segmentation.onnx")
    );
    let other = engine.call(Frame::request(6, "recognize", json!({})));
    assert_eq!(other.payload["code"], "bad_request");
    engine.finish();
}

#[test]
fn a_pass_reports_chunk_progress_and_a_cancel_is_answered_at_once() {
    let Some(model) = model_dir() else {
        eprintln!("skip: the diarization model set is not downloaded");
        return;
    };
    let wav = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dettivo-qa/fixtures/diarization/two-speakers.wav");
    let pcm = pcm_to_bytes(&read_wav(&wav).unwrap());
    let mut engine = Engine::spawn();
    let loaded = engine.call(Frame::request(
        1,
        "load",
        json!({"model": model.to_str().unwrap(), "threads": 2, "provider": "cpu"}),
    ));
    assert_eq!(loaded.name, "load", "{loaded:?}");
    assert_eq!(loaded.payload["backend"], "cpu");
    assert!(loaded.payload.get("fallback_reason").is_none());

    // A bad attachment count is refused before anything runs.
    engine.send(&Frame::request(2, "diarize", json!({})), &[]);
    let refused = engine.until_response(2).pop().unwrap();
    assert_eq!(refused.payload["code"], "bad_request");

    engine.send(
        &Frame::request(3, "diarize", json!({"speakers": 2})).with_pcm(pcm.len() as u64),
        &[&pcm],
    );
    let frames = engine.until_response(3);
    let progress: Vec<&Frame> = frames.iter().filter(|f| f.name == "progress").collect();
    assert!(progress.len() >= 2, "{frames:?}");
    let last_progress = progress.last().unwrap();
    assert_eq!(
        last_progress.payload["completed"],
        last_progress.payload["total"]
    );
    assert!(last_progress.payload["total"].as_u64().unwrap() > 1);
    assert_eq!(last_progress.payload["fraction"], 1.0);
    let result = frames.last().unwrap();
    assert_eq!(result.name, "diarize", "{result:?}");
    assert_eq!(result.payload["turns"].as_array().unwrap().len(), 6);

    // A cancel right after the request is answered before the pass ends;
    // a status meanwhile says busy; the result never arrives.
    engine.send(
        &Frame::request(4, "diarize", json!({"speakers": 2})).with_pcm(pcm.len() as u64),
        &[&pcm],
    );
    engine.send(&Frame::request(5, "status", json!({})), &[]);
    engine.send(&Frame::request(6, "cancel", json!({"request_id": 4})), &[]);
    let frames = engine.until_response(4);
    let cancelled = frames.last().unwrap();
    assert_eq!(cancelled.name, "error", "{frames:?}");
    assert_eq!(cancelled.payload["code"], "cancelled");
    let busy = frames
        .iter()
        .find(|f| f.id == 5 && f.kind == Kind::Response);
    assert!(
        busy.is_none_or(|f| f.payload["busy"] == true),
        "a status during the pass says busy: {frames:?}"
    );
    let ack = engine.until_response(6).pop().unwrap();
    assert_eq!(ack.name, "cancel");
    let idle = engine.call(Frame::request(7, "status", json!({})));
    assert_eq!(idle.payload["busy"], false);
    assert_eq!(idle.payload["loaded"], true);
    let unloaded = engine.call(Frame::request(8, "unload", json!({})));
    assert_eq!(unloaded.name, "unload");
    let idle = engine.call(Frame::request(9, "status", json!({})));
    assert_eq!(idle.payload["loaded"], false);
    engine.finish();
}
