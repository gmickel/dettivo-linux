//! R4 against a live daemon with the mock microphone and the local
//! Parakeet v2 model: `speech.selection.set` picks the provider, the
//! provider row reports it runnable with word timestamps and the
//! alignment spike's meeting verdict, `system.capabilities` carries the
//! same flag, and a dictation through the mock microphone returns the
//! fixture's words. Skipped by name when the model is not on disk.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use common::{DAEMON, Daemon, Tree};
use serde_json::json;

/// The local Parakeet v2 model and the jfk fixture, when both exist.
fn local_parakeet() -> Option<(PathBuf, PathBuf)> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let model = data.join("dettivo/models/parakeet/parakeet-v2/tdt-0.6b-v2-q8_0.gguf");
    let wav = data.join("dettivo/models/fixtures/jfk.wav");
    (model.is_file() && wav.is_file()).then_some((model, wav))
}

/// A tree whose models directory links the local model in as a verified
/// download and whose config selects the Parakeet provider.
fn tree_with_parakeet(model: &Path) -> Tree {
    let tree = Tree::new();
    let dir = tree.root().join("data/dettivo/models/parakeet/parakeet-v2");
    std::fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(model, dir.join("tdt-0.6b-v2-q8_0.gguf")).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        r#"{"provider":"parakeet","id":"parakeet-v2","sha256":"2027e2e1a4dc60ccdd8558f93b15e7c0db4ef8895b4e82e889f3a6275d8119c6","size_bytes":903835936,"downloaded_at":0,"catalogue_version":1,"verified":true}"#,
    )
    .unwrap();
    let bin_dir = Path::new(DAEMON).parent().unwrap().to_path_buf();
    if !bin_dir.join("dettivo-engine-parakeet").is_file() {
        let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-q", "-p", "dettivo-engine-parakeet"])
            .status()
            .expect("cargo build");
        assert!(status.success());
    }
    tree.write_config(&format!(
        "[engines]\ndirectory = \"{}\"\n[engines.parakeet]\nbackend = \"cpu\"\n[speech]\nprovider = \"whisper\"\nmodel = \"tiny.en\"\nparakeet_model_id = \"parakeet-v2\"\n[dictation]\nlanguage = \"en\"\n",
        bin_dir.display()
    ));
    tree
}

#[test]
fn a_parakeet_dictation_returns_the_fixture_words_with_the_provider_selected_over_the_contract() {
    let Some((model, wav)) = local_parakeet() else {
        eprintln!("skip: parakeet-v2 or jfk.wav missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let daemon = Daemon::spawn(
        tree_with_parakeet(&model),
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
        ],
    );
    let caps = daemon.result("system.capabilities", json!({}));
    assert_eq!(
        caps["speech"]["providers"]["whisper"]["meeting_capable"],
        true
    );
    assert_eq!(
        caps["speech"]["providers"]["parakeet"]["meeting_capable"],
        false
    );

    // Switching the provider alone lands on parakeet_model_id, and the
    // provider row says the engine is runnable with word timestamps.
    let selection = daemon.result("speech.selection.set", json!({"provider": "parakeet"}));
    assert_eq!(selection["provider_id"], "parakeet");
    assert_eq!(selection["model"], "parakeet-v2");
    assert_eq!(selection["dictation"]["is_parakeet"], true);
    let providers = daemon.result("speech.providers.list", json!({}));
    let parakeet = providers["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "parakeet")
        .unwrap()
        .clone();
    assert_eq!(parakeet["runtime_available"], true, "{parakeet}");
    assert_eq!(parakeet["supports_timestamps"], true);
    assert_eq!(parakeet["supports_meetings"], false);
    let v2 = parakeet["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == "parakeet-v2")
        .unwrap()
        .clone();
    assert_eq!(v2["is_downloaded"], true);
    assert_eq!(v2["is_english_only"], true);

    let started = daemon.result("dictation.start", json!({"language": "en", "mode": "raw"}));
    assert_eq!(started["job"]["state"], "running");
    let status = daemon.result("dictation.status", json!({}));
    assert!(
        status["job"]["message"]
            .as_str()
            .unwrap()
            .contains("parakeet/parakeet-v2"),
        "{status}"
    );
    std::thread::sleep(Duration::from_millis(6500));
    // Stop waits on a cold Parakeet 0.6B load and decode on the CPU, which
    // outruns the default window on a two-core CI runner.
    let stopped = daemon.result_within("dictation.stop", json!({}), Duration::from_secs(120));
    assert_eq!(stopped["job"]["state"], "succeeded", "{stopped}");
    let sessions = daemon.tree.root().join("state/dettivo/sessions/job_dict_1");
    assert!(!sessions.join("transcript.json").exists());
    let transcript = daemon.result("transcripts.get", json!({"ref": stopped["ref"]}));
    let text = transcript["text_polish"].as_str().unwrap().to_lowercase();
    assert!(
        text.contains("fellow americans") && text.contains("country"),
        "{text}"
    );

    let engines = daemon.result("speech.engines", json!({}));
    let row = engines["engines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["binary"] == "dettivo-engine-parakeet")
        .unwrap()
        .clone();
    assert_eq!(row["backend"], "cpu", "{row}");
    assert_eq!(row["reason"], "backend_preference = cpu");
    assert!(
        row["model"]
            .as_str()
            .unwrap()
            .ends_with("tdt-0.6b-v2-q8_0.gguf")
    );

    let log = daemon.stop();
    assert!(
        !log.to_lowercase().contains("americans"),
        "transcript leaked into the log"
    );
}
