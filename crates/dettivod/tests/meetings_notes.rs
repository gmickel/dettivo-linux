//! R1, R2 and R5 against a live daemon over the seed (ADR 0036): notes
//! land on the row and in `notes.md` and the search names the notes
//! column; the analysis runs from the `DETTIVO_MOCK_LLM` fixture (map
//! then reduce over a transcript longer than `chunk_chars`), lands on the
//! row and in `analysis.json`, never touches the notes, keeps the old
//! analysis through a failed regenerate and names why it failed; without
//! a provider the answer carries the notice; a finalised meeting is
//! polished and analysed on its own (with the local model); the
//! `meetings.*` notes and analysis fixtures replay by name; and
//! `DETTIVO_E2E_DISCLOSURE` seeds or clears the acknowledgement.

mod common;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use common::meetings::{Subscriber, env, fixture, spawn, start_params, track_fixture, tree};
use common::{Daemon, Tree};
use serde_json::{Value, json};

const RICH: &str = "5eed0000-0000-4000-8000-00000000a001";
const PARTIAL: &str = "5eed0000-0000-4000-8000-00000000a002";
const NOTES_ONLY: &str = "5eed0000-0000-4000-8000-00000000a003";

const ANSWER: &str = r#"{"summary": "The roadmap review agreed the Q2 budget and moved the rollout to Thursday.", "decisions": ["Hire two engineers"], "action_items": [{"text": "Write the runbook", "owner": "Gordon", "due": "Wednesday"}]}"#;

/// A fixture directory whose `default.txt` answers every call.
fn llm_fixture(answer: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix("dtvllm")
        .tempdir_in("/tmp")
        .unwrap();
    std::fs::write(dir.path().join("default.txt"), answer).unwrap();
    let path = dir.path().to_path_buf();
    (dir, path)
}

fn meeting_dir(daemon: &Daemon, id: &str) -> PathBuf {
    daemon.tree.root().join("data/dettivo/meetings").join(id)
}

fn wait_analysis(daemon: &Daemon, id: &str, wanted: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let got = daemon.result("meetings.analysis.get", json!({"meeting_id": id}));
        if got["analysis_status"] == wanted {
            return got;
        }
        assert!(
            Instant::now() < deadline,
            "{id} never reached {wanted}: {got}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn notes_land_on_the_row_and_in_notes_md_and_the_search_names_the_column() {
    let tree = Tree::new();
    tree.write_config("[ipc]\nmax_line_bytes = 4194304\n");
    let daemon = Daemon::spawn(tree, &[("DETTIVO_QA_MODE", "1"), ("DETTIVO_E2E_SEED", "1")]);
    let got = daemon.result("meetings.notes.get", json!({"meeting_id": RICH}));
    assert_eq!(
        got,
        fixture("meetings/notes.get.json")["response"]["result"]
    );

    let set = daemon.result(
        "meetings.notes.set",
        json!({"meeting_id": NOTES_ONLY, "markdown": "# Sprint\n\nWatch the Vulkan latency budget.", "source": "live"}),
    );
    assert_eq!(set["source"], "live");
    assert!(set["updated_at"].as_str().unwrap().ends_with('Z'));
    let file = meeting_dir(&daemon, NOTES_ONLY).join("notes.md");
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "# Sprint\n\nWatch the Vulkan latency budget.\n"
    );
    let row = daemon.result("meetings.get", json!({"meeting_id": NOTES_ONLY}));
    assert_eq!(row["notes"], "# Sprint\n\nWatch the Vulkan latency budget.");
    assert_eq!(row["notes_source"], "live");
    let both = daemon.result(
        "transcripts.get",
        json!({"ref": {"kind": "meeting", "id": NOTES_ONLY}}),
    );
    assert_eq!(both["meeting"]["notes_source"], "live");
    let hit = daemon.result("meetings.search", json!({"query": "sprint", "limit": 10}));
    assert_eq!(hit["items"][0]["ref"]["id"], NOTES_ONLY);
    assert_eq!(hit["items"][0]["matched_field"], "notes");
    assert!(
        hit["items"][0]["snippet"]
            .as_str()
            .unwrap()
            .contains("Sprint")
    );
    let list = daemon.result("meetings.list", json!({"limit": 10, "cursor": null}));
    assert_eq!(list, fixture("meetings/list.json")["response"]["result"]);
    // The fixture's search over the seed: the notes column of the rich row.
    let search = fixture("meetings/search.json");
    assert_eq!(
        daemon.result("meetings.search", search["request"]["params"].clone()),
        search["response"]["result"]
    );
    // The notes.set fixture, apart from the clock.
    let set = fixture("meetings/notes.set.json");
    let mut back = daemon.result("meetings.notes.set", set["request"]["params"].clone());
    assert!(back["updated_at"].as_str().unwrap().ends_with('Z'));
    back["updated_at"] = set["response"]["result"]["updated_at"].clone();
    assert_eq!(back, set["response"]["result"]);
    // Notes on a partial (recording-then-killed) meeting are allowed.
    let partial = daemon.result(
        "meetings.notes.set",
        json!({"meeting_id": PARTIAL, "markdown": "while it ran"}),
    );
    assert_eq!(partial["source"], "user");
    // Empty notes remove the file.
    daemon.result(
        "meetings.notes.set",
        json!({"meeting_id": NOTES_ONLY, "markdown": ""}),
    );
    assert!(!file.exists());
    // Over-size notes are refused naming the limit.
    let big = "x".repeat(1024 * 1024 + 1);
    let refused = daemon.request(
        "meetings.notes.set",
        json!({"meeting_id": RICH, "markdown": big}),
    );
    assert_eq!(refused["error"]["data"]["app_code"], "INVALID_PARAMS");
    let message = refused["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("1048576") && message.contains("1 MiB"),
        "{message}"
    );
    let missing = daemon.request(
        "meetings.notes.get",
        json!({"meeting_id": "00000000-0000-4000-8000-000000000000"}),
    );
    assert_eq!(missing["error"]["data"]["app_code"], "NOT_FOUND");
    daemon.stop();
}

