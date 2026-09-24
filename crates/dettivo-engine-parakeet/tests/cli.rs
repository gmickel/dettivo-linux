//! R1 and R3 through the CLI mode: every row of `fixtures/wer.json` whose
//! model is on disk and whose backend this build can use transcribes the
//! JFK fixture under its word error rate ceiling with word timestamps and
//! confidence, the backend and its reason are logged, a language the model
//! lacks is refused naming its languages, a missing model names its path,
//! a corrupt file names the reason, and the log never carries the
//! transcript.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const ENGINE: &str = env!("CARGO_BIN_EXE_dettivo-engine-parakeet");

fn models() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let dir = data.join("dettivo/models");
    dir.join("fixtures/jfk.wav").is_file().then_some(dir)
}

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Word error rate: edit distance over reference words.
fn wer(reference: &[String], hypothesis: &[String]) -> f64 {
    let (n, m) = (reference.len(), hypothesis.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = usize::from(reference[i - 1] != hypothesis[j - 1]);
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
        }
    }
    d[n][m] as f64 / n.max(1) as f64
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(ENGINE)
        .args(args)
        .env("RUST_LOG", "trace")
        .output()
        .unwrap()
}

fn fixture_rows() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/wer.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn every_fixture_row_this_machine_can_run_stays_under_its_wer_ceiling_with_word_timestamps() {
    let Some(models) = models() else {
        eprintln!("skip: jfk.wav missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let wav = models.join("fixtures/jfk.wav");
    let reference = words(&std::fs::read_to_string(models.join("fixtures/jfk.txt")).unwrap());
    let vulkan_usable = cfg!(feature = "vulkan")
        && (Path::new("/usr/share/vulkan/icd.d").is_dir()
            || Path::new("/etc/vulkan/icd.d").is_dir());
    let mut ran = 0;
    for row in fixture_rows()["rows"].as_array().unwrap() {
        let (model, file, backend) = (
            row["model"].as_str().unwrap(),
            row["file"].as_str().unwrap(),
            row["backend"].as_str().unwrap(),
        );
        let max_wer = row["max_wer"].as_f64().unwrap();
        let path = models.join("parakeet").join(model).join(file);
        if !path.is_file() {
            eprintln!(
                "skip: {model} missing (dettivo speech download --provider parakeet --model {model})"
            );
            continue;
        }
        if backend == "vulkan" && !vulkan_usable {
            eprintln!("skip: {model} on vulkan (this build or machine has no Vulkan)");
            continue;
        }
        let mut args = vec![
            "--wav",
            wav.to_str().unwrap(),
            "--model",
            path.to_str().unwrap(),
            "--json",
        ];
        if backend == "cpu" {
            args.push("--cpu");
        }
        let out = run(&args);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "{model} on {backend}: {stderr}");
        let result: Value = serde_json::from_slice(&out.stdout).unwrap();
        let rate = wer(&reference, &words(result["text"].as_str().unwrap()));
        let reason = stderr
            .lines()
            .find(|l| l.contains("model loaded"))
            .unwrap_or_default()
            .to_string();
        println!("wer: fixture=jfk model={model} backend={backend} wer={rate:.3} {reason}");
        assert!(
            rate <= max_wer,
            "fixtures/wer.json row {model}/{backend}: WER {rate:.3} exceeds {max_wer}: {:?}",
            result["text"]
        );
        assert_eq!(result["backend"], backend, "{stderr}");
        assert_eq!(
            result["language"],
            if model == "parakeet-v2" { "en" } else { "auto" }
        );
        assert!(result["duration_ms"].as_u64().unwrap() > 10_000);
        let segments = result["segments"].as_array().unwrap();
        assert!(!segments.is_empty());
        let mut last_end = 0;
        let mut count = 0;
        for segment in segments {
            let ws = segment["words"].as_array().unwrap();
            assert!(!ws.is_empty(), "{segment}");
            for w in ws {
                let (start, end) = (
                    w["start_ms"].as_u64().unwrap(),
                    w["end_ms"].as_u64().unwrap(),
                );
                assert!(start >= last_end && end >= start, "{w}");
                last_end = end;
                let c = w["confidence"].as_f64().unwrap();
                assert!(c > 0.0 && c <= 1.0, "{w}");
                assert!(!w["text"].as_str().unwrap().is_empty());
                count += 1;
            }
        }
        assert!(
            (count as i64 - reference.len() as i64).abs() <= 2,
            "{count} words for {} reference words",
            reference.len()
        );
        assert!(reason.contains("reason="), "the reason is logged: {stderr}");
        for word in ["fellow", "americans", "country"] {
            assert!(
                !stderr.to_lowercase().contains(word),
                "transcript leaked into the log:\n{stderr}"
            );
        }
        ran += 1;
        if model == "parakeet-v2" && backend == "cpu" {
            let out = run(&[
                "--wav",
                wav.to_str().unwrap(),
                "--model",
                path.to_str().unwrap(),
                "--cpu",
                "--language",
                "de",
                "--json",
            ]);
            assert_eq!(out.status.code(), Some(1));
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(
                stderr.contains("bad_request") && stderr.contains("languages: en"),
                "{stderr}"
            );
        }
    }
    if ran == 0 {
        eprintln!("skip: no fixture row could run on this machine");
    }
}

fn silence_wav(dir: &Path) -> PathBuf {
    let wav = dir.join("silence.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&wav, spec).unwrap();
    for _ in 0..1600 {
        w.write_sample(0i16).unwrap();
    }
    w.finalize().unwrap();
    wav
}

#[test]
fn a_missing_model_names_the_path_and_a_corrupt_file_names_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let wav = silence_wav(dir.path());
    let missing = dir.path().join("nope.gguf");
    let out = run(&[
        "--wav",
        wav.to_str().unwrap(),
        "--model",
        missing.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("model_missing") && stderr.contains("nope.gguf"),
        "{stderr}"
    );

    let corrupt = dir.path().join("corrupt.gguf");
    std::fs::write(&corrupt, b"RIFF this is not a model at all").unwrap();
    let out = run(&[
        "--wav",
        wav.to_str().unwrap(),
        "--model",
        corrupt.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("load_failed")
            && stderr.contains("not a GGUF file")
            && stderr.contains("RIFF"),
        "{stderr}"
    );
}
