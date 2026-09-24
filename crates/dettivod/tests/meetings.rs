//! fn-24 R1 and R4 and fn-28 R2, R4 and R5 against a live daemon with the
//! mock fixtures and the local tiny.en: the gates in order (one session,
//! the disclosure, the meeting-capable engine, the downloaded model), a
//! meeting that writes both tracks with a journal and metadata, streams
//! `meeting.segment` events while it records, answers the contract's
//! `transcribing` job on stop and finalises to `completed` with the
//! transcript on the row, the stateful `meetings.*` fixtures by name, and
//! a cancel that removes the takes. The kill and the recovery are in
//! `meetings_recovery.rs`. Skipped without the model.

mod common;

use std::time::Duration;

use common::Tree;
use common::meetings::{
    SAMPLE, Subscriber, env, fixture, spawn, start_params, track_fixture, tree, wait_status,
};
use serde_json::{Value, json};

#[test]
fn the_gates_refuse_in_order_and_the_disclosure_fixtures_answer() {
    let Some(tree) = tree("") else { return };
    let mic = track_fixture("mic");
    let daemon = spawn(tree, &env(&mic, &mic, &[]));
    let get = daemon.result("meetings.disclosure.get", json!({}));
    assert_eq!(
        get,
        fixture("meetings/disclosure.get.json")["response"]["result"]
    );
    let refused = daemon.request(
        "meetings.start",
        json!({"capture": {"microphone": true, "system_audio": true}}),
    );
    assert_eq!(
        refused["error"],
        fixture("meetings/start.error-conflict-disclosure-required.json")["error"]["error"]
    );
    let mut acknowledged = daemon.result("meetings.disclosure.acknowledge", json!({}));
    assert!(
        acknowledged["acknowledged_at"]
            .as_str()
            .unwrap()
            .ends_with('Z')
    );
    acknowledged["acknowledged_at"] = json!("2026-02-13T16:00:00Z");
    assert_eq!(
        acknowledged,
        fixture("meetings/disclosure.acknowledge.json")["response"]["result"]
    );
    let state = std::fs::read_to_string(daemon.tree.state_file()).unwrap();
    assert!(state.contains("meeting_disclosure = true"));
    assert!(state.contains("meeting_disclosure_at = "));
    // Acknowledged: the next start passes the gate without the flag.
    let started = daemon.result(
        "meetings.start",
        json!({"capture": {"microphone": true, "system_audio": false}}),
    );
    assert_eq!(started["job"]["state"], "running");
    let id = started["ref"]["id"].as_str().unwrap().to_string();
    // The dictation fixture: a dictation during a meeting is sessionActive.
    let dictation = daemon.request("dictation.start", json!({"language": "en", "mode": "raw"}));
    assert_eq!(
        dictation["error"],
        fixture("dictation/start.error-conflict-meeting-active.json")["error"]["error"]
    );
    let again = daemon.request("meetings.start", start_params("second"));
    assert_eq!(again["error"]["data"]["details"]["kind"], "sessionActive");
    let health = daemon.result("system.health", json!({}));
    assert_eq!(health["recording_state"], "meeting");
    assert_eq!(health["active_jobs"], 1);
    let status = daemon.result("meetings.status", json!({"meeting_id": id}));
    assert_eq!(
        status["capture"]["system_audio"], false,
        "system audio was not requested"
    );
    daemon.result("meetings.cancel", json!({"meeting_id": id}));
    let bad = daemon.request(
        "meetings.start",
        json!({"capture": {"microphone": false, "system_audio": true}, "acknowledge_meeting_disclosure": true}),
    );
    assert_eq!(bad["error"]["data"]["app_code"], "INVALID_PARAMS");
    // A model that is not downloaded is refused at start naming the
    // download command (fn-28), never mid-meeting.
    daemon.result(
        "config.set",
        json!({"key": "speech.meeting_model", "value": "small.en"}),
    );
    let missing = daemon.request("meetings.start", start_params("No model"));
    assert_eq!(missing["error"]["data"]["app_code"], "NOT_FOUND");
    assert!(
        missing["error"]["message"]
            .as_str()
            .unwrap()
            .contains("dettivo speech download --model small.en"),
        "{missing}"
    );
    daemon.stop();

    // A provider without meeting-capable timestamps is refused with the
    // settings link (the stateful fixture).
    let tree = Tree::new();
    tree.write_config("[speech]\nprovider = \"parakeet\"\n");
    let daemon = spawn(tree, &env(&mic, &mic, &[]));
    let refused = daemon.request("meetings.start", start_params("Weekly sync"));
    assert_eq!(
        refused["error"],
        fixture("meetings/start.error-conflict-engine-without-timestamps.json")["error"]["error"]
    );
    daemon.stop();
}

