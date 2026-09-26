//! fn-32 R2, R3 and R5 against a live daemon with the mock tracks, tiny.en
//! and the diarization model set: a two-track meeting whose system track
//! is the two-speaker fixture finalises and the post-pass runs by itself,
//! labelling the remote segments under the coverage and share rule and
//! the microphone segments `You`, with `job.progress` under `diarizing`
//! and `meeting.state` carrying the status; a re-run through
//! `meetings.diarize` replaces the assignment and refuses while one runs
//! or before the meeting completed; a microphone-only import diarizes the
//! whole track as room audio; a rename lands on every segment, the
//! exports, the search index and the suggestions; and the speaker
//! fixtures replay against the seed. The crash isolation is in
//! `meetings_diarize_crash.rs`. Skipped without the models.

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use common::diarize::{
    diarization_model, expected_turns, export_text, import_meeting, two_speakers, wait_diarization,
    wait_meeting, with_diarization,
};
use common::meetings::{
    SAMPLE, Subscriber, env, fixture, spawn, start_params, track_fixture, tree,
};
use serde_json::{Value, json};

/// Every remote segment carries a speaker (ADR 0072), and each one a
/// clear majority labelled maps onto the expected turn under its
/// midpoint, one label per expected speaker.
fn check_remote_labels(segments: &[Value]) -> BTreeMap<String, String> {
    let turns = expected_turns();
    let mut mapping: BTreeMap<String, String> = BTreeMap::new();
    let mut labelled = 0;
    for s in segments.iter().filter(|s| s["source_type"] == "system") {
        let id = s["speaker_id"]
            .as_str()
            .unwrap_or_else(|| panic!("an unlabelled remote segment: {s}"));
        labelled += 1;
        assert!(id.starts_with("speaker_"), "{s}");
        if s["speaker_confidence"].as_f64().unwrap() < 0.6 {
            continue;
        }
        let mid = (s["start_ms"].as_u64().unwrap() + s["end_ms"].as_u64().unwrap()) / 2;
        let Some((expected, _, _)) = turns.iter().find(|(_, a, b)| *a <= mid && mid < *b) else {
            continue;
        };
        let known = mapping
            .entry(expected.clone())
            .or_insert_with(|| id.to_string());
        assert_eq!(known, id, "speaker mapping drifted on {s}");
    }
    assert!(
        labelled >= 2,
        "remote segments carry speakers: {segments:?}"
    );
    mapping
}

