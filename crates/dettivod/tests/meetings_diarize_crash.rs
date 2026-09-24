//! fn-32 R4: crash isolation. The scripted engine (`dettivo-engine-proto`'s
//! `fake-engine`, the same injection the supervisor's crash tests use)
//! stands in for `dettivo-engine-diarize`: it dies mid-`diarize` while a
//! `crash-on-diarize` marker sits beside it, so the pass fails with the
//! engine's last redacted stderr line on the row while the daemon keeps
//! answering `system.ping` and the meeting stays `completed` with its
//! transcript; then it dies on `load` (`crash-on-load`), and the third
//! crash in a row marks the engine degraded on `engine.state` and
//! `speech.engines` (a successful load clears the count, as the
//! supervisor has it); once the markers are gone and the backoff has
//! passed a re-run lands `ready`. Skipped without tiny.en and jfk.wav.

mod common;

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use common::diarize::{import_meeting, wait_diarization, wait_pass_ended};
use common::meetings::{Subscriber, env, spawn, track_fixture, tree, wait_status};
use serde_json::json;
use sha2::Digest;

/// Builds the scripted engine and links it as the diarization engine in a
/// directory of its own, with the crash marker set.
fn crashing_engine_dir() -> PathBuf {
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args([
            "build",
            "-q",
            "-p",
            "dettivo-engine-proto",
            "--example",
            "fake-engine",
        ])
        .status()
        .expect("cargo build");
    assert!(status.success());
    let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug");
    let dir = target.join(format!("qa-fake-diarize-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(
        target.join("examples/fake-engine"),
        dir.join("dettivo-engine-diarize"),
    )
    .unwrap();
    std::fs::write(dir.join("crash-on-diarize"), b"").unwrap();
    dir
}

/// A catalogue with the real tiny.en and a diarization set whose two
/// files are the four bytes `fake`.
fn fake_set_catalogue() -> String {
    let fake = format!("{:x}", sha2::Sha256::digest(b"fake"));
    format!(
        r#"version = 1

[[models]]
provider = "whisper"
id = "tiny.en"
display_name = "Tiny (English)"
kind = "stt"
file_name = "ggml-tiny.en.bin"
size_bytes = 77704715
url = "http://127.0.0.1:1/ggml-tiny.en.bin"
sha256 = "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f"
license = "MIT"
redistribution = "OpenAI Whisper weights converted by whisper.cpp"
languages = ["en"]
english_only = true
default = true

[[models]]
provider = "diarize"
id = "diarization-en"
display_name = "Fake speaker diarization"
kind = "diarization"
file_name = "segmentation.onnx"
size_bytes = 4
url = "http://127.0.0.1:1/segmentation.onnx"
sha256 = "{fake}"
license = "MIT"
redistribution = "test fixture"
languages = ["multilingual"]
default = true

[[models.files]]
file_name = "embedding.onnx"
size_bytes = 4
url = "http://127.0.0.1:1/embedding.onnx"
sha256 = "{fake}"
license = "MIT"
"#
    )
}

#[test]
fn an_engine_crash_fails_the_pass_keeps_the_daemon_and_degrades_after_three() {
    let Some(tree) = tree("") else {
        eprintln!("skip: tiny.en or jfk.wav missing");
        return;
    };
    let engine_dir = crashing_engine_dir();
    // The model set must look present: the fake engine never reads the
    // files, but the daemon verifies every catalogue model before an
    // engine loads it, so a catalogue of its own names the fake files'
    // checksums beside the real tiny.en.
    let model = tree
        .root()
        .join("data/dettivo/models/diarize/diarization-en");
    std::fs::create_dir_all(&model).unwrap();
    std::fs::write(model.join("segmentation.onnx"), b"fake").unwrap();
    std::fs::write(model.join("embedding.onnx"), b"fake").unwrap();
    let catalogue = tree.root().join("catalogue.toml");
    std::fs::write(&catalogue, fake_set_catalogue()).unwrap();
    // The engine directory is searched first; whisper is not in it and
    // comes from the build directory beside the daemon as before.
    let config = std::fs::read_to_string(tree.config_file()).unwrap();
    let (head, tail) = config.split_once("\n[speech]").unwrap();
    let head: Vec<&str> = head
        .lines()
        .filter(|l| !l.starts_with("directory = "))
        .collect();
    tree.write_config(&format!(
        "{}\ndirectory = \"{}\"\n[models]\ncatalogue_file = \"{}\"\n[speech]{tail}",
        head.join("\n"),
        engine_dir.display(),
        catalogue.display()
    ));
    let mic = track_fixture("mic");
    let daemon = spawn(tree, &env(&mic, &mic, &[]));
    let mut sub = Subscriber::open(&daemon, &["engine.state", "job.progress"]);
    let id = import_meeting(&daemon, json!({}));
    let row = wait_diarization(&daemon, &id, "failed");
    assert_eq!(row["status"], "completed");
    assert!(!row["segments"].as_array().unwrap().is_empty());
    let error = row["diarization"]["error"].as_str().unwrap();
    assert!(error.contains("crashing on diarize"), "{error}");
    assert_eq!(daemon.result("system.ping", json!({}))["ok"], true);

    // The next two passes die on load: three crashes without a good load
    // between them, and the supervisor degrades the engine.
    std::fs::remove_file(engine_dir.join("crash-on-diarize")).unwrap();
    std::fs::write(engine_dir.join("crash-on-load"), b"").unwrap();
    for expected_job in ["job_diarize_2", "job_diarize_3"] {
        let started = daemon.result("meetings.diarize", json!({"meeting_id": id}));
        assert_eq!(started["job"]["job_id"], expected_job);
        wait_pass_ended(&daemon, &id);
        let row = wait_diarization(&daemon, &id, "failed");
        assert!(
            row["diarization"]["error"]
                .as_str()
                .unwrap()
                .contains("crashing on load"),
            "{}",
            row["diarization"]
        );
        assert_eq!(daemon.result("system.ping", json!({}))["ok"], true);
    }
    let events = sub.collect(
        |p| {
            p["topic"] == "job.progress"
                && p["payload"]["job_id"] == "job_diarize_3"
                && p["payload"]["stage"] == "failed"
        },
        Duration::from_secs(20),
    );
    let degraded = events
        .iter()
        .find(|p| p["topic"] == "engine.state" && p["payload"]["state"] == "degraded")
        .unwrap_or_else(|| panic!("no degraded transition: {events:?}"));
    assert_eq!(degraded["payload"]["binary"], "dettivo-engine-diarize");
    assert!(
        events
            .iter()
            .filter(|p| p["topic"] == "job.progress" && p["payload"]["stage"] == "failed")
            .count()
            >= 3,
        "{events:?}"
    );
    let engines = daemon.result("speech.engines", json!({}));
    let row = engines["engines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["binary"] == "dettivo-engine-diarize")
        .unwrap();
    assert_eq!(row["degraded"], true, "{row}");
    assert_eq!(row["crashes"], 3);
    let meeting = daemon.result("meetings.get", json!({"meeting_id": id}));
    assert_eq!(meeting["status"], "completed");
    assert_eq!(meeting["diarization"]["status"], "failed");

    // The marker gone and the backoff over, a re-run lands ready and the
    // engine is whole again.
    std::fs::remove_file(engine_dir.join("crash-on-load")).unwrap();
    std::thread::sleep(Duration::from_millis(4_500));
    let started = daemon.result("meetings.diarize", json!({"meeting_id": id, "speakers": 2}));
    assert_eq!(started["job"]["job_id"], "job_diarize_4");
    let row = wait_diarization(&daemon, &id, "ready");
    assert_eq!(
        row["speakers"].as_array().unwrap().len(),
        2,
        "{}",
        row["speakers"]
    );
    let engines = daemon.result("speech.engines", json!({}));
    let row = engines["engines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["binary"] == "dettivo-engine-diarize")
        .unwrap();
    assert_eq!(row["degraded"], false, "{row}");
    assert_eq!(row["crashes"], 0);
    wait_status(&daemon, &id, "completed");
    for policy in ["keep", "audio_only", "none"] {
        daemon.result(
            "config.set",
            json!({"key": "meetings.keep_audio", "value": false}),
        );
        daemon.result(
            "config.set",
            json!({"key": "meetings.artifacts", "value": policy}),
        );
        let fresh = import_meeting(&daemon, json!({"diarize": true}));
        let ready = wait_diarization(&daemon, &fresh, "ready");
        assert_eq!(ready["speakers"].as_array().unwrap().len(), 2, "{policy}");
        let dir = daemon
            .tree
            .root()
            .join("data/dettivo/meetings")
            .join(&fresh);
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while dir.join("microphone.wav").exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "{policy}: temporary audio remains"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(dir.join("metadata.json").exists(), policy == "keep");
        assert_eq!(dir.exists(), policy != "none");
    }
    daemon.stop();
    let _ = std::fs::remove_dir_all(&engine_dir);
}