#[test]
fn the_analysis_runs_map_then_reduce_from_the_fixture_and_keeps_the_old_one_on_failure() {
    let (_keep, dir) = llm_fixture(ANSWER);
    let tree = Tree::new();
    tree.write_config("[meetings.analysis]\nchunk_chars = 200\ntimeout_ms = 10000\n");
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_E2E_SEED", "1"),
            ("DETTIVO_MOCK_LLM", &format!("fixture:{}", dir.display())),
            ("RUST_LOG", "info"),
        ],
    );
    let notes_before = daemon.result("meetings.notes.get", json!({"meeting_id": RICH}));
    let mut sub = Subscriber::open(&daemon, &["meeting.state"]);
    // The seeded analysis is answered without a run until force is asked.
    let ready = daemon.result("meetings.analyze", json!({"meeting_id": RICH}));
    assert_eq!(ready["job"]["state"], "succeeded");
    assert_eq!(ready["analysis_status"], "ready");
    let started = daemon.result(
        "meetings.analyze",
        json!({"meeting_id": RICH, "force": true}),
    );
    assert_eq!(
        started,
        fixture("meetings/analyze.json")["response"]["result"],
        "{started}"
    );
    let events = sub.collect(
        |p| p["payload"]["analysis_status"] == "ready",
        Duration::from_secs(20),
    );
    assert!(
        events
            .iter()
            .any(|p| p["payload"]["analysis_status"] == "running"),
        "{events:?}"
    );
    let done = wait_analysis(&daemon, RICH, "ready");
    assert_eq!(
        done["analysis"]["summary"],
        "The roadmap review agreed the Q2 budget and moved the rollout to Thursday."
    );
    assert_eq!(done["analysis"]["action_items"][0]["owner"], "Gordon");
    assert!(done["model"].as_str().unwrap().starts_with("mock-fixture:"));
    assert!(done["error"].is_null());
    let file = meeting_dir(&daemon, RICH).join("analysis.json");
    let stored: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(stored["decisions"][0], "Hire two engineers");
    assert_eq!(
        daemon.result("meetings.notes.get", json!({"meeting_id": RICH})),
        notes_before,
        "the analysis never changes the notes"
    );
    let list = daemon.result("meetings.list", json!({"limit": 10, "cursor": null}));
    let row = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["ref"]["id"] == RICH)
        .unwrap();
    assert!(
        row["summary"]
            .as_str()
            .unwrap()
            .starts_with("The roadmap review")
    );
    let by_analysis = daemon.result("meetings.search", json!({"query": "rollout", "limit": 10}));
    assert!(!by_analysis["items"].as_array().unwrap().is_empty());
    assert!(
        daemon.log().contains("parts=2"),
        "a transcript over chunk_chars runs map then reduce: {}",
        daemon.log()
    );
    // A meeting without a final transcript is refused (the fixture).
    let refused = daemon.request("meetings.analyze", json!({"meeting_id": PARTIAL}));
    assert_eq!(
        refused["error"],
        fixture("meetings/analyze.error-conflict-not-completed.json")["error"]["error"]
    );
    // A timeout is failed with provider_unavailable and the old analysis stays.
    std::fs::write(dir.join("default.txt"), "__TIMEOUT__").unwrap();
    daemon.result(
        "meetings.analyze",
        json!({"meeting_id": RICH, "force": true}),
    );
    let failed = wait_analysis(&daemon, RICH, "failed");
    assert!(
        failed["error"]
            .as_str()
            .unwrap()
            .starts_with("provider_unavailable"),
        "{failed}"
    );
    assert_eq!(failed["analysis"]["decisions"][0], "Hire two engineers");
    // A degenerate answer is asked for once more, then fails naming why.
    std::fs::write(dir.join("default.txt"), "Sure! What would you like?").unwrap();
    daemon.result(
        "meetings.analyze",
        json!({"meeting_id": RICH, "force": true}),
    );
    let degenerate = wait_analysis(&daemon, RICH, "failed");
    let error = degenerate["error"].as_str().unwrap();
    assert!(
        error.starts_with("invalid_output") && error.contains("after the repair pass"),
        "{error}"
    );
    // The provider answers again: the next run succeeds.
    std::fs::write(dir.join("default.txt"), ANSWER).unwrap();
    daemon.result(
        "meetings.analyze",
        json!({"meeting_id": RICH, "force": true}),
    );
    wait_analysis(&daemon, RICH, "ready");
    // An empty transcript never reaches the model.
    daemon.result(
        "meetings.delete",
        json!({"meeting_id": NOTES_ONLY, "artifact_policy": "transcript_only"}),
    );
    let empty = daemon.result(
        "meetings.analyze",
        json!({"meeting_id": NOTES_ONLY, "force": true}),
    );
    assert_eq!(empty["job"]["state"], "running");
    let empty = wait_analysis(&daemon, NOTES_ONLY, "failed");
    assert!(
        empty["error"]
            .as_str()
            .unwrap()
            .starts_with("emptyTranscript"),
        "{empty}"
    );
    let log = daemon.stop();
    assert!(
        !log.contains("Hire two engineers"),
        "the log never carries the analysis"
    );
}

