//! fn-85 R4 end to end: `dettivo dictation stop --print` prints the
//! processed text, inserts nothing and records `printed` in the history;
//! a take without speech exits 6 (`NoSpeech`) from `stop --print` and from
//! a plain `stop` alike. Skipped by name when tiny.en or jfk.wav is not on
//! disk.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use common::{CLI, Daemon, Tree, stderr, stdout};

fn local_model() -> Option<(PathBuf, PathBuf)> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let model = data.join("dettivo/models/whisper/tiny.en/ggml-tiny.en.bin");
    let wav = data.join("dettivo/models/fixtures/jfk.wav");
    (model.is_file() && wav.is_file()).then_some((model, wav))
}

/// A daemon on tiny.en whose mock microphone plays `wav` and whose mock
/// inserter records into `state/dettivo/qa/inserted.txt`.
fn daemon(model: &Path, wav: &Path) -> Daemon {
    let tree = Tree::new();
    let models = tree.root().join("data/dettivo/models/whisper/tiny.en");
    std::fs::create_dir_all(&models).unwrap();
    std::os::unix::fs::symlink(model, models.join("ggml-tiny.en.bin")).unwrap();
    let bin_dir = Path::new(CLI).parent().unwrap().to_path_buf();
    if !bin_dir.join("dettivo-engine-whisper").is_file() {
        let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-q", "-p", "dettivo-engine-whisper"])
            .status()
            .expect("cargo build");
        assert!(status.success());
    }
    tree.write_config(&format!(
        "[engines]\ndirectory = \"{}\"\n[speech]\nmodel = \"tiny.en\"\n",
        bin_dir.display()
    ));
    Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_INSERT", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
        ],
    )
}

/// Two seconds of digital silence as a 16 kHz mono WAV.
fn silent_wav(dir: &Path) -> PathBuf {
    let samples = 32_000u32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + samples * 2).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&16_000u32.to_le_bytes());
    bytes.extend_from_slice(&32_000u32.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(samples * 2).to_le_bytes());
    bytes.resize(bytes.len() + samples as usize * 2, 0);
    let path = dir.join("silence.wav");
    std::fs::write(&path, bytes).unwrap();
    path
}

fn start(d: &Daemon) {
    let started = d.cli(&["dictation", "start", "--language", "en", "--mode", "raw"]);
    assert_eq!(started.status.code(), Some(0), "{}", stderr(&started));
}

#[test]
fn stop_print_prints_the_text_inserts_nothing_and_records_printed() {
    let Some((model, wav)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let d = daemon(&model, &wav);
    let inserted = d.tree.root().join("state/dettivo/qa/inserted.txt");

    start(&d);
    std::thread::sleep(Duration::from_secs(3));
    let json = d.cli(&["--json", "dictation", "stop", "--print"]);
    assert_eq!(json.status.code(), Some(0), "{}", stderr(&json));
    let result: serde_json::Value = serde_json::from_str(&stdout(&json)).unwrap();
    let text = result["text"].as_str().unwrap();
    assert!(text.to_lowercase().contains("fellow americans"), "{text}");
    assert_eq!(result["insertion"]["outcome"], "printed");
    assert!(!inserted.exists(), "nothing was inserted");
    let id = result["ref"]["id"].as_str().unwrap();
    let item = stdout(&d.cli(&["--json", "history", "get", id]));
    let item: serde_json::Value = serde_json::from_str(&item).unwrap();
    assert_eq!(item["facts"]["insertion"]["outcome"], "printed", "{item}");

    start(&d);
    std::thread::sleep(Duration::from_secs(3));
    let plain = d.cli(&["dictation", "stop", "--print"]);
    assert_eq!(plain.status.code(), Some(0), "{}", stderr(&plain));
    assert_eq!(stdout(&plain), format!("{text}\n"), "the text alone");
    assert!(!inserted.exists());
}

#[test]
fn a_take_without_speech_exits_no_speech_from_stop_and_stop_print() {
    let Some((model, _)) = local_model() else {
        eprintln!("skip: tiny.en missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let d = daemon(&model, &silent_wav(dir.path()));
    for print in [true, false] {
        start(&d);
        std::thread::sleep(Duration::from_millis(600));
        let mut args = vec!["dictation", "stop"];
        if print {
            args.push("--print");
        }
        let stop = d.cli(&args);
        assert_eq!(stop.status.code(), Some(6), "print={print}");
        assert_eq!(stdout(&stop), "", "print={print}");
        assert!(
            stderr(&stop).contains("no speech was heard"),
            "print={print}: {}",
            stderr(&stop)
        );
    }
}
