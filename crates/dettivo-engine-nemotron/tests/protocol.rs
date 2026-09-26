//! R1 over the protocol: a missing model is named, and with the model on
//! disk a `diarize` that asks for frame probabilities gets them as one
//! `probs_f32` attachment matching the declared shape beside the same
//! turns, while a plain `diarize` gets the turns alone; then a clean exit
//! when stdin closes. The host loop's cancel, busy and queue rules are
//! `dettivo-engine-proto`'s own tests.

use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use dettivo_engine_proto::host::read_wav;
use dettivo_engine_proto::{
    FRAME_PROBABILITIES_KIND, Frame, Kind, pcm_to_bytes, read_frame, write_frame,
};
use serde_json::json;

const ENGINE: &str = env!("CARGO_BIN_EXE_dettivo-engine-nemotron");
const MODEL_FILE: &str = "Nemotron-3-Diarization.q8_0.gguf";

fn model_dir() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("DETTIVO_TEST_NEMOTRON_MODEL") {
        return Some(PathBuf::from(path));
    }
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let dir = data.join("dettivo/models/diarize/nemotron-3-diarization");
    dir.join(MODEL_FILE).is_file().then_some(dir)
}

struct Engine {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Engine {
    fn spawn() -> Self {
        let mut child = Command::new(ENGINE)
            .arg("--cpu")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
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

    /// Sends `frame` and returns the response to it with its attachments.
    fn call(&mut self, frame: Frame, attachments: &[&[u8]]) -> (Frame, Vec<Vec<u8>>) {
        let stdin = self.stdin.as_mut().unwrap();
        write_frame(stdin, &frame, attachments).unwrap();
        stdin.flush().unwrap();
        loop {
            let (reply, bytes) = read_frame(&mut self.stdout).unwrap();
            if reply.kind == Kind::Response && reply.id == frame.id {
                return (reply, bytes);
            }
        }
    }

    fn finish(mut self) {
        drop(self.stdin.take());
        let status = self.child.wait().unwrap();
        assert!(status.success(), "{status}");
    }
}

#[test]
fn frame_probabilities_travel_as_one_attachment_when_asked_for() {
    let mut engine = Engine::spawn();
    let (missing, _) = engine.call(
        Frame::request(1, "load", json!({"model": "/nonexistent/nemotron"})),
        &[],
    );
    assert_eq!(missing.payload["code"], "model_missing");
    let Some(model) = model_dir() else {
        eprintln!("skip: Nemotron 3 Diarization is not downloaded");
        engine.finish();
        return;
    };
    let (loaded, _) = engine.call(
        Frame::request(2, "load", json!({"model": model.to_string_lossy()})),
        &[],
    );
    assert_eq!(loaded.payload["backend"], "cpu", "{}", loaded.payload);
    let wav = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dettivo-qa/fixtures/diarization/two-speakers.wav");
    let pcm = pcm_to_bytes(&read_wav(&wav).unwrap());
    let request = |id, payload| Frame::request(id, "diarize", payload).with_pcm(pcm.len() as u64);

    let (with, bytes) = engine.call(request(3, json!({"frame_probabilities": true})), &[&pcm]);
    assert_eq!(with.name, "diarize", "{}", with.payload);
    let frames = &with.payload["frames"];
    let count = frames["count"].as_u64().unwrap();
    assert_eq!(frames["speakers"], 8);
    assert_eq!(with.attachments.len(), 1);
    assert_eq!(with.attachments[0].kind, FRAME_PROBABILITIES_KIND);
    assert_eq!(bytes.len(), 1);
    assert_eq!(bytes[0].len() as u64, count * 8 * 4);
    let probability = f32::from_le_bytes(bytes[0][..4].try_into().unwrap());
    assert!((0.0..=1.0).contains(&probability));

    let (plain, bytes) = engine.call(request(4, json!({})), &[&pcm]);
    assert!(plain.payload.get("frames").is_none());
    assert!(plain.attachments.is_empty() && bytes.is_empty());
    assert_eq!(plain.payload["turns"], with.payload["turns"]);
    engine.finish();
}
