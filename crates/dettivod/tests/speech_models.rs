//! R2 to R6 against a live daemon and the fixture model server: a
//! catalogue file points a model at the server, `speech.models.download`
//! lands it verified, `speech.selection.set` validates against the
//! catalogue and preloads a downloaded model, deleting the selected model
//! needs `force`, a corrupted file is quarantined at start, and the
//! `speech.*` fixtures parse into their shapes.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{Daemon, Tree};
use dettivo_proto::methods::speech::{
    ModelDeleteResult, ModelStatus, ModelsStatusResult, ProvidersListResult, SelectionResult,
};
use dettivo_speech::fixture_server::FixtureServer;
use serde_json::json;
use sha2::{Digest, Sha256};

/// A small pseudo-random model file served by the fixture server.
fn write_fixture(dir: &Path, name: &str, len: usize) -> String {
    let mut bytes = Vec::with_capacity(len);
    let mut x: u32 = 0x9e37_79b9;
    for _ in 0..len {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        bytes.push((x & 0xff) as u8);
    }
    std::fs::write(dir.join(name), &bytes).unwrap();
    format!("{:x}", Sha256::digest(&bytes))
}

fn local_tiny_en() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let p = data.join("dettivo/models/whisper/tiny.en/ggml-tiny.en.bin");
    p.is_file().then_some(p)
}

struct Rig {
    tree: Tree,
    server: FixtureServer,
    _server_dir: tempfile::TempDir,
    has_tiny: bool,
}

/// A tree whose catalogue names a `whisper/test` model on the fixture
/// server (the default), plus `whisper/tiny.en` served from the local copy
/// when this machine has one.
fn rig() -> Rig {
    let tree = Tree::new();
    let server_dir = tempfile::Builder::new()
        .prefix("dtvsrv")
        .tempdir_in("/tmp")
        .unwrap();
    let sha = write_fixture(server_dir.path(), "ggml-test.bin", 300_000);
    let tiny = local_tiny_en();
    if let Some(src) = &tiny {
        std::fs::copy(src, server_dir.path().join("ggml-tiny.en.bin")).unwrap();
    }
    let server = FixtureServer::serve(server_dir.path().to_path_buf());
    let mut catalogue = format!(
        r#"version = 7

[[models]]
provider = "whisper"
id = "test"
display_name = "Test"
kind = "stt"
file_name = "ggml-test.bin"
size_bytes = 300000
url = "{url}/ggml-test.bin"
sha256 = "{sha}"
license = "MIT"
redistribution = "test fixture"
languages = ["en"]
english_only = true
default = true
"#,
        url = server.url,
    );
    if tiny.is_some() {
        catalogue.push_str(&format!(
            r#"
[[models]]
provider = "whisper"
id = "tiny.en"
display_name = "Tiny (English)"
kind = "stt"
file_name = "ggml-tiny.en.bin"
size_bytes = 77704715
url = "{url}/ggml-tiny.en.bin"
sha256 = "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f"
license = "MIT"
redistribution = "OpenAI Whisper weights converted by whisper.cpp"
languages = ["en"]
english_only = true
"#,
            url = server.url,
        ));
    }
    let catalogue_path = tree.root().join("catalogue.toml");
    std::fs::write(&catalogue_path, catalogue).unwrap();
    let bin_dir = Path::new(env!("CARGO_BIN_EXE_dettivod"))
        .parent()
        .unwrap()
        .to_path_buf();
    if tiny.is_some() && !bin_dir.join("dettivo-engine-whisper").is_file() {
        let status =
            std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
                .args(["build", "-q", "-p", "dettivo-engine-whisper"])
                .status()
                .expect("cargo build");
        assert!(status.success());
    }
    let bin_dir = bin_dir.to_string_lossy().into_owned();
    tree.write_config(&format!(
        "[engines]\ndirectory = \"{bin_dir}\"\nstt_idle_seconds = 600\n[speech]\nmodel = \"test\"\n[models]\ncatalogue_file = \"{}\"\n",
        catalogue_path.display()
    ));
    Rig {
        tree,
        server,
        _server_dir: server_dir,
        has_tiny: tiny.is_some(),
    }
}

