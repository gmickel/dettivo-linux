//! R2 and R5 against a live daemon and the fixture model server: the
//! `llm.models.*` methods over a catalogue file whose `llm` entry points
//! at the server, the `NOT_FOUND` naming the catalogue for an unknown id,
//! `[llm] model` marking the selection, the selected model needing
//! `force` to delete, the `local` provider and
//! `system.capabilities.llm.local_available` following the file on disk,
//! `llm.engine.status` before any load, and the stateful `llm/*.json`
//! fixtures parsing into their shapes.

mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::{Daemon, Tree};
use dettivo_proto::methods::llm::{EngineStatusResult, ModelDeleteResult, ModelsStatusResult};
use dettivo_proto::methods::speech::ModelStatus;
use dettivo_speech::fixture_server::FixtureServer;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// A small pseudo-random model file served by the fixture server.
fn write_fixture(dir: &Path, name: &str, len: usize) -> String {
    let mut bytes = Vec::with_capacity(len);
    let mut x: u32 = 0x2545_f491;
    for _ in 0..len {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        bytes.push((x & 0xff) as u8);
    }
    std::fs::write(dir.join(name), &bytes).unwrap();
    format!("{:x}", Sha256::digest(&bytes))
}

struct Rig {
    tree: Tree,
    _server: FixtureServer,
    _server_dir: tempfile::TempDir,
}

/// A tree whose catalogue names `llm/test-small` (selected) and
/// `llm/test-large` on the fixture server, beside one speech model.
fn rig() -> Rig {
    let tree = Tree::new();
    let server_dir = tempfile::Builder::new()
        .prefix("dtvllm")
        .tempdir_in("/tmp")
        .unwrap();
    let small = write_fixture(server_dir.path(), "small.gguf", 200_000);
    let large = write_fixture(server_dir.path(), "large.gguf", 400_000);
    let server = FixtureServer::serve(server_dir.path().to_path_buf());
    let catalogue = format!(
        r#"version = 9

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
redistribution = "test"
languages = ["en"]
english_only = true
default = true

[[models]]
provider = "llm"
id = "test-small"
display_name = "Test Small"
kind = "llm"
file_name = "small.gguf"
size_bytes = 200000
url = "{url}/small.gguf"
sha256 = "{small}"
license = "Apache-2.0"
redistribution = "test"
languages = ["multilingual"]
quantization = "Q4_K_M"
roles = ["polish"]
default = true

[[models]]
provider = "llm"
id = "test-large"
display_name = "Test Large"
kind = "llm"
file_name = "large.gguf"
size_bytes = 400000
url = "{url}/large.gguf"
sha256 = "{large}"
license = "Apache-2.0"
redistribution = "test"
languages = ["multilingual"]
quantization = "Q4_K_M"
roles = ["analysis"]
"#,
        url = server.url,
    );
    let catalogue_path = tree.root().join("catalogue.toml");
    std::fs::write(&catalogue_path, catalogue).unwrap();
    let bin_dir = Path::new(env!("CARGO_BIN_EXE_dettivod"))
        .parent()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    tree.write_config(&format!(
        "[engines]\ndirectory = \"{bin_dir}\"\n[speech]\nmodel = \"tiny.en\"\n[llm]\nmodel = \"test-small\"\nollama_url = \"http://127.0.0.1:1\"\n[models]\ncatalogue_file = \"{}\"\n",
        catalogue_path.display()
    ));
    Rig {
        tree,
        _server: server,
        _server_dir: server_dir,
    }
}

