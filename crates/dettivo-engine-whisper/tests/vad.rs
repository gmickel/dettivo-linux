//! The voice activity pass over the protocol (fn-79 R2): a load that names
//! only the Silero model answers `detect_speech` with the speech it hears
//! at the dictation gate's parameters. Speech, quiet speech and one short
//! word are found; silence, steady noise and a hum are not. That process
//! never recognizes.

use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use dettivo_engine_proto::{Frame, Kind, host, pcm_to_bytes, read_frame, write_frame};
use serde_json::json;

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

/// A deterministic noise source, so the test never depends on a seed.
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

#[test]
fn only_voice_counts_as_speech() {
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
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut id = 0;
    let mut call = |name: &str, payload: serde_json::Value, pcm: Option<&[i16]>| -> Frame {
        id += 1;
        let bytes = pcm.map(pcm_to_bytes);
        let mut frame = Frame::request(id, name, payload);
        if let Some(b) = &bytes {
            frame = frame.with_pcm(b.len() as u64);
        }
        let attachments: Vec<&[u8]> = bytes.iter().map(Vec::as_slice).collect();
        write_frame(&mut stdin, &frame, &attachments).unwrap();
        stdin.flush().unwrap();
        loop {
            let (reply, _) = read_frame(&mut stdout).unwrap();
            if reply.kind == Kind::Response {
                return reply;
            }
        }
    };
    let loaded = call(
        "load",
        json!({"model": "", "vad_model": vad.to_str().unwrap()}),
        None,
    );
    assert_eq!(loaded.name, "load", "{:?}", loaded.payload);
    assert_eq!(loaded.payload["backend"], "cpu");
    let params = json!({"threshold": 0.4, "min_speech_ms": 160});
    let mut speech_ms = |pcm: &[i16]| -> u64 {
        let reply = call("detect_speech", params.clone(), Some(pcm));
        assert_eq!(reply.name, "detect_speech", "{:?}", reply.payload);
        reply.payload["speech_ms"].as_u64().unwrap()
    };

    let spoken = speech_ms(&jfk);
    assert!(spoken > 5_000, "the JFK sentence is speech: {spoken} ms");
    let whispered: Vec<i16> = jfk.iter().map(|s| s / 16).collect();
    assert!(speech_ms(&whispered) > 0, "speech 24 dB down is speech");
    // "And so", the take's first 0.75 s: one short utterance.
    assert!(speech_ms(&jfk[..12_000]) > 0, "a short word is speech");

    assert_eq!(speech_ms(&vec![0i16; 48_000]), 0, "silence");
    assert_eq!(speech_ms(&noise(48_000, 3_000.0)), 0, "steady noise");
    let hum: Vec<i16> = (0..48_000)
        .map(|i| {
            ((i as f64 * 2.0 * std::f64::consts::PI * 100.0 / 16_000.0).sin() * 4_000.0) as i16
        })
        .collect();
    assert_eq!(speech_ms(&hum), 0, "a mains hum");

    let refused = call("recognize", json!({"language": "en"}), Some(&jfk[..16_000]));
    assert_eq!(refused.payload["code"], "bad_request");
    drop(stdin);
    assert!(child.wait().unwrap().success());
}