#[test]
fn without_a_provider_the_answer_carries_the_notice_and_the_row_fails() {
    let tree = Tree::new();
    tree.write_config("[llm]\nollama_url = \"http://127.0.0.1:1\"\n");
    let daemon = Daemon::spawn(tree, &[("DETTIVO_QA_MODE", "1"), ("DETTIVO_E2E_SEED", "1")]);
    let answer = daemon.result(
        "meetings.analyze",
        json!({"meeting_id": RICH, "force": true}),
    );
    assert_eq!(answer["job"]["state"], "failed", "{answer}");
    assert_eq!(answer["analysis_status"], "failed");
    assert_eq!(answer["notice"]["kind"], "provider_unavailable");
    let got = daemon.result("meetings.analysis.get", json!({"meeting_id": RICH}));
    assert_eq!(got["analysis_status"], "failed");
    assert!(
        got["error"]
            .as_str()
            .unwrap()
            .starts_with("provider_unavailable")
    );
    assert_eq!(
        got["analysis"]["summary"],
        fixture("meetings/analysis.get.json")["response"]["result"]["analysis"]["summary"],
        "the seeded analysis stays"
    );
    daemon.stop();
}

#[test]
fn a_finalised_meeting_is_polished_and_analysed_on_its_own() {
    let Some(tree) = tree("[meetings.analysis]\ntimeout_ms = 10000\n") else {
        return;
    };
    let (_keep, dir) = llm_fixture(ANSWER);
    let mic = track_fixture("mic");
    let daemon = spawn(
        tree,
        &env(
            &mic,
            &mic,
            &[("DETTIVO_MOCK_LLM", &format!("fixture:{}", dir.display()))],
        ),
    );
    let mut sub = Subscriber::open(&daemon, &["meeting.state"]);
    let started = daemon.result("meetings.start", start_params("Polished"));
    let id = started["ref"]["id"].as_str().unwrap().to_string();
    std::thread::sleep(Duration::from_millis(1500));
    daemon.result("meetings.stop", json!({"meeting_id": id}));
    let deadline = Instant::now() + Duration::from_secs(120);
    let row = loop {
        let row = daemon.result("meetings.get", json!({"meeting_id": id}));
        if row["status"] == "completed" {
            break row;
        }
        assert!(Instant::now() < deadline, "{row}");
        std::thread::sleep(Duration::from_millis(200));
    };
    // The `completed` transition names both passes the finalisation
    // planned; the analysis reports `running` only after it, and the
    // speaker pass settles `unavailable` (no model set in this tree)
    // instead of staying queued.
    let events = sub.collect(
        |p| p["payload"]["analysis_status"] == "ready",
        Duration::from_secs(60),
    );
    let planned = events
        .iter()
        .position(|p| {
            p["payload"]["state"] == "completed"
                && p["payload"]["analysis_status"] == "queued"
                && p["payload"]["diarization_status"] == "queued"
        })
        .unwrap_or_else(|| panic!("no planned transition: {events:?}\n{}", daemon.log()));
    let running = events
        .iter()
        .position(|p| p["payload"]["analysis_status"] == "running")
        .unwrap_or_else(|| panic!("no running transition: {events:?}\n{}", daemon.log()));
    assert!(planned < running, "{events:?}");
    assert!(
        events
            .iter()
            .any(|p| p["payload"]["diarization_status"] == "unavailable"),
        "{events:?}"
    );
    let segments = row["segments"].as_array().unwrap();
    assert!(!segments.is_empty(), "{row}");
    for s in segments {
        let polished = s["polished_text"].as_str().expect("polished text");
        assert!(!polished.is_empty());
    }
    let both = daemon.result(
        "transcripts.get",
        json!({"ref": {"kind": "meeting", "id": id}}),
    );
    assert_eq!(both["text_polish"], row["transcript"]);
    assert_ne!(both["text_raw"], "", "the raw words stay");
    let done = wait_analysis(&daemon, &id, "ready");
    assert_eq!(done["analysis"]["decisions"][0], "Hire two engineers");
    assert!(meeting_dir(&daemon, &id).join("analysis.json").is_file());
    // A meeting that asks for no analysis stays `none`.
    let mut quiet = start_params("Quiet");
    quiet["analyze"] = json!(false);
    let started = daemon.result("meetings.start", quiet);
    let id = started["ref"]["id"].as_str().unwrap().to_string();
    std::thread::sleep(Duration::from_millis(1200));
    daemon.result("meetings.stop", json!({"meeting_id": id}));
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let row = daemon.result("meetings.get", json!({"meeting_id": id}));
        if row["status"] == "completed" {
            break;
        }
        assert!(Instant::now() < deadline, "{row}");
        std::thread::sleep(Duration::from_millis(200));
    }
    std::thread::sleep(Duration::from_millis(500));
    let got = daemon.result("meetings.analysis.get", json!({"meeting_id": id}));
    assert_eq!(got["analysis_status"], "none", "{got}");
    daemon.stop();
}

