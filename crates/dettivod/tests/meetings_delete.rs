//! R4: `meetings.delete` per the contract's `artifact_policy` against a
//! live daemon over the seeded meetings, with a meeting directory laid
//! out the way a capture leaves it. `transcript_only` keeps the row, the
//! notes and the audio and clears the texts, segments, speakers, analysis
//! and the search entry; `transcript_and_audio` removes the takes and
//! their sidecars and keeps the facts and the notes; `all` removes the
//! row and the directory; a meeting whose audio is already gone still
//! deletes; the active meeting is `CONFLICT` (with the local model).

mod common;

use std::path::{Path, PathBuf};

use common::meetings::{env, spawn, start_params, track_fixture, tree};
use common::{Daemon, Tree};
use serde_json::{Value, json};

const RICH: &str = "5eed0000-0000-4000-8000-00000000a001";
const NOTES_ONLY: &str = "5eed0000-0000-4000-8000-00000000a003";

const SEED_ENV: &[(&str, &str)] = &[("DETTIVO_QA_MODE", "1"), ("DETTIVO_E2E_SEED", "1")];

/// The files a capture leaves under the meeting directory.
fn lay_out(tree: &Tree, id: &str) -> PathBuf {
    let dir = tree.root().join("data/dettivo/meetings").join(id);
    std::fs::create_dir_all(&dir).unwrap();
    for name in [
        "microphone.wav",
        "microphone-2.wav",
        "system.wav",
        "takes.json",
        "system-takes.json",
        "journal.jsonl",
        "metadata.json",
        "notes.md",
        "analysis.json",
    ] {
        std::fs::write(dir.join(name), format!("{name}\n")).unwrap();
    }
    let mut legacy = dettivo_storage::seed_meetings::sample();
    legacy.raw_text = "DELETED_PRIVATE_TRANSCRIPT".into();
    legacy.final_text = legacy.raw_text.clone();
    std::fs::write(dir.join("metadata.json"), serde_json::to_vec(&json!({"meeting": legacy, "takes": [], "journal": "journal.jsonl", "checkpoint_schema": 1})).unwrap()).unwrap();
    let mut checkpoint = dettivo_meeting::checkpoint::Checkpoint::new(id, "2026-01-01T00:00:00Z");
    checkpoint.segments = dettivo_storage::seed_meetings::sample().segments;
    checkpoint.segments[0].text = "DELETED_PRIVATE_TRANSCRIPT".into();
    checkpoint.write(&dir).unwrap();
    std::fs::write(
        dir.join(".live-checkpoint.tmp"),
        "DELETED_PRIVATE_TRANSCRIPT",
    )
    .unwrap();
    dir
}

fn names(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn delete(daemon: &Daemon, id: &str, policy: &str) -> Value {
    daemon.request(
        "meetings.delete",
        json!({"meeting_id": id, "artifact_policy": policy}),
    )
}

#[test]
fn transcript_only_keeps_the_row_the_notes_and_the_audio() {
    let tree = Tree::new();
    let dir = lay_out(&tree, RICH);
    let daemon = Daemon::spawn(tree, SEED_ENV);
    let before = daemon.result("meetings.get", json!({"meeting_id": RICH}));
    assert_eq!(before["analysis_status"], "ready");
    assert_eq!(before["segments"].as_array().unwrap().len(), 5);
    let found = daemon.result("meetings.search", json!({"query": "runbook", "limit": 10}));
    assert_eq!(found["items"].as_array().unwrap().len(), 1);

    let deleted = delete(&daemon, RICH, "transcript_only");
    assert_eq!(deleted["result"]["deleted"], true, "{deleted}");
    let after = daemon.result("meetings.get", json!({"meeting_id": RICH}));
    assert_eq!(after["transcript"], "");
    assert!(after["segments"].as_array().unwrap().is_empty(), "{after}");
    assert_eq!(after["analysis_status"], "none");
    assert!(after["analysis"].is_null());
    assert_eq!(after["notes"], before["notes"], "the notes stay");
    assert_eq!(after["title"], "Roadmap review", "the row stays");
    assert!(
        after["speakers"].is_null() && after["diarization"].is_null(),
        "the speakers and the diarization block go with the segments: {after}"
    );
    let list = daemon.result("meetings.list", json!({"limit": 10, "cursor": null}));
    let row = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["ref"]["id"] == RICH)
        .expect("the row is listed");
    assert!(
        row["speaker_count"].is_null(),
        "a zero speaker count is omitted on the wire: {row}"
    );
    assert_eq!(row["summary"], "");
    assert_eq!(row["has_notes"], true);
    // The search entry is gone for the transcript and the analysis; the
    // notes still match.
    let by_transcript = daemon.result("meetings.search", json!({"query": "gateway", "limit": 10}));
    assert!(by_transcript["items"].as_array().unwrap().is_empty());
    let by_notes = daemon.result("meetings.search", json!({"query": "runbook", "limit": 10}));
    assert_eq!(by_notes["items"][0]["matched_field"], "notes");
    // Every take, the sidecars, the journal and the notes file stay; the
    // analysis file is gone.
    assert_eq!(
        names(&dir),
        [
            "journal.jsonl",
            "live-checkpoint.json",
            "metadata.json",
            "microphone-2.wav",
            "microphone.wav",
            "notes.md",
            "system-takes.json",
            "system.wav",
            "takes.json"
        ]
    );
    for name in names(&dir) {
        assert!(
            !std::fs::read_to_string(dir.join(&name))
                .unwrap()
                .contains("DELETED_PRIVATE_TRANSCRIPT"),
            "leaked in {name}"
        );
    }
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(dir.join("metadata.json")).unwrap()).unwrap();
    assert!(manifest.get("meeting").is_none());
    assert!(
        dettivo_meeting::checkpoint::Checkpoint::read(&dir)
            .unwrap()
            .segments
            .is_empty()
    );
    // A second transcript_only delete on the cleared meeting still succeeds.
    assert_eq!(
        delete(&daemon, RICH, "transcript_only")["result"]["deleted"],
        true
    );
    daemon.stop();
}

