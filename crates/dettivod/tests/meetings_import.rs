//! fn-24 R5: the meeting kind of `transcripts.import` runs the chunked
//! pipeline into a meeting row (the stateful `transcripts/import.json`),
//! `transcripts.export` renders a meeting in the contract's formats over
//! a download transfer (the stateful `transcripts/export.json`), and the
//! MCP-facing `meetings.search` and `transcripts.search` answer meeting
//! hits. Needs the local tiny.en model and jfk.wav for the import.

mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use base64::Engine as _;
use common::{Daemon, Tree, local_model, tree_with_tiny};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const SAMPLE: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";

fn fixture(rel: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dettivo-proto/fixtures")
        .join(rel);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn seeded(tree: Tree) -> Daemon {
    Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_E2E_SEED", "1"),
            ("DETTIVO_MOCK_INSERT", "1"),
        ],
    )
}

fn upload(daemon: &Daemon, content_type: &str, bytes: &[u8]) -> String {
    let begun = daemon.result(
        "transfer.begin",
        json!({"direction": "upload", "content_type": content_type, "size_hint": bytes.len()}),
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

fn pull_all(daemon: &Daemon, transfer_id: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut seq = 1;
    loop {
        let chunk = daemon.result(
            "transfer.pull",
            json!({"transfer_id": transfer_id, "seq": seq}),
        );
        out.extend(
            base64::engine::general_purpose::STANDARD
                .decode(chunk["data_b64"].as_str().unwrap())
                .unwrap(),
        );
        if chunk["eof"] == true {
            return out;
        }
        seq += 1;
    }
}

fn wait_meeting(daemon: &Daemon, id: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        let row = daemon.result("meetings.get", json!({"meeting_id": id}));
        if row["status"] != "transcribing" {
            return row;
        }
        assert!(Instant::now() < deadline, "{id} still transcribing");
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[test]
fn a_meeting_export_renders_the_seed_in_every_format_over_a_download() {
    let daemon = seeded(Tree::new());
    let begun = daemon.result(
        "transfer.begin",
        json!({"direction": "download", "content_type": "application/octet-stream", "size_hint": 0}),
    );
    let transfer = begun["transfer_id"].as_str().unwrap().to_string();
    let exported = daemon.result(
        "transcripts.export",
        json!({"ref": {"kind": "meeting", "id": SAMPLE}, "format": "json", "transfer_id": transfer}),
    );
    let mut want = fixture("transcripts/export.json")["response"]["result"].clone();
    want["transfer_id"] = json!(transfer);
    assert_eq!(exported, want);
    let bytes = pull_all(&daemon, &transfer);
    let doc: Value = serde_json::from_slice(&bytes).unwrap();
    // An unknown format is refused by the catalog naming the spellings;
    // a dictation-only one on a meeting is refused listing the five.
    let bad = daemon.request(
        "transcripts.export",
        json!({"ref": {"kind": "meeting", "id": SAMPLE}, "format": "docx", "transfer_id": "xfer_none"}),
    );
    assert_eq!(bad["error"]["data"]["app_code"], "INVALID_PARAMS", "{bad}");
    assert!(
        bad["error"]["message"].as_str().unwrap().contains("`srt`"),
        "{bad}"
    );
    let zip = daemon.request(
        "transcripts.export",
        json!({"ref": {"kind": "meeting", "id": SAMPLE}, "format": "zip", "transfer_id": "xfer_none"}),
    );
    assert_eq!(zip["error"]["data"]["app_code"], "INVALID_PARAMS", "{zip}");
    assert!(
        zip["error"]["message"]
            .as_str()
            .unwrap()
            .contains("txt, md, json, srt, vtt"),
        "{zip}"
    );
    // The macOS keys (FR-G8, ADR 0036).
    assert_eq!(doc["title"], "Weekly sync");
    assert_eq!(doc["segments"][0]["text"], "Hello.");
    assert_eq!(doc["stt_provider_id"], "whisper");
    assert_eq!(doc["duration_ms"], 1_800_000);
    assert!(doc["analysis"].is_null());
    for (format, filename, head) in [
        (
            "srt",
            "meeting.srt",
            "1\n00:00:00,000 --> 00:00:01,200\nYou: Hello.",
        ),
        (
            "vtt",
            "meeting.vtt",
            "WEBVTT\n\n00:00:00.000 --> 00:00:01.200",
        ),
        ("md", "meeting.md", "# Weekly sync\n"),
        ("txt", "meeting.txt", "2026-02-13T16:00:00Z  Weekly sync\n"),
    ] {
        let begun = daemon.result(
        "transfer.begin",
        json!({"direction": "download", "content_type": "application/octet-stream", "size_hint": 0}),
    );
        let transfer = begun["transfer_id"].as_str().unwrap().to_string();
        let exported = daemon.result(
            "transcripts.export",
            json!({"ref": {"kind": "meeting", "id": SAMPLE}, "format": format, "transfer_id": transfer}),
        );
        assert_eq!(exported["filename"], filename);
        let text = String::from_utf8(pull_all(&daemon, &transfer)).unwrap();
        assert!(text.starts_with(head), "{format}: {text}");
    }
    let begun = daemon.result(
        "transfer.begin",
        json!({"direction": "download", "content_type": "application/octet-stream", "size_hint": 0}),
    );
    let transfer = begun["transfer_id"].as_str().unwrap().to_string();
    let refused = daemon.request(
        "transcripts.export",
        json!({"ref": {"kind": "meeting", "id": SAMPLE}, "format": "zip", "transfer_id": transfer}),
    );
    assert_eq!(refused["error"]["data"]["app_code"], "INVALID_PARAMS");

    // The search projections over the seeded meeting.
    let hits = daemon.result("meetings.search", json!({"query": "weekly", "limit": 10}));
    assert_eq!(hits["items"][0]["ref"]["id"], SAMPLE);
    assert!(hits["items"][0]["score"].is_null());
    let both = daemon.result(
        "transcripts.search",
        json!({"query": "weekly", "kinds": ["dictation", "meeting"], "limit": 10}),
    );
    assert!(
        both["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["ref"]["kind"] == "meeting")
    );
    let rerun = daemon.request(
        "transcripts.rerun",
        json!({"ref": {"kind": "meeting", "id": SAMPLE}}),
    );
    assert_eq!(rerun["error"]["data"]["app_code"], "NOT_IMPLEMENTED");
    let deleted = daemon.result(
        "transcripts.delete",
        json!({"ref": {"kind": "meeting", "id": SAMPLE}}),
    );
    assert_eq!(deleted["deleted"], true);
    assert_eq!(deleted["ref"]["kind"], "meeting");
    daemon.stop();
}

#[test]
fn an_audio_import_into_the_meeting_kind_transcribes_into_a_meeting_row() {
    let Some((model, wav)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing");
        return;
    };
    let daemon = seeded(tree_with_tiny(&model, ""));
    let bytes = std::fs::read(&wav).unwrap();
    let transfer = upload(&daemon, "audio/wav", &bytes);
    // The disclosure gate applies to a meeting import too (FR-G9).
    let refused = daemon.request(
        "transcripts.import",
        json!({"transfer_id": transfer, "target_kind": "meeting", "filename": "call.m4a", "language": "en", "mode": "raw"}),
    );
    assert_eq!(
        refused["error"]["data"]["details"]["kind"],
        "meetingDisclosureRequired"
    );
    let imported = daemon.result(
        "transcripts.import",
        json!({"transfer_id": transfer, "target_kind": "meeting", "filename": "call.m4a", "language": "en", "mode": "raw", "acknowledge_meeting_disclosure": true}),
    );
    let mut want = fixture("transcripts/import.json")["response"]["result"].clone();
    let id = imported["ref"]["id"].as_str().unwrap().to_string();
    want["ref"]["id"] = json!(id);
    assert_eq!(imported, want);
    let row = wait_meeting(&daemon, &id);
    assert_eq!(row["status"], "completed", "{row}");
    assert_eq!(row["title"], "call");
    assert!(
        row["transcript"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("americans")
    );
    assert!(!row["segments"].as_array().unwrap().is_empty());
    assert_eq!(row["segments"][0]["source_type"], "microphone");
    let dir = daemon.tree.root().join("data/dettivo/meetings").join(&id);
    assert!(dir.join("microphone.wav").is_file());
    // `completed` can land a moment before the finaliser writes the
    // metadata beside the audio on a slow runner.
    assert!(common::wait_for(Duration::from_secs(10), || dir
        .join("metadata.json")
        .is_file()));
    let listed = daemon.result(
        "transcripts.list",
        json!({"kinds": ["meeting"], "limit": 5, "cursor": null}),
    );
    assert_eq!(listed["items"][0]["ref"]["id"], id);
    assert_eq!(listed["items"][0]["status"], "completed");
    let projected = daemon.result(
        "transcripts.get",
        json!({"ref": {"kind": "meeting", "id": id}}),
    );
    assert!(
        projected["text_polish"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("americans")
    );
    daemon.stop();
}
