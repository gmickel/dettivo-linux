//! R2 end to end: a daemon stand-in answers oversized payloads and the
//! server binary cuts them with the markers; a message over the byte cap
//! is refused naming the cap and the session continues.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::thread;

use common::*;
use dettivo_mcp::bounds::{
    DEPTH_MARKER, MAX_DEPTH, MAX_ITEMS, MAX_MESSAGE_BYTES, MAX_OBJECT_KEYS, MAX_TEXT_CHARS,
    TRUNCATED_KEYS,
};
use dettivo_mcp::harness::{self, Session};
use dettivo_mcp::transport::Framing;
use serde_json::{Map, Value, json};

/// A socket that answers every method from `answer` (one request per
/// connection, like the daemon), until the listener is dropped.
fn fake_daemon(socket: &Path, answer: impl Fn(&str) -> Value + Send + 'static) {
    std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
    let listener = UnixListener::bind(socket).unwrap();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                continue;
            }
            let request: Value = serde_json::from_str(line.trim_end()).unwrap_or(Value::Null);
            let method = request["method"].as_str().unwrap_or("").to_string();
            let response =
                json!({"jsonrpc": "2.0", "id": request["id"], "result": answer(&method)});
            let mut stream = stream;
            let _ = stream.write_all(format!("{response}\n").as_bytes());
        }
    });
}

fn oversized() -> Value {
    let mut wide = Map::new();
    for i in 0..(MAX_OBJECT_KEYS + 20) {
        wide.insert(format!("k{i:03}"), json!(i));
    }
    let mut deep = json!("leaf");
    for _ in 0..(MAX_DEPTH + 3) {
        deep = json!({"child": deep});
    }
    json!({
        "ref": {"kind": "dictation", "id": harness::SAMPLE_ID},
        "text_raw": "x".repeat(MAX_TEXT_CHARS + 500),
        "text_polish": "y".repeat(MAX_TEXT_CHARS + 5),
        "segments": (0..(MAX_ITEMS + 10)).map(|i| json!({"index": i})).collect::<Vec<_>>(),
        "wide": Value::Object(wide),
        "deep": deep,
    })
}

#[test]
fn malformed_resource_escapes_are_errors_and_the_session_continues() {
    let tree = Tree::new();
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/resources/malformed.json")).unwrap();
    for framing in [Framing::Line, Framing::ContentLength] {
        let mut command = harness::server_command(Path::new(SERVER), &tree.socket());
        tree.env(&mut command);
        let mut session = Session::spawn(command, framing).unwrap();
        for uri in fixture["uris"].as_array().unwrap() {
            let response = session
                .request("resources/read", json!({"uri": uri}))
                .unwrap();
            assert_eq!(
                response["error"]["code"], fixture["error_code"],
                "{uri}: {response}"
            );
            assert_eq!(session.result("ping", json!({})).unwrap(), json!({}));
        }
    }
}

#[test]
fn oversized_payloads_are_cut_with_the_markers() {
    let tree = Tree::new();
    fake_daemon(&tree.socket(), |method| match method {
        "transcripts.get" => oversized(),
        "system.ping" => json!({"ok": true}),
        _ => json!({}),
    });
    wait_ready(&tree.socket());
    let mut session = Session::spawn(
        harness::server_command(Path::new(SERVER), &tree.socket()),
        Framing::Line,
    )
    .unwrap();
    let r = session
        .call_tool(
            "get_transcript",
            json!({"id": harness::SAMPLE_ID, "kind": "dictation"}),
        )
        .unwrap();
    assert_eq!(r["_truncated"], true, "{r}");
    let v = &r["structuredContent"];
    let polish = v["text_polish"].as_str().unwrap();
    assert!(
        polish.ends_with("\n\n[truncated 5 chars]"),
        "{}",
        &polish[polish.len() - 40..]
    );
    assert_eq!(
        polish.chars().count(),
        MAX_TEXT_CHARS + "\n\n[truncated 5 chars]".len()
    );
    assert_eq!(v["segments"].as_array().unwrap().len(), MAX_ITEMS);
    assert_eq!(v["wide"][TRUNCATED_KEYS], json!(20));
    assert_eq!(v["wide"].as_object().unwrap().len(), MAX_OBJECT_KEYS + 1);
    let mut cursor = &v["deep"];
    while cursor.is_object() {
        cursor = &cursor["child"];
    }
    assert_eq!(cursor, &json!(DEPTH_MARKER));
    let text = Session::text_of(&r);
    assert!(text.contains("[truncated 5 chars]"));

    let read = session
        .result(
            "resources/read",
            json!({"uri": format!("transcript://{}", harness::SAMPLE_ID)}),
        )
        .unwrap();
    let body: Value = serde_json::from_str(read["contents"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(body["segments"].as_array().unwrap().len(), MAX_ITEMS);
    assert_eq!(body["wide"][TRUNCATED_KEYS], json!(20));

    let small = session.call_tool("get_status", json!({})).unwrap();
    assert!(small.get("_truncated").is_none(), "{small}");
}

#[test]
fn a_message_over_the_byte_cap_is_refused_naming_the_cap_and_the_session_continues() {
    let tree = Tree::new();
    fake_daemon(&tree.socket(), |_| json!({"ok": true}));
    wait_ready(&tree.socket());
    for framing in [Framing::Line, Framing::ContentLength] {
        let mut session = Session::spawn(
            harness::server_command(Path::new(SERVER), &tree.socket()),
            framing,
        )
        .unwrap();
        session.result("initialize", json!({})).unwrap();
        let padding = "p".repeat(MAX_MESSAGE_BYTES as usize + 10);
        let big = json!({"jsonrpc": "2.0", "id": 99, "method": "ping", "params": {"pad": padding}});
        session
            .write_raw(&dettivo_mcp::transport::encode(framing, &big))
            .unwrap();
        let refused = session.read().unwrap();
        assert_eq!(refused["id"], Value::Null, "{framing:?}");
        assert_eq!(refused["error"]["code"], -32016);
        assert_eq!(refused["error"]["data"]["app_code"], "RATE_LIMITED_LOCAL");
        assert!(
            refused["error"]["message"]
                .as_str()
                .unwrap()
                .contains(&format!("({MAX_MESSAGE_BYTES})")),
            "{refused}"
        );
        assert_eq!(session.result("ping", json!({})).unwrap(), json!({}));
        let status = session.call_tool("get_status", json!({})).unwrap();
        assert_eq!(status["structuredContent"]["ok"], true);
    }
}