fn wait_for(daemon: &Daemon, model: &str, want: &str, budget: Duration) -> ModelStatus {
    let deadline = Instant::now() + budget;
    loop {
        let v = daemon.result("llm.models.status", json!({"model": model}));
        let r: ModelsStatusResult = serde_json::from_value(v).unwrap();
        let row = r.models.into_iter().next().unwrap();
        let state = serde_json::to_value(row.readiness).unwrap();
        if state == want {
            return row;
        }
        assert!(
            Instant::now() < deadline,
            "{model} stayed {state} waiting for {want}: {row:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn the_llm_catalogue_downloads_selects_and_deletes_through_the_llm_methods() {
    let rig = rig();
    let daemon = Daemon::spawn(rig.tree, &[("DETTIVO_QA_MODE", "1")]);

    // Before any download: the catalogue rows, the selection, the
    // provider down with the download command, and the flag off.
    let status: ModelsStatusResult =
        serde_json::from_value(daemon.result("llm.models.status", json!({}))).unwrap();
    assert_eq!(status.selected, "test-small");
    assert_eq!(status.analysis_model, "test-small", "empty means the model");
    assert_eq!(status.catalogue_version, 9);
    let ids: Vec<&str> = status.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["test-small", "test-large"], "speech rows stay out");
    assert!(status.models[0].is_selected && status.models[0].is_default);
    assert!(!status.models[1].is_selected);
    let speech: Value = daemon.result("speech.models.status", json!({}));
    assert!(
        speech["models"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["provider"] == "whisper"),
        "language models stay out of speech.models.status: {speech}"
    );
    let providers = daemon.result("speech.providers.list", json!({}));
    assert!(
        providers["providers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["id"] != "llm"),
        "{providers}"
    );
    let llm = daemon.result("llm.providers.list", json!({}));
    let local = &llm["providers"][0];
    assert_eq!(local["id"], "local");
    assert_eq!(local["model"], "test-small");
    assert_eq!(local["available"], false, "{local}");
    assert_eq!(local["hint"], "dettivo llm download --model test-small");
    let caps = daemon.result("system.capabilities", json!({}));
    assert_eq!(caps["llm"]["local_available"], false, "{caps}");
    assert!(
        caps["llm"]["methods"]
            .as_array()
            .unwrap()
            .contains(&json!("llm.engine.status"))
    );

    // An unknown id is NOT_FOUND naming what the catalogue lists.
    let err = daemon.request("llm.models.download", json!({"model": "nope"}));
    assert_eq!(err["error"]["data"]["app_code"], "NOT_FOUND", "{err}");
    let message = err["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("test-small") && message.contains("test-large"),
        "{message}"
    );
    let err = daemon.request("llm.models.status", json!({"model": "nope"}));
    assert_eq!(err["error"]["data"]["app_code"], "NOT_FOUND", "{err}");
    let err = daemon.request(
        "speech.models.download",
        json!({"provider": "llm", "model": "test-small"}),
    );
    assert_eq!(
        err["error"]["data"]["app_code"], "INVALID_PARAMS",
        "a language model is not a speech model: {err}"
    );

    // The download lands verified and the provider comes up.
    let started = daemon.result("llm.models.download", json!({"model": "test-small"}));
    assert_eq!(started["provider"], "llm");
    assert!(matches!(
        started["readiness"].as_str(),
        Some("downloading" | "ready")
    ));
    let ready = wait_for(&daemon, "test-small", "ready", Duration::from_secs(20));
    assert!(
        ready
            .path
            .as_deref()
            .unwrap()
            .ends_with("llm/test-small/small.gguf")
    );
    let llm = daemon.result("llm.providers.list", json!({}));
    let local = &llm["providers"][0];
    assert_eq!(local["available"], true, "{local}");
    assert_eq!(llm["selected"], "local", "auto prefers the local engine");
    let caps = daemon.result("system.capabilities", json!({}));
    assert_eq!(caps["llm"]["local_available"], true, "{caps}");

    // The engine has not been asked for anything: found, not running.
    let engine: EngineStatusResult =
        serde_json::from_value(daemon.result("llm.engine.status", json!({}))).unwrap();
    assert_eq!(engine.binary, "dettivo-engine-llm");
    assert!(!engine.running && !engine.loaded && !engine.busy);
    assert_eq!(engine.idle_seconds, 600);
    assert_eq!(engine.crashes, 0);

    // A cancel of a download that is not running is honest about it, and
    // the selected model needs force to go.
    let cancelled = daemon.result("llm.models.cancel", json!({"model": "test-large"}));
    assert_eq!(cancelled["readiness"], "missing");
    let err = daemon.request("llm.models.delete", json!({"model": "test-small"}));
    assert_eq!(err["error"]["data"]["app_code"], "CONFLICT", "{err}");
    let deleted: ModelDeleteResult = serde_json::from_value(daemon.result(
        "llm.models.delete",
        json!({"model": "test-small", "force": true}),
    ))
    .unwrap();
    assert!(deleted.deleted);
    assert_eq!(
        serde_json::to_value(deleted.model.readiness).unwrap(),
        "missing"
    );
    let caps = daemon.result("system.capabilities", json!({}));
    assert_eq!(caps["llm"]["local_available"], false, "{caps}");
    daemon.stop();
}

/// The stateful `llm/*.json` fixtures parse into their shapes.
#[test]
fn the_llm_fixtures_parse_into_their_shapes() {
    type Check = fn(Value) -> Result<(), serde_json::Error>;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dettivo-proto/fixtures/llm");
    let checks: [(&str, Check); 5] = [
        ("models.status.json", |v| {
            serde_json::from_value::<ModelsStatusResult>(v).map(|_| ())
        }),
        ("models.download.json", |v| {
            serde_json::from_value::<ModelStatus>(v).map(|_| ())
        }),
        ("models.cancel.json", |v| {
            serde_json::from_value::<ModelStatus>(v).map(|_| ())
        }),
        ("models.delete.json", |v| {
            serde_json::from_value::<ModelDeleteResult>(v).map(|_| ())
        }),
        ("engine.status.json", |v| {
            serde_json::from_value::<EngineStatusResult>(v).map(|_| ())
        }),
    ];
    for (name, check) in checks {
        let doc: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join(name)).unwrap()).unwrap();
        check(doc["response"]["result"].clone()).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}
