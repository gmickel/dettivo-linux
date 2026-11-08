//! fn-96 R3 end to end: `dettivo vocabulary suggest` against a daemon in
//! a temporary tree writes nothing without `--apply`, and with it writes
//! the spellings it proposed. The daemon runs the local tiny.en model and
//! a stand-in `piper` on its PATH that "speaks" the opening words of the
//! jfk fixture, so the engine hears the same clip for every saying; the
//! test is skipped by name when the model is not on disk.

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use common::{Daemon, Tree, stderr, stdout};
use serde_json::Value;

/// The local tiny.en model and the jfk fixture, when both exist.
fn local_tiny() -> Option<(PathBuf, PathBuf)> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let model = data.join("dettivo/models/whisper/tiny.en/ggml-tiny.en.bin");
    let wav = data.join("dettivo/models/fixtures/jfk.wav");
    (model.is_file() && wav.is_file()).then_some((model, wav))
}

/// The first `ms` milliseconds of a 16 kHz mono 16-bit WAV's samples, raw.
fn opening(wav: &Path, ms: usize) -> Vec<u8> {
    let bytes = std::fs::read(wav).unwrap();
    let at = bytes.windows(4).position(|w| w == b"data").unwrap() + 8;
    bytes[at..at + ms * 32].to_vec()
}

