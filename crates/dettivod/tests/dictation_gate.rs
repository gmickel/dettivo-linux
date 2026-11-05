//! fn-79 R2 against a live daemon with the Silero model downloaded: a take
//! loud enough for the level stage but with no voice in it (steady noise)
//! ends `cancelled` with `silence: voice`, stores nothing, and the voice
//! pass never reports as an engine. The same daemon dictates the jfk
//! fixture through the gate as speech. Skipped by name when tiny.en, the
//! VAD model or the fixture is not on disk.

mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::subscriber::Subscriber;
use common::{Daemon, local_model, tree_with_tiny};
use serde_json::json;

fn local_vad() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let vad = data.join("dettivo/models/whisper/silero-vad/ggml-silero-v5.1.2.bin");
    vad.is_file().then_some(vad)
}

/// One take through the mock microphone playing `wav`: the states the
/// stream showed, ending at the first `idle`.
fn take(
    wav: &std::path::Path,
    vad: &std::path::Path,
    tiny: &std::path::Path,
) -> (Vec<serde_json::Value>, Daemon) {
    let tree = tree_with_tiny(tiny, "");
    let dir = tree.root().join("data/dettivo/models/whisper/silero-vad");
    std::fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(vad, dir.join("ggml-silero-v5.1.2.bin")).unwrap();
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
        ],
    );
    let mut sub = Subscriber::open(&daemon, &["dictation.state", "engine.state"], 256);
    daemon.result("dictation.start", json!({"language": "en", "mode": "raw"}));
    let events = sub.collect(
        |p| p["topic"] == "dictation.state" && p["payload"]["state"] == "idle",
        Duration::from_secs(120),
    );
    (events, daemon)
}

#[test]
fn a_take_without_a_voice_never_reaches_the_engine() {
    let (Some((tiny, jfk)), Some(vad)) = (local_model(), local_vad()) else {
        eprintln!(
            "skip: tiny.en, silero-vad or jfk.wav missing (run scripts/models/fetch-test-model.sh)"
        );
        return;
    };
    let noise = std::env::temp_dir().join(format!("dettivo-noise-{}.wav", std::process::id()));
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&noise, spec).unwrap();
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    for _ in 0..32_000 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        writer.write_sample(((x >> 52) as i16 - 2_048) * 2).unwrap();
    }
    writer.finalize().unwrap();

    let (events, daemon) = take(&noise, &vad, &tiny);
    let _ = std::fs::remove_file(&noise);
    let cancelled = events
        .iter()
        .find(|p| p["topic"] == "dictation.state" && p["payload"]["state"] == "cancelled")
        .unwrap_or_else(|| panic!("{events:?}"));
    assert_eq!(cancelled["payload"]["reason"], "silence: voice");
    assert!(
        events
            .iter()
            .all(|p| p["topic"] != "engine.state"
                || p["payload"]["binary"] == "dettivo-engine-whisper"),
        "{events:?}"
    );
    let items = daemon.result(
        "transcripts.list",
        json!({"kinds": ["dictation"], "limit": 1, "cursor": null}),
    );
    assert_eq!(items["items"].as_array().unwrap().len(), 0);

    let (events, _daemon) = take(&jfk, &vad, &tiny);
    let states: Vec<&str> = events
        .iter()
        .filter(|p| p["topic"] == "dictation.state")
        .map(|p| p["payload"]["state"].as_str().unwrap())
        .collect();
    assert_eq!(
        states,
        ["recording", "transcribing", "inserting", "idle"],
        "{events:?}"
    );
}
