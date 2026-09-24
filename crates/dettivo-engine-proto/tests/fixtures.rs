//! R1: every fixture round-trips byte-stable through the typed payloads
//! and the frame codec, every request and event name has a fixture, and
//! a malformed frame is rejected with the field named.

use std::path::{Path, PathBuf};

use dettivo_engine_proto::{
    EVENTS, Frame, FrameError, REQUESTS, check_event, check_request, check_response, read_frame,
    write_frame,
};
use serde_json::Value;

fn fixtures() -> Vec<(PathBuf, Value)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let mut out: Vec<(PathBuf, Value)> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|p| {
            let doc = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            (p, doc)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn every_fixture_round_trips_byte_stable() {
    for (path, doc) in fixtures() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let frame: Frame =
            serde_json::from_value(doc.clone()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let back = serde_json::to_value(&frame).unwrap();
        assert_eq!(back, doc, "{name}: frame header drift");
        let kind = doc["kind"].as_str().unwrap();
        let payload = &doc["payload"];
        let checked = match kind {
            "request" => check_request(&frame.name, payload),
            "response" => check_response(&frame.name, payload),
            "event" => check_event(&frame.name, payload),
            other => panic!("{name}: kind {other}"),
        }
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(&checked, payload, "{name}: payload drift");
        assert!(
            name.starts_with(&format!("{kind}.{}", frame.name)),
            "{name} does not name its message"
        );

        let attachments: Vec<Vec<u8>> = frame
            .attachments
            .iter()
            .map(|a| vec![0u8; a.bytes as usize])
            .collect();
        let refs: Vec<&[u8]> = attachments.iter().map(Vec::as_slice).collect();
        let mut buf = Vec::new();
        write_frame(&mut buf, &frame, &refs).unwrap();
        let (decoded, bytes) = read_frame(&mut buf.as_slice()).unwrap();
        assert_eq!(decoded, frame, "{name}: wire drift");
        assert_eq!(bytes, attachments);
    }
}

#[test]
fn every_request_and_event_name_has_a_fixture() {
    let names: Vec<String> = fixtures()
        .into_iter()
        .map(|(p, _)| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    for r in REQUESTS {
        assert!(
            names.iter().any(|n| n == &format!("request.{r}.json")),
            "request.{r}"
        );
        assert!(
            names.iter().any(|n| n == &format!("response.{r}.json")),
            "response.{r}"
        );
    }
    for e in EVENTS {
        assert!(
            names.iter().any(|n| n == &format!("event.{e}.json")),
            "event.{e}"
        );
    }
}

#[test]
fn drift_is_rejected_with_the_field_named() {
    let err = check_request(
        "recognize",
        &serde_json::json!({"language": "en", "stray": 1}),
    )
    .unwrap_err();
    assert!(err.contains("stray"), "{err}");
    let err = check_response(
        "load",
        &serde_json::json!({"model": "m", "backend": "gpu", "reason": ""}),
    )
    .unwrap_err();
    assert!(err.contains("backend"), "{err}");
    assert!(check_request("frobnicate", &serde_json::json!({})).is_err());
    let mut buf = Vec::new();
    buf.extend_from_slice(&(3u32).to_be_bytes());
    buf.extend_from_slice(b"{\"v");
    assert!(matches!(
        read_frame(&mut buf.as_slice()),
        Err(FrameError::Malformed(_))
    ));
}

/// R7: a header that cannot be decoded is reported by field and kind, never
/// by quoting the value, so a prompt or a transcript in a bad frame cannot
/// reach a log through the error.
#[test]
fn a_malformed_header_never_echoes_its_content() {
    let header = br#"{"v":1,"id":"MARKER_PROMPT_TEXT","kind":"request","name":"load","payload":{},"attachments":[]}"#;
    let mut buf = (header.len() as u32).to_be_bytes().to_vec();
    buf.extend_from_slice(header);
    let err = read_frame(&mut buf.as_slice()).unwrap_err();
    let text = err.to_string();
    assert!(text.contains("id"), "{text}");
    assert!(text.contains("wrong type"), "{text}");
    assert!(!text.contains("MARKER"), "value echoed: {text}");
}