fn wait_for(daemon: &Daemon, model: &str, want: &str, budget: Duration) -> ModelStatus {
    let deadline = Instant::now() + budget;
    loop {
        let v = daemon.result(
            "speech.models.status",
            json!({"provider": "whisper", "model": model}),
        );
        let r: ModelsStatusResult = serde_json::from_value(v).unwrap();
        let row = r.models.into_iter().next().unwrap();
        let state = serde_json::to_value(row.readiness).unwrap();
        if state == want || Instant::now() > deadline {
            assert_eq!(state, want, "{row:?}");
            return row;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_model_downloads_verified_and_the_selection_preloads_it() {
    let rig = rig();
    let daemon = Daemon::spawn(rig.tree, &[]);

    let status: ModelsStatusResult =
        serde_json::from_value(daemon.result("speech.models.status", json!({}))).unwrap();
    assert_eq!(status.catalogue_version, 7);
    let test = status.models.iter().find(|m| m.id == "test").unwrap();
    assert_eq!(serde_json::to_value(test.readiness).unwrap(), "missing");
    assert!(test.is_selected && test.is_default);

    let started: ModelStatus = serde_json::from_value(daemon.result(
        "speech.models.download",
        json!({"provider": "whisper", "model": "test"}),
    ))
    .unwrap();
    assert!(matches!(
        serde_json::to_value(started.readiness).unwrap().as_str(),
        Some("downloading" | "ready")
    ));
    let ready = wait_for(&daemon, "test", "ready", Duration::from_secs(20));
    assert_eq!(ready.bytes_done, 300_000);
    let path = PathBuf::from(ready.path.clone().unwrap());
    assert!(path.is_file());
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(path.parent().unwrap().join("manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["verified"], true);
    assert_eq!(manifest["catalogue_version"], 7);
    assert_eq!(rig.server.seen().len(), 1);

    // providers.list reads the same readiness source.
    let providers: ProvidersListResult =
        serde_json::from_value(daemon.result("speech.providers.list", json!({}))).unwrap();
    let whisper = providers
        .providers
        .iter()
        .find(|p| p.id == "whisper")
        .unwrap();
    let test = whisper.models.iter().find(|m| m.id == "test").unwrap();
    assert!(test.is_downloaded && test.is_recommended && test.is_english_only);
    assert_eq!(providers.default_provider_id, "whisper");

    // An unknown model is INVALID_PARAMS naming it; a cloud id is refused.
    let err = daemon.request("speech.selection.set", json!({"model": "nope"}));
    assert_eq!(err["error"]["data"]["app_code"], "INVALID_PARAMS");
    assert!(
        err["error"]["message"]
            .as_str()
            .unwrap()
            .contains("whisper/nope")
    );
    let err = daemon.request("speech.selection.set", json!({"cloud_model_id": "x"}));
    assert_eq!(err["error"]["data"]["app_code"], "INVALID_PARAMS");
    let err = daemon.request(
        "speech.models.download",
        json!({"provider": "whisper", "model": "nope"}),
    );
    assert_eq!(err["error"]["message"], "unknown model whisper/nope");

    // Selecting the downloaded model applies and asks for a preload; the
    // fake file cannot load, which is logged and never an error here.
    let sel: SelectionResult = serde_json::from_value(daemon.result(
        "speech.selection.set",
        json!({"model": "test", "meeting_model": ""}),
    ))
    .unwrap();
    assert_eq!(sel.dictation.model_id, "test");
    assert_eq!(sel.meeting_effective_model_id, "test");
    assert_eq!(sel.provider_id, "whisper");

    if rig.has_tiny {
        daemon.result(
            "speech.models.download",
            json!({"provider": "whisper", "model": "tiny.en"}),
        );
        wait_for(&daemon, "tiny.en", "ready", Duration::from_secs(60));
        let sel: SelectionResult = serde_json::from_value(
            daemon.result("speech.selection.set", json!({"model": "tiny.en"})),
        )
        .unwrap();
        assert_eq!(sel.dictation_model_id, "tiny.en");
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let engines = daemon.result("speech.engines", json!({}));
            let whisper = &engines["engines"][0];
            if whisper["running"] == true
                && whisper["model"]
                    .as_str()
                    .is_some_and(|m| m.ends_with("ggml-tiny.en.bin"))
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "engine not warm after the selection: {whisper}"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        // The selected model is protected; force removes it.
        let err = daemon.request(
            "speech.models.delete",
            json!({"provider": "whisper", "model": "tiny.en"}),
        );
        assert_eq!(err["error"]["data"]["app_code"], "CONFLICT");
        let del: ModelDeleteResult = serde_json::from_value(daemon.result(
            "speech.models.delete",
            json!({"provider": "whisper", "model": "tiny.en", "force": true}),
        ))
        .unwrap();
        assert!(del.deleted);
        assert_eq!(
            serde_json::to_value(del.model.readiness).unwrap(),
            "missing"
        );
    } else {
        eprintln!("skip: local tiny.en missing, the warm-engine half of R4 not exercised");
        let err = daemon.request(
            "speech.models.delete",
            json!({"provider": "whisper", "model": "test"}),
        );
        assert_eq!(err["error"]["data"]["app_code"], "CONFLICT");
    }

    // A missing model never downloads on selection.
    let before = rig.server.seen().len();
    let _ = daemon.request(
        "speech.models.cancel",
        json!({"provider": "whisper", "model": "test"}),
    );
    assert_eq!(rig.server.seen().len(), before);
    let log = daemon.stop();
    assert!(log.contains("model download started"), "{log}");
    assert!(log.contains("preload"), "{log}");
}

#[test]
fn a_corrupted_model_on_disk_is_quarantined_at_start() {
    let rig = rig();
    let models = rig.tree.root().join("data/dettivo/models/whisper/test");
    std::fs::create_dir_all(&models).unwrap();
    std::fs::write(models.join("ggml-test.bin"), b"not the model").unwrap();
    let daemon = Daemon::spawn(rig.tree, &[]);
    let row = wait_for(&daemon, "test", "quarantined", Duration::from_secs(10));
    assert!(row.path.is_none());
    assert!(!models.join("ggml-test.bin").exists());
    let log = daemon.stop();
    assert!(log.contains("quarantined"), "{log}");
}

#[test]
fn a_session_never_loads_a_catalogue_model_that_did_not_verify() {
    // No verification at start: the first load has to hash the file
    // itself, and a corrupt one is quarantined instead of opened.
    let rig = rig();
    let config = std::fs::read_to_string(rig.tree.config_file()).unwrap();
    rig.tree
        .write_config(&format!("{config}verify_on_start = false\n"));
    let models = rig.tree.root().join("data/dettivo/models/whisper/test");
    std::fs::create_dir_all(&models).unwrap();
    std::fs::write(models.join("ggml-test.bin"), b"not the model").unwrap();
    let daemon = Daemon::spawn(rig.tree, &[("DETTIVO_QA_MODE", "1")]);
    // The start-up preload hashes the selected model before the engine
    // opens it and moves the corrupt file aside.
    let row = wait_for(&daemon, "test", "quarantined", Duration::from_secs(5));
    assert!(row.path.is_none());
    assert!(
        !models.join("ggml-test.bin").exists(),
        "the corrupt file is moved aside, never loaded"
    );
    let refused = daemon.request("dictation.start", json!({"language": "en", "mode": "raw"}));
    let message = refused["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("whisper/test failed verification and is quarantined"),
        "{refused}"
    );
    let log = daemon.stop();
    assert!(log.contains("hashing the model before it loads"), "{log}");
    assert!(log.contains("preload skipped"), "{log}");
    assert!(
        !log.contains("model loaded"),
        "no engine opened the file: {log}"
    );
}

#[test]
fn speech_fixtures_parse_into_their_shapes() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dettivo-proto/fixtures/speech");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let method = doc["method"].as_str().unwrap();
        let Some(result) = doc.get("response").map(|r| r["result"].clone()) else {
            continue;
        };
        match method {
            "speech.models.status" => {
                serde_json::from_value::<ModelsStatusResult>(result).unwrap();
            }
            "speech.models.download" | "speech.models.cancel" => {
                serde_json::from_value::<ModelStatus>(result).unwrap();
            }
            "speech.models.delete" => {
                serde_json::from_value::<ModelDeleteResult>(result).unwrap();
            }
            "speech.providers.list" => {
                serde_json::from_value::<ProvidersListResult>(result).unwrap();
            }
            "speech.selection.get" | "speech.selection.set" => {
                serde_json::from_value::<SelectionResult>(result).unwrap();
            }
            _ => continue,
        }
        checked += 1;
    }
    assert!(checked >= 7, "{checked}");
}
