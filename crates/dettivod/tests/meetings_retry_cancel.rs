//! Cancel an imported retry through the same meetings API the GUI uses.
mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;

use common::{Daemon, local_model, tree_with_tiny};
use dettivo_storage::{
    Store,
    meetings::{MeetingRow, MeetingStatus},
};
use serde_json::json;

struct EngineGate(Option<PathBuf>);

impl EngineGate {
    fn resume(&mut self) {
        if let Some(marker) = self.0.take()
            && let Ok(pid) = std::fs::read_to_string(marker)
            && let Ok(pid) = pid.trim().parse::<u32>()
        {
            let _ = std::process::Command::new("kill")
                .args(["-CONT", &pid.to_string()])
                .status();
        }
    }
}

impl Drop for EngineGate {
    fn drop(&mut self) {
        self.resume();
    }
}

#[test]
fn meetings_cancel_stops_an_import_retry_and_preserves_its_identity_notes_and_audio() {
    let contract: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/meeting-recovery-correctness.json")).unwrap();
    let cancel_method = contract["cancel_method"].as_str().unwrap();
    let Some((model, speech)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing");
        return;
    };
    let tree = tree_with_tiny(
        &model,
        "[transcribe]\nchunk_seconds = 30\noverlap_seconds = 1\nsafety_margin_seconds = 1\n",
    );
    let engine_dir = tree.root().join("gated-engine");
    std::fs::create_dir_all(&engine_dir).unwrap();
    let marker = engine_dir.join("pid");
    let real_dir = std::path::Path::new(common::DAEMON).parent().unwrap();
    let wrapper = engine_dir.join("dettivo-engine-whisper");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nprintf '%s' \"$$\" > '{}'\nkill -STOP \"$$\"\nexec '{}' \"$@\"\n",
            marker.display(),
            real_dir.join("dettivo-engine-whisper").display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let config = std::fs::read_to_string(tree.config_file())
        .unwrap()
        .replace(
            &format!("directory = \"{}\"", real_dir.display()),
            &format!("directory = \"{}\"", engine_dir.display()),
        )
        .replace("model = \"tiny.en\"", "model = \"large-v3-turbo\"");
    tree.write_config(&config);
    let data = tree.root().join("data/dettivo");
    let store = Store::open(&data.join("dettivo.db")).unwrap();
    let mut row = MeetingRow::new();
    row.source_kind = "audioImport".into();
    row.status = MeetingStatus::Failed;
    row.stt_provider = "whisper".into();
    row.stt_model = "tiny.en".into();
    row.notes_markdown = "Original user notes".into();
    let dir = data.join("meetings").join(&row.id);
    std::fs::create_dir_all(&dir).unwrap();
    let audio = dir.join(dettivo_storage::retention::AUDIO_FILE);
    std::fs::copy(speech, &audio).unwrap();
    assert!(
        hound::WavReader::open(&audio).unwrap().duration() <= 30 * 16_000,
        "the cancellation regression must use one chunk"
    );
    row.audio_dir = Some(dir.to_string_lossy().into_owned());
    store.insert_meeting(&row).unwrap();
    let mut completed = MeetingRow::new();
    completed.status = MeetingStatus::Completed;
    store.insert_meeting(&completed).unwrap();
    drop(store);
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_INSERT", "1"),
            ("DETTIVO_FORCE_CPU", "1"),
        ],
    );
    let mut gate = EngineGate(Some(marker.clone()));
    let recovered = daemon.result("meetings.recover", json!({"meeting_id": row.id}));
    assert_eq!(recovered["ref"]["id"], row.id);
    assert!(
        common::wait_for(Duration::from_secs(10), || {
            std::fs::read_to_string(&marker)
                .ok()
                .and_then(|s| s.parse::<u32>().ok())
                .and_then(|pid| std::fs::read_to_string(format!("/proc/{pid}/status")).ok())
                .is_some_and(|s| s.lines().any(|line| line.starts_with("State:\tT")))
        }),
        "retry never reached the stopped engine"
    );
    daemon.result(
        "meetings.notes.set",
        json!({"meeting_id": row.id, "markdown": "Notes edited during retry", "source": "user"}),
    );
    let cancelled = daemon.request(cancel_method, json!({"meeting_id": row.id}));
    gate.resume();
    assert_eq!(
        cancelled["result"]["job"]["state"], contract["cancel_job_state"],
        "{cancelled}"
    );
    assert_eq!(cancelled["result"]["ref"]["id"], row.id);
    assert!(common::wait_for(Duration::from_secs(30), || daemon
        .result("meetings.status", json!({"meeting_id": row.id}))["status"]
        == "cancelled"));
    let result = daemon.result("meetings.get", json!({"meeting_id": row.id}));
    assert_eq!(result["notes"], "Notes edited during retry");
    assert!(
        !result["transcript"].as_str().unwrap().is_empty(),
        "the last chunk's text must survive cancellation"
    );
    assert!(audio.is_file());
    let status = daemon.result("meetings.status", json!({"meeting_id": row.id}));
    assert!(
        status["recoverable"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["ref"]["id"] == row.id)
    );
    daemon.result("meetings.recover", json!({"meeting_id": row.id}));
    assert!(common::wait_for(Duration::from_secs(30), || daemon
        .result("meetings.status", json!({"meeting_id": row.id}))["status"]
        == "completed"));
    assert_eq!(
        daemon.result("meetings.get", json!({"meeting_id": row.id}))["notes"],
        "Notes edited during retry"
    );
    let refused = daemon.request(cancel_method, json!({"meeting_id": completed.id}));
    assert_eq!(
        refused["error"]["data"]["details"]["kind"],
        contract["cancel_completed_error"]
    );
    daemon.stop();
}