#[test]
fn transcript_and_audio_removes_the_takes_and_sidecars_and_keeps_the_facts_and_notes() {
    let tree = Tree::new();
    let dir = lay_out(&tree, RICH);
    let daemon = Daemon::spawn(tree, SEED_ENV);
    let deleted = delete(&daemon, RICH, "transcript_and_audio");
    assert_eq!(deleted["result"]["deleted"], true, "{deleted}");
    let after = daemon.result("meetings.get", json!({"meeting_id": RICH}));
    assert_eq!(after["title"], "Roadmap review");
    assert_eq!(after["started_at"], "2026-02-02T10:00:00Z");
    assert_eq!(after["duration_seconds"], 2520);
    assert_eq!(after["stt_provider_id"], "whisper");
    assert!(
        after["notes"]
            .as_str()
            .unwrap()
            .contains("Budget for Q2 agreed")
    );
    assert_eq!(after["transcript"], "");
    assert_eq!(after["analysis_status"], "none");
    let status = daemon.result("meetings.status", json!({"meeting_id": RICH}));
    assert_eq!(status["capture"]["microphone_takes"], 0);
    assert_eq!(status["capture"]["system_audio"], false);
    assert_eq!(
        names(&dir),
        [
            "journal.jsonl",
            "live-checkpoint.json",
            "metadata.json",
            "notes.md"
        ]
    );
    daemon.stop();
}