#[test]
fn whisper_meetings_leave_parakeet_dictation_selected() {
    let Some(tree) = tree("") else { return };
    let mic = track_fixture("independent-model");
    let daemon = spawn(tree, &env(&mic, &mic, &[]));
    daemon.result("speech.selection.set", json!({"provider": "parakeet"}));
    let selected = daemon.result("speech.selection.set", json!({"meeting_model": "tiny.en"}));
    assert_eq!(selected["dictation"]["provider_id"], "parakeet");
    assert_eq!(selected["meeting"]["provider_id"], "whisper");
    assert_eq!(selected["meeting"]["model_id"], "tiny.en");
    let started = daemon.result("meetings.start", start_params("Independent meeting model"));
    assert_eq!(started["job"]["state"], "running");
    daemon.result(
        "meetings.cancel",
        json!({"meeting_id": started["ref"]["id"]}),
    );
    let selected = daemon.result("speech.selection.get", json!({}));
    assert_eq!(selected["dictation"]["provider_id"], "parakeet");
    daemon.stop();
}

#[test]
fn a_meeting_writes_both_tracks_streams_segments_and_finalises_and_the_fixtures_replay() {
    let Some(tree) = tree("") else { return };
    let mic = track_fixture("mic");
    let daemon = spawn(tree, &env(&mic, &mic, &[]));
    // Levels arrive twenty times a second per track and would overflow a
    // subscriber that reads only after the meeting; they get their own.
    let mut levels = Subscriber::open(&daemon, &["audio.level"]);
    let mut sub = Subscriber::open(
        &daemon,
        &["meeting.state", "meeting.segment", "job.progress"],
    );
    let started = daemon.result("meetings.start", start_params("Weekly sync"));
    let expected = fixture("meetings/start.json")["response"]["result"].clone();
    assert_eq!(started["job"], expected["job"]);
    assert_eq!(started["ref"]["kind"], "meeting");
    let id = started["ref"]["id"].as_str().unwrap().to_string();
    let meeting_dir = daemon.tree.root().join("data/dettivo/meetings").join(&id);
    std::thread::sleep(Duration::from_millis(1200));

    // meetings/status.json: the live shape with the capture block; the
    // live counts are the meeting's own.
    let status = daemon.result("meetings.status", json!({"meeting_id": id}));
    let mut want = fixture("meetings/status.json")["response"]["result"].clone();
    want["ref"]["id"] = json!(id);
    want["live_segment_count"] = status["live_segment_count"].clone();
    want["live_last_end_ms"] = status["live_last_end_ms"].clone();
    let mut got = status.clone();
    let capture = got.as_object_mut().unwrap().remove("capture").unwrap();
    got.as_object_mut().unwrap().remove("recoverable");
    assert_eq!(got, want, "{status}");
    assert_eq!(capture["system_audio"], true);
    assert_eq!(capture["microphone_takes"], 1);
    assert!(capture["duration_ms"].as_u64().unwrap() >= 1000);
    assert!(meeting_dir.join("microphone.wav").is_file());
    assert!(meeting_dir.join("system.wav").is_file());
    let listed = daemon.result("meetings.list", json!({"limit": 20, "cursor": null}));
    assert_eq!(listed["items"][0]["ref"]["id"], id);
    assert_eq!(listed["items"][0]["status"], "recording");
    assert_eq!(listed["items"][0]["title"], "Weekly sync");
    let mut seen = std::collections::BTreeSet::new();
    levels.collect(
        |p| {
            if let Some(source) = p["payload"]["source"].as_str() {
                seen.insert(source.to_string());
            }
            false
        },
        Duration::from_millis(500),
    );
    drop(levels);
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        ["microphone", "system"]
    );

    // Let the live path run over the fixture: the jfk clip is eleven
    // seconds, so segments harden while it plays.
    std::thread::sleep(Duration::from_millis(6000));

    // meetings/stop.json: the contract's transcribing job at once, a
    // second stop the same, then the finalisation to completed.
    let stopped = daemon.result("meetings.stop", json!({"meeting_id": id}));
    let mut want = fixture("meetings/stop.json")["response"]["result"].clone();
    want["ref"]["id"] = json!(id);
    assert_eq!(stopped, want);
    let twice = daemon.result("meetings.stop", json!({"meeting_id": id}));
    assert_eq!(twice["job"]["state"], "running");
    let settled = wait_status(&daemon, &id, "completed");
    assert_eq!(settled["is_finalizing"], false);
    assert!(
        settled["live_segment_count"].as_u64().unwrap() >= 1,
        "{settled}"
    );
    let thrice = daemon.result("meetings.stop", json!({"meeting_id": id}));
    assert_eq!(thrice["job"]["state"], "succeeded");

    let events = sub.collect(
        |p| p["payload"]["state"] == "completed",
        Duration::from_secs(60),
    );
    let states: Vec<&str> = events
        .iter()
        .filter(|p| p["topic"] == "meeting.state")
        .map(|p| p["payload"]["state"].as_str().unwrap())
        .collect();
    assert_eq!(
        states,
        [
            "recording",
            "stopping",
            "stopped",
            "transcribing",
            "completed"
        ],
        "{events:?}"
    );
    // meeting.segment: provisional then final per source, in time order,
    // every payload the registered shape (fn-28 R2, R5).
    let segments: Vec<&Value> = events
        .iter()
        .filter(|p| p["topic"] == "meeting.segment")
        .map(|p| &p["payload"])
        .collect();
    assert!(
        !segments.is_empty(),
        "no meeting.segment events: {events:?}"
    );
    for source in ["you", "remote"] {
        let mine: Vec<&&Value> = segments.iter().filter(|s| s["source"] == source).collect();
        assert!(!mine.is_empty(), "{source} never spoke: {segments:?}");
        assert!(mine[0]["provisional"] == true, "{source}: {mine:?}");
        assert!(
            mine.iter().any(|s| s["provisional"] == false),
            "{source} never hardened: {mine:?}"
        );
        let finals: Vec<u64> = mine
            .iter()
            .filter(|s| s["provisional"] == false)
            .map(|s| s["start_ms"].as_u64().unwrap())
            .collect();
        assert!(finals.windows(2).all(|w| w[0] <= w[1]), "{finals:?}");
        for s in &mine {
            assert_eq!(s["meeting_id"], id);
            assert!(s["segment_id"].as_str().unwrap().starts_with(source));
            assert!(s["end_ms"].as_u64().unwrap() >= s["start_ms"].as_u64().unwrap());
            assert!(s["text"].as_str().is_some() && s["words"].is_array());
        }
    }
    // job.progress climbs per chunk under the meeting's job id with the
    // transcribing stage.
    let progress: Vec<&Value> = events
        .iter()
        .filter(|p| p["topic"] == "job.progress")
        .map(|p| &p["payload"])
        .collect();
    assert!(
        progress
            .iter()
            .any(|p| p["stage"] == "transcribing" && p["job_id"] == "job_meeting_1"),
        "{progress:?}"
    );
    assert_eq!(progress.last().unwrap()["stage"], "done");
    let transcribing = events
        .iter()
        .find(|p| p["topic"] == "meeting.state" && p["payload"]["state"] == "transcribing")
        .unwrap();
    assert_eq!(transcribing["payload"]["is_finalizing"], true);
    let last = events.last().unwrap();
    assert_eq!(last["payload"]["kind"], "meeting");
    assert_eq!(last["payload"]["previous_state"], "transcribing");
    assert_eq!(last["payload"]["microphone_takes"], 1);
    assert!(last["payload"]["live_segment_count"].as_u64().unwrap() >= 1);

    // The transcript on the row: both sources on one clock (fn-28 R3, R4).
    let got = daemon.result("meetings.get", json!({"meeting_id": id}));
    assert_eq!(got["status"], "completed");
    assert_eq!(got["title"], "Weekly sync");
    assert!(got["duration_seconds"].as_u64().unwrap() >= 1);
    let transcript = got["transcript"].as_str().unwrap().to_lowercase();
    assert!(
        transcript.contains("country") || transcript.contains("americans"),
        "{transcript}"
    );
    let sources: std::collections::BTreeSet<&str> = got["segments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["source_type"].as_str().unwrap())
        .collect();
    assert_eq!(
        sources.into_iter().collect::<Vec<_>>(),
        ["microphone", "system"]
    );
    let starts: Vec<u64> = got["segments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["start_ms"].as_u64().unwrap())
        .collect();
    assert!(starts.windows(2).all(|w| w[0] <= w[1]), "{starts:?}");
    let hits = daemon.result("meetings.search", json!({"query": "country", "limit": 10}));
    assert!(
        hits["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["ref"]["id"] == id),
        "meetings.search finds the transcript: {hits}"
    );
    let both = daemon.result(
        "transcripts.search",
        json!({"query": "country", "kinds": ["meeting"], "limit": 10}),
    );
    assert!(
        both["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["ref"]["id"] == id)
    );
    for file in [
        "microphone.wav",
        "system.wav",
        "takes.json",
        "system-takes.json",
        "journal.jsonl",
        "metadata.json",
    ] {
        assert!(meeting_dir.join(file).is_file(), "{file}");
    }
    assert!(!meeting_dir.join("live-checkpoint.json").exists());
    let mic_wav = hound::WavReader::open(meeting_dir.join("microphone.wav")).unwrap();
    let sys_wav = hound::WavReader::open(meeting_dir.join("system.wav")).unwrap();
    assert!(mic_wav.len() >= 16_000 && sys_wav.len() >= 16_000);
    assert!(
        mic_wav.len().abs_diff(sys_wav.len()) < 8_000,
        "aligned within half a second"
    );
    let journal = std::fs::read_to_string(meeting_dir.join("journal.jsonl")).unwrap();
    assert!(journal.contains("\"event\":\"start\""));
    assert!(journal.contains("\"event\":\"stop\""));
    assert!(journal.contains("\"event\":\"finalized\""));
    let metadata: Value =
        serde_json::from_str(&std::fs::read_to_string(meeting_dir.join("metadata.json")).unwrap())
            .unwrap();
    assert_eq!(metadata["meeting_id"], id);
    assert!(metadata.get("meeting").is_none());
    assert_eq!(metadata["takes"].as_array().unwrap().len(), 2);

    // The timeline lists the meeting beside the dictations, and the
    // transcripts projection reads it.
    let timeline = daemon.result(
        "transcripts.list",
        json!({"kinds": ["dictation", "meeting"], "limit": 3, "cursor": null}),
    );
    assert_eq!(timeline["items"][0]["ref"]["id"], id);
    assert_eq!(timeline["items"][0]["ref"]["kind"], "meeting");
    assert_eq!(timeline["items"][0]["status"], "completed");
    let projected = daemon.result(
        "transcripts.get",
        json!({"ref": {"kind": "meeting", "id": id}}),
    );
    assert_eq!(projected["text_polish"], got["transcript"]);
    assert_eq!(projected["segments"], got["segments"]);
    let latest = daemon.result("transcripts.latest", json!({"kind": "any"}));
    assert_eq!(latest["ref"]["id"], id);
    let latest = daemon.result("transcripts.latest", json!({"kind": "dictation"}));
    assert_eq!(latest["ref"]["kind"], "dictation");

    // meetings/cancel.json on a fresh meeting: cancelled, the takes gone.
    let second = daemon.result("meetings.start", start_params("Cancelled one"));
    let second_id = second["ref"]["id"].as_str().unwrap().to_string();
    std::thread::sleep(Duration::from_millis(300));
    let cancelled = daemon.result("meetings.cancel", json!({"meeting_id": second_id}));
    let mut want = fixture("meetings/cancel.json")["response"]["result"].clone();
    want["ref"]["id"] = json!(second_id);
    want["job"]["job_id"] = json!("job_meeting_2");
    assert_eq!(cancelled, want);
    assert!(
        !daemon
            .tree
            .root()
            .join("data/dettivo/meetings")
            .join(&second_id)
            .exists()
    );
    let row = daemon.result("meetings.get", json!({"meeting_id": second_id}));
    assert_eq!(row["status"], "cancelled");
    let again = daemon.result("meetings.cancel", json!({"meeting_id": second_id}));
    assert_eq!(again["job"]["state"], "cancelled");
    let not_active = daemon.request("meetings.cancel", json!({"meeting_id": id}));
    assert_eq!(
        not_active["error"]["data"]["details"]["kind"],
        "meetingNotActive"
    );

    // meetings/delete.json and the discard refusal on the seeded sample.
    let refused = daemon.request("meetings.discard", json!({"meeting_id": SAMPLE}));
    assert_eq!(
        refused["error"],
        fixture("meetings/discard.error-conflict-not-partial.json")["error"]["error"]
    );
    let deleted = daemon.result(
        "meetings.delete",
        json!({"meeting_id": SAMPLE, "artifact_policy": "all"}),
    );
    assert_eq!(
        deleted,
        fixture("meetings/delete.json")["response"]["result"]
    );
    let gone = daemon.request("meetings.get", json!({"meeting_id": SAMPLE}));
    assert_eq!(
        gone["error"],
        fixture("meetings/get.error-not-found.json")["error"]["error"]
    );
    let kept = daemon.result(
        "meetings.delete",
        json!({"meeting_id": id, "artifact_policy": "transcript_only"}),
    );
    assert_eq!(kept["deleted"], true);
    assert!(
        meeting_dir.join("microphone.wav").is_file(),
        "transcript_only keeps the audio"
    );
    daemon.stop();
}
