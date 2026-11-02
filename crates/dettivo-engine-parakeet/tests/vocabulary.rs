//! Vocabulary rescoring through the CLI mode on the term-recall fixture
//! clips (ADR 0088): R1, the output is byte-identical to a plain decode
//! with the boost off, an empty list, or no candidate in the take; and a
//! misheard term the audio supports comes back as the term, with the log
//! carrying counts and never words. Skips when Parakeet v3 is not on disk.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ENGINE: &str = env!("CARGO_BIN_EXE_dettivo-engine-parakeet");

const VOCABULARY: &str = r#"[{"term":"Dettivo","aliases":["detivo","debt evo"]},
    {"term":"Clef","aliases":["clay","cleff"]},
    {"term":"SapienXT","aliases":["sapient x t","sapien xt"]},
    {"term":"Omarchy","aliases":["oh marky","omar key"]},
    {"term":"Hyprland","aliases":["hyper land","hyperland"]}]"#;

fn model() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let path = data.join("dettivo/models/parakeet/parakeet-v3/tdt-0.6b-v3-q8_0.gguf");
    path.is_file().then_some(path)
}

fn clip(id: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dettivo-qa/fixtures/vocabulary")
        .join(format!("{id}.wav"))
}

fn run(model: &Path, id: &str, extra: &[&str]) -> Output {
    let wav = clip(id);
    let out = Command::new(ENGINE)
        .args([
            "--wav",
            wav.to_str().unwrap(),
            "--model",
            model.to_str().unwrap(),
        ])
        .args(["--cpu", "--json"])
        .args(extra)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{id} {extra:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

#[test]
fn off_empty_or_without_a_candidate_the_output_is_byte_identical() {
    let Some(model) = model() else {
        eprintln!("skip: parakeet-v3 missing");
        return;
    };
    let plain = run(&model, "guard-rain", &[]).stdout;
    for extra in [
        &["--vocabulary", "[]"][..],
        &["--vocabulary", VOCABULARY, "--boost", "off"][..],
        // A guard clip holds no candidate: the take path, nothing rescored.
        &["--vocabulary", VOCABULARY][..],
    ] {
        let out = run(&model, "guard-rain", extra);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&plain),
            "{extra:?}"
        );
    }
}

#[test]
fn a_misheard_term_the_audio_supports_comes_back_spelt_as_listed() {
    let Some(model) = model() else {
        eprintln!("skip: parakeet-v3 missing");
        return;
    };
    let text = |out: &Output| {
        serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()["text"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let plain = run(&model, "target-sapienxt-1", &[]);
    assert!(!text(&plain).contains("SapienXT"), "{}", text(&plain));
    let biased = run(&model, "target-sapienxt-1", &["--vocabulary", VOCABULARY]);
    assert!(text(&biased).starts_with("SapienXT "), "{}", text(&biased));
    let stderr = String::from_utf8_lossy(&biased.stderr);
    assert!(
        stderr.contains("vocabulary rescoring") && stderr.contains("replaced=1"),
        "{stderr}"
    );
    for word in ["sapien", "zapian", "contract"] {
        assert!(
            !stderr.to_lowercase().contains(word),
            "words in the log:\n{stderr}"
        );
    }
}

#[test]
fn a_boost_that_is_not_a_level_is_refused_by_name() {
    let out = Command::new(ENGINE)
        .args([
            "--wav",
            "x.wav",
            "--model",
            "x.gguf",
            "--vocabulary",
            "[]",
            "--boost",
            "loud",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--boost loud"));
}