#[test]
fn the_disclosure_variable_seeds_or_clears_the_acknowledgement() {
    let daemon = Daemon::spawn(
        Tree::new(),
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_E2E_DISCLOSURE", "acknowledged"),
        ],
    );
    let got = daemon.result("meetings.disclosure.get", json!({}));
    assert_eq!(got["acknowledged"], true, "{got}");
    assert!(got["acknowledged_at"].as_str().unwrap().ends_with('Z'));
    let (_, tree) = daemon.stop_keep();
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_E2E_DISCLOSURE", "pending"),
        ],
    );
    let got = daemon.result("meetings.disclosure.get", json!({}));
    assert_eq!(
        got,
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
    daemon.stop();
    let bad = Tree::new()
        .command()
        .env("DETTIVO_QA_MODE", "1")
        .env("DETTIVO_E2E_DISCLOSURE", "maybe")
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("DETTIVO_E2E_DISCLOSURE"));
}

#[test]
fn analysis_respects_artifact_policy_without_recreating_a_directory() {
    for policy in ["none", "audio_only"] {
        let tree = Tree::new();
        tree.write_config(&format!("[meetings]\nartifacts = \"{policy}\"\n"));
        let (_fixture, path) = llm_fixture(ANSWER);
        let daemon = Daemon::spawn(
            tree,
            &[
                ("DETTIVO_QA_MODE", "1"),
                ("DETTIVO_E2E_SEED", "1"),
                ("DETTIVO_MOCK_LLM", &format!("fixture:{}", path.display())),
            ],
        );
        daemon.result(
            "meetings.analyze",
            json!({"meeting_id": RICH, "force": true}),
        );
        wait_analysis(&daemon, RICH, "ready");
        assert!(
            !meeting_dir(&daemon, RICH).join("analysis.json").exists(),
            "{policy}"
        );
        daemon.stop();
    }
}
