//! Failed, cancelled and interrupted finalization retries preserve identity and notes.
mod common;

use common::{Daemon, local_model, tree_with_tiny};
use dettivo_storage::{
    Store,
    meetings::{MeetingRow, MeetingStatus},
};
use serde_json::json;
use std::time::Duration;

#[test]
fn finalization_failures_and_restart_are_retryable_for_capture_and_import() {
    let Some((model, _)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing");
        return;
    };
    let tree = tree_with_tiny(&model, "[meetings]\nkeep_audio = true\n");
    let data = tree.root().join("data/dettivo");
    let store = Store::open(&data.join("dettivo.db")).unwrap();
    let contract: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/meeting-recovery-correctness.json")).unwrap();
    let mut rows = Vec::new();
    for source_kind in contract["source_kinds"].as_array().unwrap() {
        let source_kind = source_kind.as_str().unwrap();
        for status in contract["recoverable_statuses"].as_array().unwrap() {
            let mut row = MeetingRow::new();
            row.status = serde_json::from_value(status.clone()).unwrap();
            row.source_kind = source_kind.into();
            row.stt_provider = "whisper".into();
            row.stt_model = "tiny.en".into();
            row.notes_markdown = format!("User notes for {}", row.id);
            row.error_code = Some("old error".into());
            row.error_message = Some("old failure".into());
            let dir = data.join("meetings").join(&row.id);
            std::fs::create_dir_all(&dir).unwrap();
            if source_kind == "audioImport" {
                dettivo_storage::seed::write_silence(
                    &dir.join(dettivo_storage::retention::AUDIO_FILE),
                )
                .unwrap();
            } else {
                let mut take = dettivo_audio::takes::TakeWriter::with_prefix(
                    &dir,
                    "microphone",
                    std::time::Instant::now(),
                )
                .unwrap();
                take.write(&[0; 16_000]).unwrap();
                take.finish().unwrap();
                row.microphone_takes = 1;
            }
            row.audio_dir = Some(dir.to_string_lossy().into());
            store.insert_meeting(&row).unwrap();
            rows.push(row);
        }
    }
    let mut damaged = Vec::new();
    for kind in contract["damaged_inputs"].as_array().unwrap() {
        let kind = kind.as_str().unwrap();
        let mut row = MeetingRow::new();
        row.status = MeetingStatus::Failed;
        row.source_kind = if kind == "truncated_import" {
            "audioImport"
        } else {
            "capture"
        }
        .into();
        let dir = data.join("meetings").join(&row.id);
        std::fs::create_dir_all(&dir).unwrap();
        if kind == "truncated_import" {
            let wav = dir.join(dettivo_storage::retention::AUDIO_FILE);
            dettivo_storage::seed::write_silence(&wav).unwrap();
            std::fs::OpenOptions::new()
                .write(true)
                .open(wav)
                .unwrap()
                .set_len(50)
                .unwrap();
        } else if kind == "missing_system" {
            let mut take = dettivo_audio::takes::TakeWriter::with_prefix(
                &dir,
                "microphone",
                std::time::Instant::now(),
            )
            .unwrap();
            take.write(&[0; 16_000]).unwrap();
            take.finish().unwrap();
            row.system_audio = true;
        }
        row.audio_dir = Some(dir.to_string_lossy().into_owned());
        store.insert_meeting(&row).unwrap();
        damaged.push(row);
    }
    drop(store);
    let daemon = Daemon::spawn(
        tree,
        &[("DETTIVO_QA_MODE", "1"), ("DETTIVO_MOCK_INSERT", "1")],
    );
    for original in rows {
        let status = daemon.result("meetings.status", json!({"meeting_id": original.id}));
        assert!(
            status["recoverable"]
                .as_array()
                .is_some_and(|rows| rows.iter().any(|r| r["ref"]["id"] == original.id)),
            "retry must be discoverable: {status}"
        );
        let recovered = daemon.result("meetings.recover", json!({"meeting_id": original.id}));
        assert_eq!(recovered["ref"]["id"], original.id);
        assert_eq!(recovered["status"], contract["retry_status"]);
        assert!(
            common::wait_for(Duration::from_secs(30), || {
                daemon.result("meetings.status", json!({"meeting_id": original.id}))["status"]
                    == "completed"
            }),
            "retry failed: {}",
            daemon.result("meetings.get", json!({"meeting_id": original.id}))
        );
        let row = daemon.result("meetings.get", json!({"meeting_id": original.id}));
        assert_eq!(row["notes"], original.notes_markdown);
        // `completed` can land before the retry's job has released the
        // meeting on a slow runner; the refusal is read once it has.
        let mut completed = daemon.request("meetings.recover", json!({"meeting_id": original.id}));
        common::wait_for(Duration::from_secs(30), || {
            completed = daemon.request("meetings.recover", json!({"meeting_id": original.id}));
            completed["error"]["data"]["details"]["kind"] != "jobRunning"
        });
        assert_eq!(
            completed["error"]["data"]["details"]["kind"],
            contract["completed_retry_error"]
        );
    }
    for row in damaged {
        let status = daemon.result("meetings.status", json!({"meeting_id": row.id}));
        assert!(
            !status["recoverable"]
                .as_array()
                .is_some_and(|rows| rows.iter().any(|r| r["ref"]["id"] == row.id)),
            "damaged input advertised: {status}"
        );
        let refused = daemon.request("meetings.recover", json!({"meeting_id": row.id}));
        assert_eq!(
            refused["error"]["data"]["details"]["kind"], contract["damaged_input_error"],
            "{refused}"
        );
        assert_eq!(
            daemon.result("meetings.status", json!({"meeting_id": row.id}))["status"],
            "failed"
        );
    }
    daemon.stop();
}
