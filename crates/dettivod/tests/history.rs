//! R3 without a model: `dictation.reinsert_last` and
//! `insert.perform { source_ref }` reading the store, list, search, delete
//! and stats over the seed, and a daemon that refuses to start on a
//! database it cannot read. The transfer and export tests are in
//! `history_export.rs`.

mod common;

use std::path::{Path, PathBuf};

use common::{Daemon, Tree};
use serde_json::{Value, json};

const SAMPLE: &str = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const SEED_01: &str = "5eed0000-0000-4000-8000-000000000001";

fn fixture(rel: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dettivo-proto/fixtures")
        .join(rel);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn seeded() -> Daemon {
    Daemon::spawn(
        Tree::new(),
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_E2E_SEED", "1"),
            ("DETTIVO_MOCK_INSERT", "1"),
        ],
    )
}

#[test]
fn reinsert_last_and_insert_perform_read_the_store_through_the_mock_inserter() {
    let daemon = seeded();
    let inserted = daemon.tree.root().join("state/dettivo/qa/inserted.txt");
    let reinserted = daemon.result("dictation.reinsert_last", json!({}));
    assert_eq!(reinserted["ref"]["id"], SAMPLE);
    assert_eq!(reinserted["insertion"]["outcome"], "inserted");
    assert_eq!(reinserted["insertion"]["method"], "paste");
    assert_eq!(reinserted["insertion"]["backend"]["name"], "mock");
    assert_eq!(
        reinserted["insertion"]["target_app"]["bundle_id"], "org.gnome.TextEditor",
        "the QA focus probe's fixed target"
    );
    let by_ref = daemon.result(
        "insert.perform",
        json!({"mode": "raw", "source_ref": {"kind": "dictation", "id": SEED_01}}),
    );
    assert_eq!(by_ref["outcome"], "inserted");
    let literal = daemon.result(
        "insert.perform",
        json!({"mode": "raw", "text": "Hello team"}),
    );
    assert_eq!(literal["outcome"], "inserted");
    let lines: Vec<String> = std::fs::read_to_string(&inserted)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(
        lines,
        [
            "The contract sample dictation. Thirty seconds of speech.",
            "Morning standup notes, the API gateway rollout moves to Thursday.",
            "Hello team"
        ]
    );
    let guard = daemon.request(
        "insert.perform",
        json!({"mode": "raw", "text": "Hello", "expected_target_pid": "4242"}),
    );
    assert_eq!(
        guard["error"],
        fixture("insert/perform.error-conflict-target-mismatch.json")["error"]["error"]
    );
    let nothing = daemon.request("insert.perform", json!({"mode": "raw"}));
    assert_eq!(nothing["error"]["data"]["app_code"], "INVALID_PARAMS");
    let unknown = daemon.request(
        "insert.perform",
        json!({"mode": "raw", "source_ref": {"kind": "dictation", "id": "00000000-0000-4000-8000-000000000000"}}),
    );
    assert_eq!(unknown["error"]["data"]["app_code"], "NOT_FOUND");
    daemon.stop();

    let empty = Daemon::spawn(
        Tree::new(),
        &[("DETTIVO_QA_MODE", "1"), ("DETTIVO_MOCK_INSERT", "1")],
    );
    let none = empty.request("dictation.reinsert_last", json!({}));
    assert_eq!(none["error"]["data"]["app_code"], "NOT_FOUND");
    empty.stop();
}

#[test]
fn list_pages_search_limits_delete_and_stats_answer_over_the_seed() {
    let daemon = seeded();
    let first = daemon.result(
        "transcripts.list",
        json!({"kinds": ["dictation", "meeting"], "limit": 5, "cursor": null}),
    );
    assert_eq!(first["items"].as_array().unwrap().len(), 5);
    assert_eq!(first["items"][0]["ref"]["id"], SAMPLE);
    assert_eq!(first["items"][0]["status"], "completed");
    assert_eq!(first["items"][0]["duration_seconds"], 30);
    let cursor = first["next_cursor"].as_str().expect("a cursor").to_string();
    let second = daemon.result(
        "transcripts.list",
        json!({"kinds": ["dictation"], "limit": 5, "cursor": cursor}),
    );
    assert_eq!(second["items"].as_array().unwrap().len(), 5);
    assert_ne!(second["items"][0]["ref"]["id"], SAMPLE);
    let meetings_only = daemon.result(
        "transcripts.list",
        json!({"kinds": ["meeting"], "limit": 5, "cursor": null}),
    );
    assert_eq!(
        meetings_only["items"].as_array().unwrap().len(),
        4,
        "the seeded meetings"
    );
    assert_eq!(meetings_only["items"][0]["ref"]["kind"], "meeting");

    let hits = daemon.result(
        "transcripts.search",
        json!({"query": "api", "kinds": ["dictation", "meeting"], "limit": 10}),
    );
    // Two dictations and the seeded roadmap meeting mention the API.
    assert_eq!(hits["items"].as_array().unwrap().len(), 3);
    assert!(hits["items"][0]["score"].is_null());
    let folded = daemon.result(
        "transcripts.search",
        json!({"query": "cafe", "kinds": ["dictation"], "limit": 10}),
    );
    assert_eq!(folded["items"].as_array().unwrap().len(), 1);
    let long = daemon.request(
        "transcripts.search",
        json!({"query": "x".repeat(1025), "kinds": ["dictation"], "limit": 10}),
    );
    assert_eq!(long["error"]["data"]["app_code"], "INVALID_PARAMS");
    assert!(long["error"]["message"].as_str().unwrap().contains("1024"));
    let stops = daemon.request(
        "transcripts.search",
        json!({"query": "... ---", "kinds": ["dictation"], "limit": 10}),
    );
    assert_eq!(stops["error"]["data"]["app_code"], "INVALID_PARAMS");

    let before = daemon.result("transcripts.stats", json!({}));
    assert_eq!(before["item_count"], 12);
    assert_eq!(before["schema_version"], 8);
    assert_eq!(before["last_migration"], "0008-analysis-queued");
    assert!(
        before["path"]
            .as_str()
            .unwrap()
            .ends_with("data/dettivo/dettivo.db")
    );
    let deleted = daemon.result(
        "transcripts.delete",
        json!({"ref": {"kind": "dictation", "id": SEED_01}}),
    );
    assert_eq!(deleted["deleted"], true);
    let gone = daemon.request(
        "transcripts.get",
        json!({"ref": {"kind": "dictation", "id": SEED_01}}),
    );
    assert_eq!(gone["error"]["data"]["app_code"], "NOT_FOUND");
    let after = daemon.result("transcripts.stats", json!({}));
    assert_eq!(after["item_count"], 11);
    let meeting = daemon.request(
        "transcripts.get",
        json!({"ref": {"kind": "meeting", "id": SEED_01}}),
    );
    assert_eq!(meeting["error"]["data"]["app_code"], "NOT_FOUND");
    daemon.stop();
}

