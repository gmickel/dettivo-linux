//! fn-93 (ADR 0093): a `feed_speech` that names a threshold answers where
//! the take's last voiced window ends, so the release can tell a silent
//! ending from a spoken one. Asking changes nothing the gate then decides.

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

    /// `pcm` fed in 20 ms pieces, the last one asking where the voice ends.
    fn voiced_until(&mut self, pcm: &[i16]) -> Value {
        let pieces: Vec<&[i16]> = pcm.chunks(320).collect();
        for (i, piece) in pieces.iter().enumerate() {
            let mut params = json!({"start": i == 0});
            if i + 1 == pieces.len() {
                params["threshold"] = json!(0.4);
            }
            let fed = self.call("feed_speech", params, piece);
            assert_eq!(fed.name, "feed_speech", "{:?}", fed.payload);
            if i + 1 == pieces.len() {
                assert_eq!(fed.payload["samples"], pcm.len() as u64);
                return fed.payload["voiced_until"].clone();
            }
            assert!(fed.payload.get("voiced_until").is_none(), "not asked");
        }
        unreachable!("an empty take")
    }
}

#[test]
fn a_feed_that_asks_answers_where_the_voice_ends() {
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
    let ms = |n: usize| n * 16;

    // Mid-sentence ("my fellow Americans"): voiced into the last window run.
    let spoken = ms(2_000);
    let until = engine.voiced_until(&jfk[..spoken]).as_u64().unwrap() as usize;
    assert!(
        until <= spoken && spoken - until < ms(64),
        "voiced until {} ms of a take cut at 2000 ms",
        until / 16
    );
    // In the pause after it: the voice ended more than 150 ms before.
    let paused = ms(2_600);
    let until = engine.voiced_until(&jfk[..paused]).as_u64().unwrap() as usize;
    assert!(
        paused - until > ms(150),
        "voiced until {} ms of a take cut at 2600 ms",
        until / 16
    );
    // Silence holds no voiced window.
    assert_eq!(engine.voiced_until(&vec![0i16; ms(1_000)]), json!(0));

    // Asking leaves the verdict to the gate: the whole take's speech.
    let params = json!({"threshold": 0.4, "min_speech_ms": 160});
    let whole = engine.call("detect_speech", params.clone(), &jfk);
    engine.voiced_until(&jfk);
    let streamed = engine.call(
        "detect_speech",
        json!({"threshold": 0.4, "min_speech_ms": 160, "continues": jfk.len()}),
        &[],
    );
    assert_eq!(streamed.payload, whole.payload);

    drop(engine);
    assert!(child.wait().unwrap().success());
}
