//! R3, R4 and R7 through the CLI mode: the tiny model transcribes the JFK
//! fixture under the stated word error rate on CPU, the backend is
//! reported (`cpu` when forced; `vulkan` only when built with it and a
//! device is installed), a missing model names its path, and the log never
//! carries the prompt or the transcript.

use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use dettivo_engine_proto::{Frame, Kind, read_frame, write_frame};

const ENGINE: &str = env!("CARGO_BIN_EXE_dettivo-engine-whisper");
/// The threshold the fixture must stay under (tiny.en on CPU scores 0).
const MAX_WER: f64 = 0.15;

fn models() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let dir = data.join("dettivo/models");
    dir.join("whisper/tiny.en/ggml-tiny.en.bin")
        .is_file()
        .then_some(dir)
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

#[test]
fn cli_mode_transcribes_the_fixture_under_the_wer_threshold_and_reports_the_backend() {
    let Some(models) = models() else {
        eprintln!("skip: test model missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let model = models.join("whisper/tiny.en/ggml-tiny.en.bin");
    let wav = models.join("fixtures/jfk.wav");
    let reference = words(&std::fs::read_to_string(models.join("fixtures/jfk.txt")).unwrap());
    let out = Command::new(ENGINE)
        .args([
            "--wav",
            wav.to_str().unwrap(),
            "--model",
            model.to_str().unwrap(),
            "--cpu",
            "--json",
            "--prompt",
            "MARKER_PROMPT_TEXT",
        ])
        .env("RUST_LOG", "trace")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let hypothesis = words(result["text"].as_str().unwrap());
    let rate = wer(&reference, &hypothesis);
    assert!(rate < MAX_WER, "WER {rate:.3}: {:?}", result["text"]);
    assert_eq!(result["backend"], "cpu");
    assert_eq!(result["language"], "en");
    assert!(result["segments"].as_array().is_some_and(|s| !s.is_empty()));
    assert!(result["duration_ms"].as_u64().unwrap() > 10_000);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("MARKER_PROMPT_TEXT"),
        "prompt leaked into the log:\n{stderr}"
    );
    for word in ["fellow", "americans", "country"] {
        assert!(
            !stderr.to_lowercase().contains(word),
            "transcript leaked into the log:\n{stderr}"
        );
    }
    assert!(
        stderr.contains("model loaded"),
        "the log still carries lifecycle lines:\n{stderr}"
    );

    let auto = Command::new(ENGINE)
        .args([
            "--wav",
            wav.to_str().unwrap(),
            "--model",
            model.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    let auto: serde_json::Value = serde_json::from_slice(&auto.stdout).unwrap();
    let backend = auto["backend"].as_str().unwrap();
    assert!(backend == "cpu" || backend == "vulkan", "{backend}");
    let vulkan_built = cfg!(feature = "vulkan");
    if backend == "vulkan" {
        assert!(
            vulkan_built,
            "vulkan reported by a build without the feature"
        );
    }
}

#[test]
fn a_missing_model_names_the_path_and_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("silence.wav");
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
    let missing = dir.path().join("nope.bin");
    let out = Command::new(ENGINE)
        .args([
            "--wav",
            wav.to_str().unwrap(),
            "--model",
            missing.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("model_missing") && stderr.contains("nope.bin"),
        "{stderr}"
    );
}

/// `backend_preference = vulkan` is Vulkan or a refusal: a build without
/// the backend answers `load_failed` naming the preference instead of
/// opening the model on the CPU.
#[test]
fn a_vulkan_preference_on_a_cpu_build_is_refused_never_loaded_on_the_cpu() {
    if cfg!(feature = "vulkan") {
        eprintln!("skip: a Vulkan build answers by its device");
        return;
    }
    let Some(models) = models() else {
        eprintln!("skip: test model missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let model = models.join("whisper/tiny.en/ggml-tiny.en.bin");
    let mut child = Command::new(ENGINE)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let load = Frame::request(
        1,
        "load",
        serde_json::json!({"model": model.to_string_lossy(), "backend_preference": "vulkan"}),
    );
    write_frame(&mut stdin, &load, &[]).unwrap();
    stdin.flush().unwrap();
    let (reply, _) = read_frame(&mut stdout).unwrap();
    assert_eq!(
        (reply.kind, reply.name.as_str()),
        (Kind::Response, "error"),
        "{reply:?}"
    );
    assert_eq!(reply.payload["code"], "load_failed");
    let message = reply.payload["message"].as_str().unwrap();
    assert!(message.contains("backend_preference = vulkan"), "{message}");
    assert!(message.contains("no Vulkan backend"), "{message}");
    let (status, _) = {
        write_frame(
            &mut stdin,
            &Frame::request(2, "status", serde_json::json!({})),
            &[],
        )
        .unwrap();
        stdin.flush().unwrap();
        read_frame(&mut stdout).unwrap()
    };
    assert_eq!(
        status.payload["loaded"], false,
        "nothing loaded on the CPU: {status:?}"
    );
    drop(stdin);
    let _ = child.wait();
}
