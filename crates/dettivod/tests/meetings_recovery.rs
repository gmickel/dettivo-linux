//! fn-24 R3 and fn-28 R3 against a live daemon with the mock fixtures and
//! the local tiny.en: a SIGKILL mid-meeting followed by a restart that
//! promotes the meeting to partial with its takes intact and the
//! checkpoint's live tail on the row, `meetings.recover` that answers
//! `transcribing` and finalises the takes to `completed`, and
//! `meetings.discard` on a second interrupted meeting whose checkpoint
//! cannot be read. Skipped without the model.

mod common;

use std::time::Duration;

use common::meetings::{env, fixture, spawn, start_params, track_fixture, tree, wait_status};
use serde_json::{Value, json};

#[test]
fn a_killed_daemon_promotes_the_meeting_to_partial_and_recover_and_discard_settle_it() {
    let Some(tree) = tree("[meetings]\ncheckpoint_interval_seconds = 1\n") else {
        return;
    };
    let mic = track_fixture("mic");
    let daemon = spawn(tree, &env(&mic, &mic, &[]));
    let started = daemon.result("meetings.start", start_params("Interrupted"));
    let id = started["ref"]["id"].as_str().unwrap().to_string();
    std::thread::sleep(Duration::from_millis(2600));
    let meeting_dir = daemon.tree.root().join("data/dettivo/meetings").join(&id);
    assert!(meeting_dir.join("live-checkpoint.json").is_file());
    let checkpoint: Value = serde_json::from_str(
        &std::fs::read_to_string(meeting_dir.join("live-checkpoint.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(checkpoint["schema_version"], 1);
    assert_eq!(checkpoint["meeting_id"], id);
    assert_eq!(checkpoint["takes"].as_array().unwrap().len(), 2);
    let tree = daemon.kill_keep();

    let daemon = spawn(tree, &env(&mic, &mic, &[]));
    let status = daemon.result("meetings.status", json!({"meeting_id": id}));
    assert_eq!(status["status"], "partial", "{status}");
    // The checkpoint's live tail shows on the partial row until the
    // finalisation replaces it.
    let partial_row = daemon.result("meetings.get", json!({"meeting_id": id}));
    let tail_segments = partial_row["segments"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    assert_eq!(status["capture"]["is_partial"], true);
    assert_eq!(status["capture"]["chunks_completed"], 0);
    assert_eq!(status["capture"]["chunks_total"], 0);
    assert_eq!(status["capture"]["microphone_takes"], 1);
    // The seed carries a partial meeting of its own; this one is listed
    // beside it.
    let mine = status["recoverable"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["ref"]["id"] == id)
        .unwrap_or_else(|| panic!("{status}"))
        .clone();
    assert!(mine["duration_ms"].as_u64().unwrap() >= 1500);
    let mic_wav = hound::WavReader::open(meeting_dir.join("microphone.wav")).unwrap();
    assert!(
        mic_wav.len() >= 24_000,
        "the takes are intact: {} samples",
        mic_wav.len()
    );
    let journal = std::fs::read_to_string(meeting_dir.join("journal.jsonl")).unwrap();
    assert!(journal.contains("\"event\":\"recovered\""));
    let listed = daemon.result("meetings.list", json!({"limit": 20, "cursor": null}));
    assert_eq!(listed["items"][0]["status"], "partial");
    let mut recovered = daemon.result("meetings.recover", json!({"meeting_id": id}));
    let mut want = fixture("meetings/recover.json")["response"]["result"].clone();
    want["ref"]["id"] = json!(id);
    assert!(recovered["duration_ms"].as_u64().unwrap() >= 1500);
    recovered["duration_ms"] = want["duration_ms"].clone();
    assert_eq!(recovered, want);
    // The recovery finalises from the takes (fn-28 R3): transcribing,
    // then completed with the transcript and no partial flag.
    let row = wait_status(&daemon, &id, "completed");
    assert_eq!(row["capture"]["is_partial"], false, "{row}");
    assert!(
        row["capture"]["chunks_total"].as_u64().unwrap() >= 2,
        "{row}"
    );
    let row = daemon.result("meetings.get", json!({"meeting_id": id}));
    assert_eq!(row["status"], "completed");
    assert!(
        row["segments"].as_array().unwrap().len() >= tail_segments.min(1),
        "{row}"
    );
    assert!(meeting_dir.join("microphone.wav").is_file());
    let status = daemon.result("meetings.status", json!({"meeting_id": id}));
    assert!(
        status["recoverable"]
            .as_array()
            .into_iter()
            .flatten()
            .all(|r| r["ref"]["id"] != id),
        "{status}"
    );
    let again = daemon.request("meetings.recover", json!({"meeting_id": id}));
    assert_eq!(
        again["error"]["data"]["details"]["kind"],
        "meetingNotPartial"
    );

    // A second interruption whose checkpoint cannot be read still keeps
    // the audio and names the reason; discard removes it all.
    let second = daemon.result("meetings.start", start_params("Discarded"));
    let second_id = second["ref"]["id"].as_str().unwrap().to_string();
    std::thread::sleep(Duration::from_millis(1600));
    let second_dir = daemon
        .tree
        .root()
        .join("data/dettivo/meetings")
        .join(&second_id);
    let tree = daemon.kill_keep();
    std::fs::write(second_dir.join("live-checkpoint.json"), "{").unwrap();
    let daemon = spawn(tree, &env(&mic, &mic, &[]));
    let status = daemon.result("meetings.status", json!({"meeting_id": second_id}));
    assert_eq!(status["status"], "partial");
    assert!(
        status["capture"]["reason"]
            .as_str()
            .unwrap()
            .contains("not a checkpoint"),
        "{status}"
    );
    assert!(
        hound::WavReader::open(second_dir.join("microphone.wav"))
            .unwrap()
            .len()
            > 8_000
    );
    let mut discarded = daemon.result("meetings.discard", json!({"meeting_id": second_id}));
    let mut want = fixture("meetings/discard.json")["response"]["result"].clone();
    want["ref"]["id"] = json!(second_id);
    discarded["ref"]["id"] = json!(second_id);
    assert_eq!(discarded, want);
    assert!(!second_dir.exists());
    assert_eq!(
        daemon.request("meetings.get", json!({"meeting_id": second_id}))["error"]["data"]["app_code"],
        "NOT_FOUND"
    );
    daemon.stop();
}
