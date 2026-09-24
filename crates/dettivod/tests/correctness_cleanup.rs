//! Socket regressions for the fn-53 correctness contracts.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use common::{Daemon, Tree};
use dettivo_storage::Store;
use dettivo_storage::item::{DictationItem, SourceKind};
use dettivo_storage::meetings::MeetingRow;
use serde_json::{Value, json};

#[test]
fn explicit_null_id_answers_while_absent_id_remains_a_notification() {
    let daemon = Daemon::spawn(Tree::new(), &[]);
    let mut socket = UnixStream::connect(daemon.tree.socket()).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    for id in [json!("text"), json!(17), Value::Null] {
        writeln!(
            socket,
            "{}",
            json!({"jsonrpc":"2.0", "id":id,
            "method":"system.ping", "params":{}})
        )
        .unwrap();
        writeln!(
            socket,
            "{}",
            json!({"jsonrpc":"2.0", "id":"sentinel",
            "method":"system.ping", "params":{}})
        )
        .unwrap();
        let mut reader = BufReader::new(&mut socket);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["id"], id, "explicit IDs must answer");
        assert!(response.get("result").is_some(), "{response}");
        line.clear();
        reader.read_line(&mut line).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&line).unwrap()["id"],
            "sentinel"
        );
    }
    writeln!(
        socket,
        "{}",
        json!({"jsonrpc":"2.0", "method":"system.ping", "params":{}})
    )
    .unwrap();
    writeln!(
        socket,
        "{}",
        json!({"jsonrpc":"2.0", "id":23, "method":"system.ping", "params":{}})
    )
    .unwrap();
    let mut line = String::new();
    BufReader::new(socket).read_line(&mut line).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&line).unwrap()["id"], 23);
}

#[test]
fn combined_search_orders_exact_titles_before_recency_across_kinds() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/search-ordering.json")).unwrap();
    let tree = Tree::new();
    let store = Store::open(&tree.root().join("data/dettivo/dettivo.db")).unwrap();
    let mut meeting = MeetingRow::new();
    meeting.title = fixture["meeting_title"].as_str().unwrap().into();
    meeting.created_at = fixture["meeting_created_at"].as_str().unwrap().into();
    store.insert_meeting(&meeting).unwrap();
    for i in 0..fixture["dictation_count"].as_u64().unwrap() {
        let mut item = DictationItem::new(SourceKind::Dictation);
        item.title = format!(
            "{} {i}",
            fixture["dictation_title_prefix"].as_str().unwrap()
        );
        item.raw_text = item.title.clone();
        item.final_text = item.title.clone();
        item.created_at = fixture["dictation_created_at"].as_str().unwrap().into();
        store.insert(&item).unwrap();
    }
    drop(store);
    let daemon = Daemon::spawn(tree, &[]);
    for limit in fixture["limits"].as_array().unwrap() {
        let result = daemon.result(
            "transcripts.search",
            json!({
                "query":fixture["query"], "limit":limit, "kinds":["dictation", "meeting"]
            }),
        );
        assert_eq!(
            result["items"].as_array().unwrap().len(),
            limit.as_u64().unwrap() as usize
        );
        assert_eq!(result["items"][0]["ref"]["id"], meeting.id);
        assert_eq!(
            result["items"][0]["ref"]["kind"],
            fixture["expected_first_kind"]
        );
    }
}

#[test]
fn speaker_names_reject_control_characters_before_mutating_the_row() {
    let daemon = Daemon::spawn(
        Tree::new(),
        &[("DETTIVO_QA_MODE", "1"), ("DETTIVO_E2E_SEED", "1")],
    );
    let id = "0f8fad5b-d9cb-469f-a165-70867728950e";
    for name in ["Ada\n\nBob", "Ada\rBob", "Ada\tBob", "Ada\u{1}Bob"] {
        let response = daemon.request(
            "meetings.speakers.rename",
            json!({
                "meeting_id":id, "speaker_id":"you", "name":name
            }),
        );
        assert_eq!(
            response["error"]["data"]["app_code"], "INVALID_PARAMS",
            "{response}"
        );
        assert_eq!(response["error"]["data"]["details"]["field"], "name");
    }
    let result = daemon.result(
        "meetings.speakers.rename",
        json!({
            "meeting_id":id, "speaker_id":"you", "name":"Ada & 李 <developer>"
        }),
    );
    assert_eq!(result["speaker"]["name"], "Ada & 李 <developer>");
}

#[test]
fn conditional_cancel_preserves_a_replacement_session() {
    let Some((model, wav)) = common::local_model() else {
        eprintln!("skip: tiny.en and jfk fixture missing");
        return;
    };
    let tree = common::tree_with_tiny(&model, "mode = \"raw\"\n");
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
        ],
    );
    let started = daemon.result("dictation.start", json!({}));
    let id = started["job"]["job_id"].clone();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../dettivo-proto/fixtures/dictation/cancel.error-stale-job.json"
    ))
    .unwrap();
    let response = daemon.request("dictation.cancel", fixture["request"]["params"].clone());
    assert_eq!(response, fixture["error"], "{response}");
    let status = daemon.result("dictation.status", json!({}));
    assert_eq!(status["is_active"], true);
    assert_eq!(status["job"]["job_id"], id);
    let cancelled = daemon.result("dictation.cancel", json!({"expected_job_id":id}));
    assert_eq!(cancelled["job"]["state"], "cancelled");
    assert_eq!(
        daemon.result("dictation.status", json!({}))["is_active"],
        false
    );
}
