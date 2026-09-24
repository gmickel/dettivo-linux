//! R1 through the CLI mode: the two-speaker fixture diarizes into six
//! turns of two speakers that line up with the expected turns (the
//! diarization error rate is scored by `dettivo-qa pipeline diarization`),
//! a missing model names its path, a bad WAV is refused, and the log
//! carries timings and counts only.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const ENGINE: &str = env!("CARGO_BIN_EXE_dettivo-engine-diarize");

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dettivo-qa/fixtures/diarization")
        .join(name)
}

/// The model set, when this machine has it (`scripts/models/fetch-diarization-model.sh`).
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

fn run(args: &[&str]) -> std::process::Output {
    Command::new(ENGINE)
        .args(args)
        .env("RUST_LOG", "trace")
        .output()
        .unwrap()
}

#[test]
fn the_provider_flag_selects_cpu_and_reports_the_backend() {
    let Some(model) = model_dir() else { return };
    let out = run(&[
        "--provider",
        "cpu",
        "--wav",
        fixture("two-speakers.wav").to_str().unwrap(),
        "--model",
        model.to_str().unwrap(),
        "--speakers",
        "2",
        "--json",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["backend"], "cpu");
    assert!(result.get("fallback_reason").is_none());
}

#[cfg(not(feature = "cuda"))]
#[test]
fn a_strict_cuda_pin_on_a_cpu_build_is_refused() {
    let Some(model) = model_dir() else { return };
    let out = run(&[
        "--provider",
        "cuda",
        "--wav",
        fixture("two-speakers.wav").to_str().unwrap(),
        "--model",
        model.to_str().unwrap(),
        "--json",
    ]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("backend_unavailable") && stderr.contains("no CUDA provider"),
        "{stderr}"
    );
}

#[test]
fn the_fixture_diarizes_into_the_expected_turns() {
    let Some(model) = model_dir() else {
        eprintln!("skip: the diarization model set is not downloaded");
        return;
    };
    let wav = fixture("two-speakers.wav");
    let expected: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture("two-speakers.turns.json")).unwrap())
            .unwrap();
    let out = run(&[
        "--provider",
        "cpu",
        "--wav",
        wav.to_str().unwrap(),
        "--model",
        model.to_str().unwrap(),
        "--speakers",
        "2",
        "--json",
    ]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    let turns = result["turns"].as_array().unwrap();
    let want = expected["turns"].as_array().unwrap();
    assert_eq!(turns.len(), want.len(), "{result}");
    let labels: std::collections::BTreeSet<&str> = turns
        .iter()
        .map(|t| t["speaker"].as_str().unwrap())
        .collect();
    assert_eq!(labels.len(), 2, "two speakers: {labels:?}");
    // Every expected turn's midpoint falls inside one answered turn, and
    // the mapping from expected speaker to label is one to one.
    let mut mapping = std::collections::BTreeMap::new();
    for w in want {
        let mid = (w["start_ms"].as_u64().unwrap() + w["end_ms"].as_u64().unwrap()) / 2;
        let hit = turns
            .iter()
            .find(|t| {
                t["start_ms"].as_u64().unwrap() <= mid && mid <= t["end_ms"].as_u64().unwrap()
            })
            .unwrap_or_else(|| panic!("no turn covers {mid} ms: {result}"));
        let label = hit["speaker"].as_str().unwrap().to_string();
        let previous = mapping
            .entry(w["speaker"].as_str().unwrap().to_string())
            .or_insert_with(|| label.clone());
        assert_eq!(previous, &label, "speaker mapping drifted: {result}");
        let start_delta = w["start_ms"]
            .as_u64()
            .unwrap()
            .abs_diff(hit["start_ms"].as_u64().unwrap());
        assert!(
            start_delta <= 250,
            "turn start off by {start_delta} ms: {result}"
        );
    }
    assert!(stderr.contains("model loaded backend=cpu"), "{stderr}");
    assert!(stderr.contains("diarized audio_ms=18855"), "{stderr}");
    assert!(stderr.contains("progress completed="), "{stderr}");
    let human = run(&[
        "--provider",
        "cpu",
        "--wav",
        wav.to_str().unwrap(),
        "--model",
        model.to_str().unwrap(),
        "--speakers",
        "2",
    ]);
    let lines = String::from_utf8_lossy(&human.stdout);
    assert_eq!(lines.lines().count(), 6, "{lines}");
    assert!(lines.contains("SPEAKER_00") && lines.contains("SPEAKER_01"));
}

#[test]
fn a_missing_model_names_its_path_and_a_bad_wav_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let wav = fixture("two-speakers.wav");
    let out = run(&[
        "--wav",
        wav.to_str().unwrap(),
        "--model",
        dir.path().to_str().unwrap(),
        "--json",
    ]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("model_missing"), "{stderr}");
    assert!(stderr.contains("segmentation.onnx"), "{stderr}");
    let bad = dir.path().join("bad.wav");
    std::fs::write(&bad, b"not a wav").unwrap();
    let Some(model) = model_dir() else { return };
    let out = run(&[
        "--wav",
        bad.to_str().unwrap(),
        "--model",
        model.to_str().unwrap(),
        "--json",
    ]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("bad_request"), "{stderr}");
    assert!(stderr.contains("bad.wav"), "{stderr}");
    let out = run(&["--wav", wav.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(4), "--model is required");
}