#[test]
fn a_two_track_meeting_learns_its_speakers_and_a_rerun_replaces_them() {
    // The live path is off: the finalisation and the pass are what this
    // test judges, and a busy CPU must not stretch the stop.
    let (Some(tree), Some(model)) = (tree("[meetings]\nlive = false\n"), diarization_model())
    else {
        eprintln!("skip: tiny.en, jfk.wav or the diarization model set missing");
        return;
    };
    with_diarization(&tree, &model);
    let mic = track_fixture("mic");
    let daemon = spawn(tree, &env(&mic, &two_speakers(), &[]));
    let mut sub = Subscriber::open(&daemon, &["meeting.state", "job.progress"]);
    let started = daemon.result(
        "meetings.start",
        json!({"capture": {"microphone": true, "system_audio": true}, "title": "Two voices", "acknowledge_meeting_disclosure": true, "expected_speakers": 2}),
    );
    let id = started["ref"]["id"].as_str().unwrap().to_string();
    // The system fixture is nineteen seconds; the microphone clip ends first.
    std::thread::sleep(Duration::from_millis(19_800));
    daemon.result("meetings.stop", json!({"meeting_id": id}));
    wait_meeting(&daemon, &id, "completed");
    let row = wait_diarization(&daemon, &id, "ready");
    assert_eq!(row["status"], "completed");
    let events = sub.collect(
        |p| p["topic"] == "meeting.state" && p["payload"]["diarization_status"] == "ready",
        Duration::from_secs(60),
    );
    assert!(
        events.iter().any(|p| p["topic"] == "job.progress"
            && p["payload"]["stage"] == "diarizing"
            && p["payload"]["job_id"] == "job_diarize_1"),
        "{events:?}"
    );
    // The `completed` transition already names the planned pass, and the
    // pass reports `running` only after it.
    let states: Vec<&Value> = events
        .iter()
        .filter(|p| p["topic"] == "meeting.state")
        .collect();
    let queued = states
        .iter()
        .position(|p| {
            p["payload"]["state"] == "completed" && p["payload"]["diarization_status"] == "queued"
        })
        .unwrap_or_else(|| panic!("no queued transition: {events:?}"));
    let running = states
        .iter()
        .position(|p| p["payload"]["diarization_status"] == "running")
        .unwrap_or_else(|| panic!("no running transition: {events:?}"));
    assert!(queued < running, "{events:?}");
    let speakers = row["speakers"].as_array().unwrap();
    assert_eq!(speakers[0]["speaker_id"], "you");
    assert_eq!(speakers[0]["name"], "You");
    assert_eq!(speakers[0]["color_index"], 0);
    assert_eq!(speakers.len(), 3, "you and two voices: {speakers:?}");
    assert!(
        speakers.iter().all(|s| s["talk_ms"].as_u64().unwrap() > 0),
        "{speakers:?}"
    );
    let segments = row["segments"].as_array().unwrap();
    for s in segments.iter().filter(|s| s["source_type"] == "microphone") {
        assert_eq!(s["speaker"], "You", "{s}");
        assert_eq!(s["speaker_id"], "you");
    }
    let mapping = check_remote_labels(segments);
    assert_eq!(mapping.len(), 2, "{mapping:?}");
    assert_eq!(row["diarization"]["engine"], "dettivo-engine-diarize");
    assert_eq!(row["diarization"]["model"], "diarize/diarization-en");
    assert!(row["diarization"]["coverage"].as_f64().unwrap() > 0.5);
    assert_eq!(row["diarization"]["expected_speakers"], 2);
    let listed = daemon.result("meetings.list", json!({"limit": 5, "cursor": null}));
    assert_eq!(listed["items"][0]["speaker_count"], 3);

    // A rename, then a re-run keeps the name by id (same count) and
    // refuses while it runs.
    let remote = mapping.values().next().unwrap().clone();
    daemon.result(
        "meetings.speakers.rename",
        json!({"meeting_id": id, "speaker_id": remote, "name": "Amy"}),
    );
    let text_of = |row: &Value| {
        row["segments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["text"].as_str().unwrap().trim().to_string())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let text_before = text_of(&row);
    let rerun = daemon.result("meetings.diarize", json!({"meeting_id": id, "speakers": 2}));
    assert_eq!(rerun["job"]["job_id"], "job_diarize_2");
    assert_eq!(rerun["job"]["message"], "diarizing");
    let busy = daemon.request("meetings.diarize", json!({"meeting_id": id}));
    assert_eq!(
        busy["error"]["data"]["details"]["kind"],
        "diarizationRunning"
    );
    let renamed_while = daemon.request(
        "meetings.speakers.rename",
        json!({"meeting_id": id, "speaker_id": "you", "name": "Me"}),
    );
    assert_eq!(
        renamed_while["error"]["data"]["details"]["kind"],
        "diarizationRunning"
    );
    let status = daemon.result("meetings.status", json!({"meeting_id": id}));
    assert_eq!(status["status"], "completed");
    assert!(
        status["job"].is_null() || status["job"]["message"] == "diarizing",
        "{status}"
    );
    let row = wait_diarization(&daemon, &id, "ready");
    assert!(
        row["speakers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["name"] == "Amy"),
        "{}",
        row["speakers"]
    );
    assert!(
        row["segments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["speaker"] == "Amy"),
        "the name follows the segments through a re-run"
    );
    // The re-run relabels without losing a word, and every part it split
    // is polished again.
    assert_eq!(text_of(&row), text_before);
    assert!(
        row["segments"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["polished_text"].is_string()),
        "{}",
        row["segments"]
    );
    daemon.stop();
}

#[test]
fn a_room_audio_import_diarizes_the_whole_track_and_renames_reach_everything() {
    let (Some(tree), Some(model)) = (tree(""), diarization_model()) else {
        eprintln!("skip: tiny.en, jfk.wav or the diarization model set missing");
        return;
    };
    with_diarization(&tree, &model);
    let mic = track_fixture("mic");
    let daemon = spawn(tree, &env(&mic, &mic, &[("DETTIVO_E2E_SEED", "1")]));
    let id = import_meeting(&daemon, json!({"expected_speakers": 2}));
    let row = wait_diarization(&daemon, &id, "ready");
    let speakers = row["speakers"].as_array().unwrap();
    assert_eq!(speakers.len(), 2, "room audio has no `you`: {speakers:?}");
    assert!(speakers.iter().all(|s| s["speaker_id"] != "you"));
    let segments = row["segments"].as_array().unwrap();
    assert!(segments.iter().all(|s| s["source_type"] == "microphone"));
    assert!(
        segments
            .iter()
            .filter(|s| s["speaker_id"].is_string())
            .count()
            >= 2,
        "{segments:?}"
    );
    let first = speakers[0]["speaker_id"].as_str().unwrap().to_string();

    // meetings.speakers.list, then the rename across the meeting.
    let list = daemon.result("meetings.speakers.list", json!({"meeting_id": id}));
    assert_eq!(list["speakers"], row["speakers"]);
    assert_eq!(list["diarization"]["status"], "ready");
    let renamed = daemon.result(
        "meetings.speakers.rename",
        json!({"meeting_id": id, "speaker_id": first, "name": "Ada Lovelace"}),
    );
    assert_eq!(renamed["speaker"]["name"], "Ada Lovelace");
    let updated = renamed["segments_updated"].as_u64().unwrap();
    assert!(updated >= 1, "{renamed}");
    let row = daemon.result("meetings.get", json!({"meeting_id": id}));
    assert_eq!(
        row["segments"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["speaker"] == "Ada Lovelace")
            .count() as u64,
        updated
    );
    for format in ["txt", "md", "srt", "vtt", "json"] {
        let text = export_text(&daemon, &id, format);
        assert!(text.contains("Ada Lovelace"), "{format}: {text}");
    }
    let hits = daemon.result("meetings.search", json!({"query": "lovelace", "limit": 5}));
    assert!(
        hits["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["ref"]["id"] == id),
        "the search index carries the name: {hits}"
    );
    let suggested = daemon.result("meetings.speakers.suggest", json!({}));
    assert_eq!(suggested["names"][0]["name"], "Ada Lovelace");
    // An empty name restores the label; the first speaker to appear is
    // speaker_00, Speaker 1.
    assert_eq!(first, "speaker_00");
    let restored = daemon.result(
        "meetings.speakers.rename",
        json!({"meeting_id": id, "speaker_id": first, "name": ""}),
    );
    assert_eq!(restored["speaker"]["name"], "Speaker 1");

    // The stateful speaker fixtures replay against the seed.
    let list = daemon.result("meetings.speakers.list", json!({"meeting_id": SAMPLE}));
    assert_eq!(
        list,
        fixture("meetings/speakers.list.json")["response"]["result"]
    );
    let renamed = daemon.result(
        "meetings.speakers.rename",
        json!({"meeting_id": SAMPLE, "speaker_id": "you", "name": "Gordon"}),
    );
    assert_eq!(
        renamed,
        fixture("meetings/speakers.rename.json")["response"]["result"]
    );
    let mut suggested = daemon.result(
        "meetings.speakers.suggest",
        json!({"prefix": "g", "limit": 5}),
    );
    suggested["names"][0]["last_used_at"] = json!("2026-02-13T16:40:00Z");
    assert_eq!(
        suggested,
        fixture("meetings/speakers.suggest.json")["response"]["result"]
    );
    let long = daemon.request(
        "meetings.speakers.rename",
        json!({"meeting_id": SAMPLE, "speaker_id": "you", "name": "x".repeat(65)}),
    );
    assert_eq!(
        long["error"],
        fixture("meetings/speakers.rename.error-invalid-params-too-long.json")["error"]["error"]
    );
    let unknown = daemon.request(
        "meetings.speakers.rename",
        json!({"meeting_id": SAMPLE, "speaker_id": "speaker_07", "name": "Ada"}),
    );
    assert_eq!(
        unknown["error"],
        fixture("meetings/speakers.rename.error-not-found-speaker.json")["error"]["error"]
    );
    let started = daemon.result(
        "meetings.diarize",
        json!({"meeting_id": SAMPLE, "speakers": 2}),
    );
    let mut want = fixture("meetings/diarize.json")["response"]["result"].clone();
    want["job"]["job_id"] = started["job"]["job_id"].clone();
    assert_eq!(started, want);
    // A meeting still recording is refused by name.
    let recording = daemon.result("meetings.start", start_params("Later"));
    let recording_id = recording["ref"]["id"].as_str().unwrap().to_string();
    let early = daemon.request("meetings.diarize", json!({"meeting_id": recording_id}));
    assert_eq!(
        early["error"]["data"]["details"]["kind"],
        "meetingNotCompleted"
    );
    daemon.result("meetings.cancel", json!({"meeting_id": recording_id}));
    daemon.stop();
}

#[test]
fn a_missing_model_set_leaves_the_meeting_completed_and_unavailable() {
    let Some(tree) = tree("") else {
        eprintln!("skip: tiny.en or jfk.wav missing");
        return;
    };
    let mic = track_fixture("mic");
    let daemon = spawn(tree, &env(&mic, &mic, &[("DETTIVO_E2E_SEED", "1")]));
    let refused = daemon.request("meetings.diarize", json!({"meeting_id": SAMPLE}));
    assert_eq!(
        refused["error"],
        fixture("meetings/diarize.error-not-found-model-missing.json")["error"]["error"]
    );
    let row = daemon.result("meetings.get", json!({"meeting_id": SAMPLE}));
    assert_eq!(row["status"], "completed");
    assert_eq!(row["diarization"]["status"], "unavailable");
    assert!(
        row["diarization"]["error"]
            .as_str()
            .unwrap()
            .contains("dettivo speech download --provider diarize --model diarization-en")
    );
    // An import under auto lands completed with its transcript, the
    // block unavailable.
    let id = import_meeting(&daemon, json!({}));
    let row = wait_diarization(&daemon, &id, "unavailable");
    assert_eq!(row["status"], "completed");
    assert!(!row["segments"].as_array().unwrap().is_empty());
    assert!(row["speakers"].as_array().is_none_or(|s| s.is_empty()));
    daemon.stop();
}