#[test]
fn all_removes_the_row_and_the_directory_and_a_missing_directory_is_fine() {
    let tree = Tree::new();
    let dir = lay_out(&tree, RICH);
    let daemon = Daemon::spawn(tree, SEED_ENV);
    assert_eq!(delete(&daemon, RICH, "all")["result"]["deleted"], true);
    assert!(!dir.exists(), "the directory is gone");
    let gone = daemon.request("meetings.get", json!({"meeting_id": RICH}));
    assert_eq!(gone["error"]["data"]["app_code"], "NOT_FOUND");
    assert!(
        daemon.result("meetings.search", json!({"query": "runbook", "limit": 10}))["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    // A meeting whose audio was never kept deletes under every policy.
    assert_eq!(
        delete(&daemon, NOTES_ONLY, "transcript_only")["result"]["deleted"],
        true
    );
    assert_eq!(
        delete(&daemon, NOTES_ONLY, "transcript_and_audio")["result"]["deleted"],
        true
    );
    assert_eq!(
        delete(&daemon, NOTES_ONLY, "all")["result"]["deleted"],
        true
    );
    let unknown = delete(&daemon, NOTES_ONLY, "all");
    assert_eq!(unknown["error"]["data"]["app_code"], "NOT_FOUND");
    let bad = delete(&daemon, RICH, "everything");
    assert_eq!(bad["error"]["data"]["app_code"], "INVALID_PARAMS");
    daemon.stop();
}

/// meetings/F11 (fn-43): a directory the daemon cannot clean is an
/// error that keeps the facts and the row as they are, and a retry once
/// it can finishes the delete.
#[test]
fn a_directory_that_cannot_be_cleaned_keeps_the_facts_and_the_row_until_a_retry() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    if std::fs::metadata("/proc/self")
        .map(|m| m.uid())
        .unwrap_or(1)
        == 0
    {
        eprintln!("skipped: root reads and removes every directory");
        return;
    }
    let tree = Tree::new();
    let dir = lay_out(&tree, RICH);
    let daemon = Daemon::spawn(tree, SEED_ENV);
    let takes = daemon.result("meetings.status", json!({"meeting_id": RICH}))["capture"]
        ["microphone_takes"]
        .clone();
    assert_ne!(takes, 0);
    // Writable but not readable: the analysis file goes, the takes cannot
    // be listed.
    std::fs::set_permissions(&dir, PermissionsExt::from_mode(0o300)).unwrap();
    let refused = delete(&daemon, RICH, "transcript_and_audio");
    std::fs::set_permissions(&dir, PermissionsExt::from_mode(0o755)).unwrap();
    assert!(refused.get("error").is_some(), "{refused}");
    assert!(dir.join("microphone.wav").is_file(), "the takes stay");
    let status = daemon.result("meetings.status", json!({"meeting_id": RICH}));
    assert_eq!(
        status["capture"]["microphone_takes"], takes,
        "the audio facts stay true while the takes stay: {status}"
    );
    assert_eq!(
        delete(&daemon, RICH, "transcript_and_audio")["result"]["deleted"],
        true
    );
    assert_eq!(
        names(&dir),
        [
            "journal.jsonl",
            "live-checkpoint.json",
            "metadata.json",
            "notes.md"
        ]
    );
    // Readable but not writable: the directory cannot be removed.
    std::fs::set_permissions(&dir, PermissionsExt::from_mode(0o500)).unwrap();
    let refused = delete(&daemon, RICH, "all");
    std::fs::set_permissions(&dir, PermissionsExt::from_mode(0o755)).unwrap();
    assert!(refused.get("error").is_some(), "{refused}");
    assert_eq!(
        daemon.result("meetings.get", json!({"meeting_id": RICH}))["title"],
        "Roadmap review",
        "the row stays addressable"
    );
    assert_eq!(delete(&daemon, RICH, "all")["result"]["deleted"], true);
    assert!(!dir.exists(), "the retry removes the directory");
    daemon.stop();
}

/// agent-surfaces/F1 (fn-43): a delete that names no policy takes
/// `[meetings] delete_artifact_policy` from the daemon's configuration,
/// so no client decides a policy of its own or falls back to `all`.
#[test]
fn an_omitted_policy_is_the_configured_one() {
    let tree = Tree::new();
    tree.write_config("[meetings]\ndelete_artifact_policy = \"transcript_only\"\n");
    let dir = lay_out(&tree, RICH);
    let daemon = Daemon::spawn(tree, SEED_ENV);
    let deleted = daemon.request("meetings.delete", json!({"meeting_id": RICH}));
    assert_eq!(deleted["result"]["deleted"], true, "{deleted}");
    let after = daemon.result("meetings.get", json!({"meeting_id": RICH}));
    assert_eq!(after["title"], "Roadmap review", "the row stays");
    assert_eq!(after["transcript"], "");
    assert!(after["notes"].as_str().unwrap().contains("Budget"));
    assert!(dir.join("microphone.wav").is_file(), "the audio stays");
    assert_eq!(
        delete(&daemon, RICH, "all")["result"]["deleted"],
        true,
        "a named policy still wins"
    );
    assert!(!dir.exists());
    daemon.stop();
}

#[test]
fn the_active_meeting_is_a_conflict() {
    let Some(tree) = tree("") else { return };
    let mic = track_fixture("mic");
    let daemon = spawn(tree, &env(&mic, &mic, &[]));
    let started = daemon.result("meetings.start", start_params("Live"));
    let id = started["ref"]["id"].as_str().unwrap().to_string();
    let refused = delete(&daemon, &id, "all");
    assert_eq!(
        refused["error"]["data"]["app_code"], "CONFLICT",
        "{refused}"
    );
    assert_eq!(refused["error"]["data"]["details"]["kind"], "sessionActive");
    daemon.result("meetings.cancel", json!({"meeting_id": id}));
    daemon.stop();
}
