//! R1 over the protocol without a model: `status` before any load, an
//! unknown request answered with the protocol's error shape, `generate`
//! before `load` refused, `cancel` acknowledged, a missing model named
//! with the reason, and a clean exit when stdin closes.

use std::io::{BufReader, Write};
use std::process::{Command, Stdio};

use dettivo_engine_proto::{Frame, Kind, read_frame, write_frame};
use serde_json::json;

const ENGINE: &str = env!("CARGO_BIN_EXE_dettivo-engine-llm");

#[test]
fn the_protocol_answers_every_request_shape_without_a_model() {
    let mut child = Command::new(ENGINE)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut call = |frame: Frame| -> Frame {
        write_frame(&mut stdin, &frame, &[]).unwrap();
        stdin.flush().unwrap();
        loop {
            let (reply, _) = read_frame(&mut stdout).unwrap();
            if reply.kind == Kind::Response {
                return reply;
            }
        }
    };
    let status = call(Frame::request(1, "status", json!({})));
    assert_eq!(status.name, "status");
    assert_eq!(status.payload["loaded"], false);
    assert_eq!(status.payload["busy"], false);

    let unknown = call(Frame::request(2, "frobnicate", json!({})));
    assert_eq!(unknown.name, "error");
    assert_eq!(unknown.payload["code"], "bad_request");
    assert_eq!(unknown.payload["request_id"], 2);

    let early = call(Frame::request(3, "generate", json!({"user": "hi"})));
    assert_eq!(early.payload["code"], "bad_request");
    assert!(
        early.payload["message"]
            .as_str()
            .unwrap()
            .contains("no model is loaded")
    );

    let recognize = call(Frame::request(4, "recognize", json!({"language": "en"})));
    assert_eq!(recognize.payload["code"], "bad_request");

    let cancel = call(Frame::request(5, "cancel", json!({"request_id": 3})));
    assert_eq!(cancel.name, "cancel");

    let missing = call(Frame::request(
        6,
        "load",
        json!({"model": "/nonexistent/llm.gguf"}),
    ));
    assert_eq!(missing.payload["code"], "model_missing");
    let message = missing.payload["message"].as_str().unwrap();
    assert!(message.contains("/nonexistent/llm.gguf"), "{message}");
    assert!(message.contains("not found"), "{message}");

    let bad = call(Frame::request(7, "generate", json!({"nope": 1})));
    assert_eq!(bad.payload["code"], "bad_request");

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success(), "{status}");
}
