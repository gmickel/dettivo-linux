//! ADR 0073 R1 and R5 against a live daemon with the mock tracks, tiny.en
//! and `[meetings.diarization] model = "nemotron-3-diarization"`: with the
//! sherpa-onnx set alone on disk the pass runs it and the block says
//! Nemotron is not downloaded; with Nemotron on disk the pass runs
//! `dettivo-engine-nemotron`, and a meeting expecting nine speakers falls
//! back to the sherpa-onnx set naming the eight-speaker limit. Skipped
//! without the models.

mod common;

use common::diarize::{
    diarization_model, import_meeting, nemotron_model, wait_diarization, with_diarization,
    with_nemotron,
};
use common::meetings::{env, spawn, track_fixture, tree};
use serde_json::json;

const CONFIG: &str = "[meetings.diarization]\nmodel = \"nemotron-3-diarization\"\n";

#[test]
fn a_missing_nemotron_model_falls_back_to_the_sherpa_set_with_the_reason() {
    let (Some(tree), Some(sherpa)) = (tree(CONFIG), diarization_model()) else {
        eprintln!("skip: tiny.en, jfk.wav or the diarization model set missing");
        return;
    };
    with_diarization(&tree, &sherpa);
    let mic = track_fixture("mic");
    let daemon = spawn(tree, &env(&mic, &mic, &[("DETTIVO_E2E_SEED", "1")]));
    let id = import_meeting(&daemon, json!({"expected_speakers": 2}));
    let row = wait_diarization(&daemon, &id, "ready");
    let block = &row["diarization"];
    assert_eq!(block["engine"], "dettivo-engine-diarize");
    assert_eq!(block["model"], "diarize/diarization-en");
    let why = block["fallback_reason"].as_str().unwrap();
    assert!(
        why.contains("diarize/nemotron-3-diarization is not downloaded")
            && why.contains("diarize/diarization-en runs instead"),
        "{why}"
    );
    assert_eq!(row["speakers"].as_array().unwrap().len(), 2);
    daemon.stop();
}

#[test]
fn nemotron_runs_from_the_daemon_and_nine_speakers_fall_back() {
    let (Some(tree), Some(sherpa), Some(nemotron)) =
        (tree(CONFIG), diarization_model(), nemotron_model())
    else {
        eprintln!("skip: tiny.en, jfk.wav, the diarization model set or Nemotron missing");
        return;
    };
    with_diarization(&tree, &sherpa);
    with_nemotron(&tree, &nemotron);
    let mic = track_fixture("mic");
    let daemon = spawn(tree, &env(&mic, &mic, &[("DETTIVO_E2E_SEED", "1")]));
    let id = import_meeting(&daemon, json!({"expected_speakers": 2}));
    let row = wait_diarization(&daemon, &id, "ready");
    let block = &row["diarization"];
    assert_eq!(block["engine"], "dettivo-engine-nemotron", "{block}");
    assert_eq!(block["model"], "diarize/nemotron-3-diarization");
    assert!(block.get("fallback_reason").is_none(), "{block}");
    assert_eq!(row["speakers"].as_array().unwrap().len(), 2);
    let engines = daemon.result("speech.engines", json!({}));
    let engine = engines["engines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["binary"] == "dettivo-engine-nemotron")
        .cloned()
        .unwrap();
    assert!(engine["backend"].is_string(), "{engine}");
    // The backend and reason the daemon's engine loaded with, for a run on
    // a GPU build (`--nocapture`).
    eprintln!(
        "nemotron engine: {} ({})",
        engine["backend"], engine["reason"]
    );

    let crowded = import_meeting(&daemon, json!({"expected_speakers": 9}));
    let row = wait_diarization(&daemon, &crowded, "ready");
    let block = &row["diarization"];
    assert_eq!(block["engine"], "dettivo-engine-diarize");
    let why = block["fallback_reason"].as_str().unwrap();
    assert!(
        why.starts_with("9 speakers are expected and Nemotron tracks at most 8"),
        "{why}"
    );
    daemon.stop();
}
