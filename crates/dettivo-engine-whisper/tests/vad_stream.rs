//! The speech check during the take (fn-90 R1, ADR 0097): every gate
//! fixture fed through `feed_speech` in capture-sized pieces, then ended
//! by `detect_speech` with `continues`, finds exactly the speech one
//! `detect_speech` over the whole take finds, so the gate's verdict cannot
//! change. A `continues` that does not match the stream is refused.

use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::process::{ChildStdin, ChildStdout, Command, Stdio};

use dettivo_engine_proto::{Frame, Kind, host, pcm_to_bytes, read_frame, write_frame};
use serde_json::{Value, json};

const ENGINE: &str = env!("CARGO_BIN_EXE_dettivo-engine-whisper");

fn models() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let dir = data.join("dettivo/models");
    (dir.join("whisper/silero-vad/ggml-silero-v5.1.2.bin")
        .is_file()
        && dir.join("fixtures/jfk.wav").is_file())
    .then_some(dir)
}

/// The same deterministic noise `tests/vad.rs` gates.
fn noise(n: usize, amplitude: f64) -> Vec<i16> {
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let unit = (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0;
            (unit * amplitude) as i16
        })
        .collect()
}

struct Engine {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    id: u64,
}

impl Engine {
    fn call(&mut self, name: &str, payload: Value, pcm: &[i16]) -> Frame {
        self.id += 1;
        let mut frame = Frame::request(self.id, name, payload);
        let bytes = pcm_to_bytes(pcm);
        let mut attachments: Vec<&[u8]> = Vec::new();
        if name != "load" {
            frame = frame.with_pcm(bytes.len() as u64);
            attachments.push(&bytes);
        }
        write_frame(&mut self.stdin, &frame, &attachments).unwrap();
        self.stdin.flush().unwrap();
        loop {
            let (reply, _) = read_frame(&mut self.stdout).unwrap();
            if reply.kind == Kind::Response {
                return reply;
            }
        }
    }

    fn speech_ms(reply: &Frame) -> u64 {
        assert_eq!(reply.name, "detect_speech", "{:?}", reply.payload);
        reply.payload["speech_ms"].as_u64().unwrap()
    }

    /// `pcm` fed in pieces of `piece` samples with its last `rest` held
    /// back for the release, as the session does.
    fn streamed(&mut self, pcm: &[i16], piece: usize, rest: usize) -> u64 {
        let split = pcm.len().saturating_sub(rest);
        for (i, chunk) in pcm[..split].chunks(piece).enumerate() {
            let fed = self.call("feed_speech", json!({"start": i == 0}), chunk);
            assert_eq!(fed.name, "feed_speech", "{:?}", fed.payload);
        }
        let params = json!({"threshold": 0.4, "min_speech_ms": 160, "continues": split});
        Self::speech_ms(&self.call("detect_speech", params, &pcm[split..]))
    }
}

#[test]
fn a_take_heard_as_it_is_captured_finds_the_whole_take_speech() {
    let Some(models) = models() else {
        eprintln!("skip: VAD model missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let vad = models.join("whisper/silero-vad/ggml-silero-v5.1.2.bin");
    let jfk = host::read_wav(&models.join("fixtures/jfk.wav")).unwrap();
    let mut child = Command::new(ENGINE)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut engine = Engine {
        stdin: child.stdin.take().unwrap(),
        stdout: BufReader::new(child.stdout.take().unwrap()),
        id: 0,
    };
    let loaded = engine.call(
        "load",
        json!({"model": "", "vad_model": vad.to_str().unwrap()}),
        &[],
    );
    assert_eq!(loaded.name, "load", "{:?}", loaded.payload);

    let whispered: Vec<i16> = jfk.iter().map(|s| s / 16).collect();
    let hum: Vec<i16> = (0..48_000)
        .map(|i| {
            ((i as f64 * 2.0 * std::f64::consts::PI * 100.0 / 16_000.0).sin() * 4_000.0) as i16
        })
        .collect();
    let long: Vec<i16> = [&jfk[..], &vec![0i16; 40_000], &jfk[..]].concat();
    let fixtures: [(&str, Vec<i16>); 7] = [
        ("speech", jfk.clone()),
        ("speech 24 dB down", whispered),
        ("a short word", jfk[..12_000].to_vec()),
        ("silence", vec![0i16; 48_000]),
        ("steady noise", noise(48_000, 3_000.0)),
        ("a mains hum", hum),
        ("two sentences around a pause", long),
    ];
    let params = json!({"threshold": 0.4, "min_speech_ms": 160});
    for (what, pcm) in &fixtures {
        let whole = Engine::speech_ms(&engine.call("detect_speech", params.clone(), pcm));
        // 20 ms capture chunks with the 200 ms release tail held back, a
        // piece that never fills a window, an uneven backlog, and the
        // whole take fed at once with nothing left for the release.
        for (piece, rest) in [(320, 3_200), (100, 0), (7_777, 1), (pcm.len(), 0)] {
            assert_eq!(
                engine.streamed(pcm, piece.max(1), rest),
                whole,
                "{what}: pieces of {piece}, {rest} samples at the release"
            );
        }
    }

    // A stream that does not hold what the release says is refused, and
    // so is one never opened.
    engine.call("feed_speech", json!({"start": true}), &jfk[..1_024]);
    let wrong = json!({"threshold": 0.4, "min_speech_ms": 160, "continues": 2_048});
    let refused = engine.call("detect_speech", wrong.clone(), &jfk[1_024..]);
    assert_eq!(
        refused.payload["code"], "bad_request",
        "{:?}",
        refused.payload
    );
    let unopened = engine.call("detect_speech", wrong, &jfk[1_024..]);
    assert_eq!(
        unopened.payload["code"], "bad_request",
        "{:?}",
        unopened.payload
    );
    let not_started = engine.call("feed_speech", json!({"start": false}), &jfk[..512]);
    assert_eq!(
        not_started.payload["code"], "bad_request",
        "{:?}",
        not_started.payload
    );

    drop(engine);
    assert!(child.wait().unwrap().success());
}