/// A daemon on tiny.en whose vocabulary is `vocabulary` (TOML), with two
/// voice files and a `piper` that answers every saying with `clip`.
fn daemon(model: &Path, clip: &[u8], vocabulary: &str) -> Daemon {
    let tree = Tree::new();
    let root = tree.root();
    let tiny = root.join("data/dettivo/models/whisper/tiny.en");
    std::fs::create_dir_all(&tiny).unwrap();
    std::os::unix::fs::symlink(model, tiny.join("ggml-tiny.en.bin")).unwrap();
    let voices = root.join("data/dettivo/models/piper");
    std::fs::create_dir_all(&voices).unwrap();
    for voice in ["en_US-amy-low", "en_US-ryan-low"] {
        std::fs::write(voices.join(format!("{voice}.onnx")), b"voice").unwrap();
        let config = r#"{"audio": {"sample_rate": 16000}}"#;
        std::fs::write(voices.join(format!("{voice}.onnx.json")), config).unwrap();
    }
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(root.join("clip.raw"), clip).unwrap();
    let piper = bin.join("piper");
    let script = format!(
        "#!/bin/sh\ncat > /dev/null\ncat '{}'\n",
        root.join("clip.raw").display()
    );
    std::fs::write(&piper, script).unwrap();
    std::fs::set_permissions(&piper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let engines = Path::new(common::CLI).parent().unwrap().to_path_buf();
    if !engines.join("dettivo-engine-whisper").is_file() {
        let status = std::process::Command::new(std::env::var("CARGO").unwrap_or("cargo".into()))
            .args(["build", "-q", "-p", "dettivo-engine-whisper"])
            .status()
            .expect("cargo build");
        assert!(status.success());
    }
    tree.write_config(&format!(
        "[engines]\ndirectory = \"{}\"\n[engines.whisper]\nbackend = \"cpu\"\n[speech]\nmodel = \"tiny.en\"\n[dictation]\nlanguage = \"en\"\nvocabulary = {vocabulary}\n",
        engines.display()
    ));
    let path = format!("{}:/usr/bin:/bin", bin.display());
    Daemon::spawn(tree, &[("PATH", path.as_str())])
}

fn file(d: &Daemon) -> String {
    std::fs::read_to_string(d.tree.config_file()).unwrap()
}

fn suggest(d: &Daemon, args: &[&str]) -> std::process::Output {
    let mut all = vec!["vocabulary", "suggest"];
    all.extend_from_slice(args);
    let out = d.cli(&all);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    out
}

#[test]
fn nothing_is_written_without_apply_and_apply_writes_the_proposal() {
    let Some((model, wav)) = local_tiny() else {
        eprintln!("skip: tiny.en or jfk.wav missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    // "And so my fellow Americans": six words at most, which the engine
    // hears where a four-word term should be, so they are its spelling.
    let d = daemon(
        &model,
        &opening(&wav, 2_500),
        "[\"Fellow Americans Ltd Inc\"] # mine",
    );
    let before = file(&d);

    // The whole vocabulary, as a person reads it and as JSON.
    let out = suggest(&d, &[]);
    let text = stdout(&out);
    assert!(
        text.starts_with("Fellow Americans Ltd Inc: heard as \""),
        "{text}"
    );
    assert!(
        text.ends_with(
            "nothing written; --apply would write new sounds_like for 1 term already in dictation.vocabulary\n"
        ),
        "{text}"
    );
    let out = suggest(
        &d,
        &[
            "--json",
            "Fellow Americans Ltd Inc",
            "Kennedy Speech Ltd Inc",
        ],
    );
    let got: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(got["change"]["written"], false);
    assert_eq!(got["change"]["added"], serde_json::json!([1]));
    assert_eq!(got["change"]["extended"], serde_json::json!([0]));
    let spellings = got["suggestions"][0]["sounds_like"].clone();
    let proposal = spellings[0].as_str().unwrap().to_string();
    assert!(proposal.contains("fellow americans"), "{got}");
    assert_eq!(got["suggestions"][1]["term"], "Kennedy Speech Ltd Inc");
    assert_eq!(file(&d), before, "nothing written without --apply");

    // With --apply the held term gains the spellings in place, the named
    // term the file lacked is added with its own, and "Americans", which
    // has no spelling to propose, is added on its own: the clips were heard
    // with it in the vocabulary.
    let out = suggest(
        &d,
        &[
            "Fellow Americans Ltd Inc",
            "Kennedy Speech Ltd Inc",
            "Americans",
            "--apply",
        ],
    );
    assert!(
        stdout(&out).ends_with(
            "Americans: misheard, with no new spelling to propose\nwrote 2 new terms to dictation.vocabulary and new sounds_like for 1 term already in dictation.vocabulary\n"
        ),
        "{}",
        stdout(&out)
    );
    let list = format!("{spellings}").replace(',', ", ");
    assert_eq!(
        file(&d),
        before.replace(
            "[\"Fellow Americans Ltd Inc\"] # mine",
            &format!(
                "[{{ term = \"Fellow Americans Ltd Inc\", sounds_like = {list} }}, {{ term = \"Kennedy Speech Ltd Inc\", sounds_like = {list} }}, \"Americans\"] # mine"
            )
        )
    );
}

#[test]
fn a_refused_write_still_prints_every_suggestion() {
    let Some((model, wav)) = local_tiny() else {
        eprintln!("skip: tiny.en or jfk.wav missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    // The entry already carries the 16 spellings a term may have, so one
    // more is refused, as a dry run and with --apply alike.
    let full: Vec<String> = (0..16).map(|i| format!("\"s{i}\"")).collect();
    let vocabulary = format!(
        "[{{ term = \"Fellow Americans Ltd Inc\", sounds_like = [{}] }}]",
        full.join(", ")
    );
    let d = daemon(&model, &opening(&wav, 2_500), &vocabulary);
    let before = file(&d);
    for apply in [false, true] {
        let mut args = vec!["--json", "vocabulary", "suggest"];
        if apply {
            args.push("--apply");
        }
        let out = d.cli(&args);
        assert_eq!(out.status.code(), Some(4), "{}", stderr(&out));
        assert!(
            stderr(&out).contains("17 sounds_like spellings"),
            "{}",
            stderr(&out)
        );
        // One document: the suggestions, and the refusal beside them.
        let got: Value = serde_json::from_str(&stdout(&out)).unwrap();
        assert_eq!(got["change"], Value::Null);
        assert_eq!(got["error"]["data"]["app_code"], "INVALID_PARAMS", "{got}");
        let heard = got["suggestions"][0]["sounds_like"][0].as_str().unwrap();
        assert!(heard.contains("fellow americans"), "{got}");
    }
    assert_eq!(file(&d), before);
}
