//! The diarization helpers the `meetings_diarize*` test binaries share
//! (ADR 0035): the two-speaker fixture and its expected turns, the model
//! set on this machine linked into a tree, a meeting import of the
//! fixture, and a wait on the diarization block.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{DAEMON, Daemon, Tree};

/// The two-speaker fixture.
pub fn two_speakers() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dettivo-qa/fixtures/diarization/two-speakers.wav")
}

/// The expected turns of the fixture: speaker, start, end.
pub fn expected_turns() -> Vec<(String, u64, u64)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dettivo-qa/fixtures/diarization/two-speakers.turns.json");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    doc["turns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["speaker"].as_str().unwrap().to_string(),
                t["start_ms"].as_u64().unwrap(),
                t["end_ms"].as_u64().unwrap(),
            )
        })
        .collect()
}

/// The model set on this machine, when downloaded
/// (`scripts/models/fetch-diarization-model.sh`).
pub fn diarization_model() -> Option<PathBuf> {
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

/// Links the model set's files into a tree (the manifest is copied, so
/// the verification the daemon runs at start rewrites the tree's copy
/// and never the real one) and makes sure the engine is built.
pub fn with_diarization(tree: &Tree, model: &Path) {
    let dir = tree
        .root()
        .join("data/dettivo/models/diarize/diarization-en");
    std::fs::create_dir_all(&dir).unwrap();
    for file in ["segmentation.onnx", "embedding.onnx"] {
        std::os::unix::fs::symlink(model.join(file), dir.join(file)).unwrap();
    }
    if model.join("manifest.json").is_file() {
        std::fs::copy(model.join("manifest.json"), dir.join("manifest.json")).unwrap();
    }
    let bin_dir = Path::new(DAEMON).parent().unwrap();
    if !bin_dir.join("dettivo-engine-diarize").is_file() {
        let status =
            std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
                .args(["build", "-q", "-p", "dettivo-engine-diarize"])
                .status()
                .expect("cargo build");
        assert!(status.success());
    }
}

/// Polls `meetings.get` until the diarization block reaches `wanted`.
pub fn wait_diarization(daemon: &Daemon, id: &str, wanted: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let row = daemon.result("meetings.get", json!({"meeting_id": id}));
        if row["diarization"]["status"] == wanted {
            return row;
        }
        assert!(
            Instant::now() < deadline,
            "{id} never reached diarization {wanted}: {}",
            row["diarization"]
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Polls `meetings.status` until the meeting reaches `wanted`, with the
/// patience a finalisation over two long tracks needs.
pub fn wait_meeting(daemon: &Daemon, id: &str, wanted: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        let status = daemon.result("meetings.status", json!({"meeting_id": id}));
        if status["status"] == wanted {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "{id} never reached {wanted}: {status}"
        );
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Polls `meetings.status` until no diarization job runs on `id`.
pub fn wait_pass_ended(daemon: &Daemon, id: &str) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let status = daemon.result("meetings.status", json!({"meeting_id": id}));
        if status["job"].is_null() || status["job"]["message"] != "diarizing" {
            return;
        }
        assert!(Instant::now() < deadline, "the pass on {id} never ended");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn upload(daemon: &Daemon, bytes: &[u8]) -> String {
    let begun = daemon.result(
        "transfer.begin",
        json!({"direction": "upload", "content_type": "audio/wav", "size_hint": bytes.len()}),
    );
    let id = begun["transfer_id"].as_str().unwrap().to_string();
    let mut total = 0;
    for (i, chunk) in bytes.chunks(256 * 1024).enumerate() {
        daemon.result(
            "transfer.chunk",
            json!({"transfer_id": id, "seq": i + 1, "data_b64": base64::engine::general_purpose::STANDARD.encode(chunk)}),
        );
        total += 1;
    }
    daemon.result(
        "transfer.commit",
        json!({"transfer_id": id, "total_chunks": total, "sha256": format!("{:x}", Sha256::digest(bytes))}),
    );
    id
}

/// Imports the fixture as a meeting (with `extra` on the import params)
/// and waits for the transcription to complete.
pub fn import_meeting(daemon: &Daemon, extra: Value) -> String {
    let transfer = upload(daemon, &std::fs::read(two_speakers()).unwrap());
    let mut params = json!({"transfer_id": transfer, "target_kind": "meeting", "filename": "two-speakers.wav", "language": "en", "mode": "raw", "acknowledge_meeting_disclosure": true});
    for (k, v) in extra.as_object().unwrap() {
        params[k] = v.clone();
    }
    let imported = daemon.result("transcripts.import", params);
    let id = imported["ref"]["id"].as_str().unwrap().to_string();
    wait_meeting(daemon, &id, "completed");
    id
}

/// The bytes of a meeting export in `format`, pulled in one chunk.
pub fn export_text(daemon: &Daemon, id: &str, format: &str) -> String {
    let begun = daemon.result(
        "transfer.begin",
        json!({"direction": "download", "content_type": "application/octet-stream", "size_hint": 0}),
    );
    let transfer = begun["transfer_id"].as_str().unwrap().to_string();
    daemon.result(
        "transcripts.export",
        json!({"ref": {"kind": "meeting", "id": id}, "format": format, "transfer_id": transfer}),
    );
    let mut out = Vec::new();
    let mut seq = 1;
    loop {
        let chunk = daemon.result(
            "transfer.pull",
            json!({"transfer_id": transfer, "seq": seq}),
        );
        out.extend(
            base64::engine::general_purpose::STANDARD
                .decode(chunk["data_b64"].as_str().unwrap())
                .unwrap(),
        );
        if chunk["eof"] == true {
            return String::from_utf8(out).unwrap();
        }
        seq += 1;
    }
}
