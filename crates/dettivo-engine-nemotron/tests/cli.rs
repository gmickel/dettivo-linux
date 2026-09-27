//! R1 and R5 through the CLI mode: the two-speaker fixture diarizes into
//! its six alternating turns within 250 ms of the expected starts, with
//! the per-frame probabilities written as `.npy`; more than eight expected
//! speakers, a missing model and a backend the build lacks are refused
//! naming why; an automatic load on a build without a GPU backend says why
//! it is on the CPU. The model-backed tests skip without the model
//! (`scripts/models/fetch-diarization-model.sh "" nemotron-3-diarization`).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

const ENGINE: &str = env!("CARGO_BIN_EXE_dettivo-engine-nemotron");
const MODEL_FILE: &str = "Nemotron-3-Diarization.q8_0.gguf";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dettivo-qa/fixtures/diarization")
        .join(name)
}

/// The model directory, when this machine has it.
fn model_dir() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("DETTIVO_TEST_NEMOTRON_MODEL") {
        let path = PathBuf::from(path);
        assert!(path.join(MODEL_FILE).is_file(), "{}", path.display());
        return Some(path);
    }
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let dir = data.join("dettivo/models/diarize/nemotron-3-diarization");
    dir.join(MODEL_FILE).is_file().then_some(dir)
}

fn run(args: &[&str]) -> Output {
    Command::new(ENGINE)
        .args(args)
        .env_remove("DETTIVO_FORCE_CPU")
        .output()
        .unwrap()
}

fn diarize(model: &Path, extra: &[&str]) -> Output {
    let wav = fixture("two-speakers.wav");
    let mut args = vec![
        "--wav",
        wav.to_str().unwrap(),
        "--model",
        model.to_str().unwrap(),
        "--json",
    ];
    args.extend_from_slice(extra);
    run(&args)
}

#[test]
fn the_fixture_diarizes_into_its_six_turns_with_the_frame_probabilities() {
    let Some(model) = model_dir() else {
        eprintln!("skip: Nemotron 3 Diarization is not downloaded");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let probs = dir.path().join("probs.npy");
    let out = diarize(
        &model,
        &["--provider", "cpu", "--probs", probs.to_str().unwrap()],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["backend"], "cpu");
    assert!(result.get("fallback_reason").is_none());
    let expected: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture("two-speakers.turns.json")).unwrap())
            .unwrap();
    let (turns, expected) = (
        result["turns"].as_array().unwrap(),
        expected["turns"].as_array().unwrap(),
    );
    assert_eq!(turns.len(), expected.len(), "{turns:?}");
    for (got, want) in turns.iter().zip(expected) {
        let (a, b) = (
            got["start_ms"].as_i64().unwrap(),
            want["start_ms"].as_i64().unwrap(),
        );
        assert!((a - b).abs() <= 250, "{got} against {want}");
        // A and B alternate; so must SPEAKER_00 and SPEAKER_01.
        let first = want["speaker"] == "A";
        assert_eq!(got["speaker"] == "SPEAKER_00", first, "{got}");
    }
    let frames = &result["frames"];
    assert_eq!(
        (frames["speakers"].as_u64(), frames["frame_ms"].as_u64()),
        (Some(8), Some(10))
    );
    let count = frames["count"].as_u64().unwrap();
    assert!((1880..=1890).contains(&count), "{count} frames for 18.9 s");
    let npy = std::fs::read(&probs).unwrap();
    assert!(npy.starts_with(b"\x93NUMPY\x01\x00"));
    let header_len = u16::from_le_bytes([npy[8], npy[9]]) as usize;
    let header = std::str::from_utf8(&npy[10..10 + header_len]).unwrap();
    assert!(
        header.contains(&format!("'shape': ({count}, 8)")),
        "{header}"
    );
    assert_eq!(npy.len(), 10 + header_len + count as usize * 8 * 4);
}

#[test]
fn more_than_eight_expected_speakers_is_refused() {
    let Some(model) = model_dir() else {
        eprintln!("skip: Nemotron 3 Diarization is not downloaded");
        return;
    };
    let out = diarize(&model, &["--provider", "cpu", "--speakers", "9"]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("tracks at most 8 speakers; 9 expected"),
        "{stderr}"
    );
}

#[cfg(not(any(feature = "vulkan", feature = "cuda")))]
#[test]
fn an_automatic_load_on_a_build_without_a_gpu_backend_says_why() {
    let Some(model) = model_dir() else {
        eprintln!("skip: Nemotron 3 Diarization is not downloaded");
        return;
    };
    let out = diarize(&model, &[]);
    assert!(out.status.success());
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["backend"], "cpu");
    let reason = result["fallback_reason"].as_str().unwrap();
    assert!(reason.contains("has no GPU backend"), "{reason}");
}

#[test]
fn a_missing_model_and_a_backend_the_build_lacks_are_refused() {
    let wav = fixture("two-speakers.wav");
    let empty = tempfile::tempdir().unwrap();
    let out = run(&[
        "--wav",
        wav.to_str().unwrap(),
        "--model",
        empty.path().to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    let want = format!(
        "model_missing: model file not found: {}",
        empty.path().join(MODEL_FILE).display()
    );
    assert!(stderr.contains(&want), "{stderr}");
    // The refusal comes before the model is opened, so any file will do.
    let absent = if cfg!(feature = "cuda") {
        "vulkan"
    } else {
        "cuda"
    };
    let out = run(&[
        "--wav",
        wav.to_str().unwrap(),
        "--model",
        wav.to_str().unwrap(),
        "--provider",
        absent,
    ]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("backend_unavailable"), "{stderr}");
}
