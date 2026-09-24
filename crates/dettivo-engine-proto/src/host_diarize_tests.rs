//! The diarize host loop under test: a fake engine that reports one turn
//! per second of audio, a pass that answers its turns after chunk
//! progress, and a cancel that lands mid-pass and is answered at once
//! while a status meanwhile says busy.

use super::*;
use crate::messages::{Backend, SpeakerTurn};
use std::sync::mpsc::Sender;

/// Answers one turn per second of audio, reporting each as a chunk;
/// `speakers == 7` makes the pass wait for a signal so a cancel lands
/// mid-pass.
struct Fake {
    model: String,
}

static GATE: std::sync::Mutex<Option<Receiver<()>>> = std::sync::Mutex::new(None);

impl DiarizeEngine for Fake {
    fn load(params: &LoadParams, _force_cpu: bool) -> Result<Self, EngineError> {
        if params.model == "missing" {
            return Err(EngineError::new("model_missing", "no such model"));
        }
        Ok(Self {
            model: params.model.clone(),
        })
    }

    fn loaded(&self) -> LoadedResult {
        LoadedResult {
            model: self.model.clone(),
            backend: Backend::Cpu,
            reason: "fake".into(),
            fallback_reason: None,
        }
    }

    fn diarize(
        &self,
        pcm: &[i16],
        params: &DiarizeParams,
        progress: &mut dyn FnMut(u32, u32),
    ) -> Result<DiarizeResult, EngineError> {
        let seconds = (pcm.len() / 16_000) as u32;
        let mut turns = Vec::new();
        for i in 0..seconds {
            if params.speakers == Some(7) && i == 1 {
                let gate = GATE
                    .lock()
                    .unwrap()
                    .take()
                    .expect("a gate for the slow pass");
                let _ = gate.recv();
            }
            turns.push(SpeakerTurn {
                start_ms: u64::from(i) * 1000,
                end_ms: u64::from(i + 1) * 1000,
                speaker: format!("SPEAKER_{:02}", i % 2),
            });
            progress(i + 1, seconds);
        }
        Ok(DiarizeResult { turns })
    }
}

fn rig() -> (Host<Fake, Vec<u8>>, Sender<Incoming>, Receiver<Incoming>) {
    let (tx, rx) = mpsc::channel();
    let host = Host {
        out: Vec::new(),
        engine: None,
        pending: VecDeque::new(),
    };
    (host, tx, rx)
}

fn frames(bytes: &[u8]) -> Vec<Frame> {
    let mut cursor = bytes;
    let mut out = Vec::new();
    while let Ok((f, _)) = read_frame(&mut cursor) {
        out.push(f);
    }
    out
}

fn pcm(seconds: usize) -> Vec<Vec<u8>> {
    vec![vec![0u8; seconds * 32_000]]
}

#[test]
fn a_pass_reports_chunks_and_answers_the_turns() {
    let (mut host, _tx, rx) = rig();
    assert!(host.handle(
        Frame::request(1, "load", json!({"model": "m", "threads": 2})),
        Vec::new(),
        true,
        &rx
    ));
    let early = Frame::request(2, "diarize", json!({}));
    assert!(host.handle(early, Vec::new(), true, &rx));
    assert!(host.handle(
        Frame::request(3, "diarize", json!({"speakers": 2})),
        pcm(3),
        true,
        &rx
    ));
    let out = frames(&host.out);
    assert_eq!(out[2].payload["code"], "bad_request", "no attachment");
    let progress: Vec<(u64, u64)> = out
        .iter()
        .filter(|f| f.name == "progress" && f.id == 3)
        .map(|f| {
            (
                f.payload["completed"].as_u64().unwrap(),
                f.payload["total"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(progress.first(), Some(&(0, 0)));
    assert_eq!(progress.last(), Some(&(3, 3)));
    let last = out.last().unwrap();
    assert_eq!(last.name, "diarize");
    assert_eq!(last.payload["turns"].as_array().unwrap().len(), 3);
    assert_eq!(last.payload["turns"][1]["speaker"], "SPEAKER_01");
    assert!(host.handle(
        Frame::request(4, "load", json!({"model": "missing"})),
        Vec::new(),
        true,
        &rx
    ));
    assert_eq!(
        frames(&host.out).last().unwrap().payload["code"],
        "model_missing"
    );
}

#[test]
fn a_cancel_mid_pass_is_answered_at_once_and_status_says_busy() {
    let (mut host, tx, rx) = rig();
    let (open, gate) = mpsc::channel::<()>();
    *GATE.lock().unwrap() = Some(gate);
    assert!(host.handle(
        Frame::request(1, "load", json!({"model": "m"})),
        Vec::new(),
        true,
        &rx
    ));
    // A status, the cancel and an unload arrive while the pass waits
    // at its gate; the gate opens once the cancel is on the channel.
    tx.send(Some(Ok((
        Frame::request(3, "status", json!({})),
        Vec::new(),
    ))))
    .unwrap();
    tx.send(Some(Ok((
        Frame::request(4, "cancel", json!({"request_id": 2})),
        Vec::new(),
    ))))
    .unwrap();
    tx.send(Some(Ok((
        Frame::request(5, "unload", json!({})),
        Vec::new(),
    ))))
    .unwrap();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        let _ = open.send(());
    });
    assert!(host.handle(
        Frame::request(2, "diarize", json!({"speakers": 7})),
        pcm(3),
        true,
        &rx
    ));
    let out = frames(&host.out);
    let names: Vec<(u64, &str)> = out
        .iter()
        .filter(|f| f.name != "progress")
        .map(|f| (f.id, f.name.as_str()))
        .collect();
    assert_eq!(
        names,
        [(0, "loaded"), (1, "load"), (3, "status"), (2, "error")],
        "{names:?}"
    );
    assert_eq!(
        out.iter().find(|f| f.id == 3).unwrap().payload["busy"],
        true
    );
    let cancelled = out.iter().find(|f| f.id == 2 && f.name == "error").unwrap();
    assert_eq!(cancelled.payload["code"], "cancelled");
    assert!(
        !out.iter().any(|f| f.name == "diarize"),
        "the finished pass's result is dropped"
    );
    let pending: Vec<&str> = host.pending.iter().map(|(f, _)| f.name.as_str()).collect();
    assert_eq!(pending, ["cancel", "unload"]);
    while let Some((f, a)) = host.pending.pop_front() {
        assert!(host.handle(f, a, true, &rx));
    }
    assert!(host.handle(
        Frame::request(6, "status", json!({})),
        Vec::new(),
        true,
        &rx
    ));
    let out = frames(&host.out);
    assert_eq!(out.last().unwrap().payload["loaded"], false);
}