/// `transcripts.get` carries the facts the history detail shows, the seed
/// keeps one take for the drive's re-run, and a search hit paints its
/// matches and lists as a row (fn-22, ADR 0025).
#[test]
fn get_carries_the_facts_and_a_search_hit_paints_its_matches() {
    // The sweep removes audio older than `audio_retention_days` and the
    // seed is dated in February, so the take stays only with the limit off.
    let tree = Tree::new();
    tree.write_config("[history]\naudio_retention_days = 0\n");
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_E2E_SEED", "1"),
            ("DETTIVO_MOCK_INSERT", "1"),
        ],
    );
    let sample = daemon.result(
        "transcripts.get",
        json!({"ref": {"kind": "dictation", "id": SAMPLE}}),
    );
    let facts = &sample["facts"];
    assert_eq!(facts["app_id"], "org.gnome.TextEditor");
    assert_eq!(facts["source"], "dictation");
    assert_eq!(facts["status"], "completed");
    assert_eq!(facts["model"], "large-v3-turbo");
    assert_eq!(facts["duration_seconds"], 30);
    assert_eq!(facts["audio"]["retained"], false);
    assert_eq!(facts["audio"]["reason"], "not_retained");
    assert_eq!(facts["insertion"]["backend"]["name"], "mock");
    assert_eq!(facts["timings"]["stop_to_insert_ms"], 920);
    assert!(facts.get("rerun_of").is_none());

    const AUDIO_ITEM: &str = "5eed0000-0000-4000-8000-000000000011";
    let kept = daemon.result(
        "transcripts.get",
        json!({"ref": {"kind": "dictation", "id": AUDIO_ITEM}}),
    );
    assert_eq!(kept["facts"]["audio"]["retained"], true);
    let path = kept["facts"]["audio"]["path"].as_str().unwrap();
    assert!(path.ends_with("microphone.wav"), "{path}");
    assert!(Path::new(path).is_file());

    let listed = daemon.result(
        "transcripts.list",
        json!({"kinds": ["dictation"], "limit": 1, "cursor": null}),
    );
    assert_eq!(listed["items"][0]["app_id"], "org.gnome.TextEditor");
    assert_eq!(listed["items"][0]["mode"], "raw");
    assert_eq!(listed["items"][0]["source"], "dictation");

    let hits = daemon.result(
        "transcripts.search",
        json!({"query": "api", "kinds": ["dictation"], "limit": 10}),
    );
    let first = &hits["items"][0];
    let snippet = first["snippet"].as_str().unwrap();
    let start = first["matches"][0]["start"].as_u64().unwrap() as usize;
    let end = first["matches"][0]["end"].as_u64().unwrap() as usize;
    let painted: String = snippet.chars().skip(start).take(end - start).collect();
    assert_eq!(painted.to_lowercase(), "api", "{snippet}");
    assert_eq!(first["item"]["ref"]["id"], first["ref"]["id"]);
    assert!(
        first["item"]["title"]
            .as_str()
            .is_some_and(|t| !t.is_empty())
    );
    daemon.stop();
}

#[test]
fn a_database_the_daemon_cannot_read_stops_it_with_the_reason() {
    let tree = Tree::new();
    let db = tree.root().join("data/dettivo/dettivo.db");
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();
    std::fs::write(&db, b"this is not a database").unwrap();
    let output = tree.command().output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("history store"), "{stderr}");
    assert!(!tree.socket().exists());
}

/// The seeded tree's item directory for `id`.
#[allow(dead_code)]
fn item_dir(tree: &Tree, id: &str) -> PathBuf {
    tree.root().join("data/dettivo/dictations").join(id)
}
