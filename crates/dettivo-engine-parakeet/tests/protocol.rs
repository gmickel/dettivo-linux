//! R2 over the protocol without a model: `status` before any load, an
//! unknown request answered with the protocol's error shape, `recognize`
//! before `load` refused, `cancel` acknowledged, a missing model named,
//! and a clean exit when stdin closes.

use std::io::{BufReader, Write};
use std::process::{Command, Stdio};

use dettivo_engine_proto::{Frame, Kind, read_frame, write_frame};
use serde_json::json;

const ENGINE: &str = env!("CARGO_BIN_EXE_dettivo-engine-parakeet");

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

    let unknown = call(Frame::request(2, "frobnicate", json!({})));
    assert_eq!(unknown.name, "error");
    assert_eq!(unknown.payload["code"], "bad_request");
    assert_eq!(unknown.payload["request_id"], 2);
    assert!(
        unknown.payload["message"]
            .as_str()
            .unwrap()
            .contains("frobnicate")
    );

    let early = call(Frame::request(3, "recognize", json!({"language": "en"})));
    assert_eq!(early.payload["code"], "bad_request");

    let cancel = call(Frame::request(4, "cancel", json!({"request_id": 3})));
    assert_eq!(cancel.name, "cancel");

    let missing = call(Frame::request(
        5,
        "load",
        json!({"model": "/nonexistent/parakeet.gguf"}),
    ));
    assert_eq!(missing.payload["code"], "model_missing");
    assert!(
        missing.payload["message"]
            .as_str()
            .unwrap()
            .contains("/nonexistent/parakeet.gguf")
    );

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success(), "{status}");
}
