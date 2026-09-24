//! Shared Rust and Qt QA-hook conformance cases.

use dettivo_core::qa::{BuildKind, QaEnv};
use std::ffi::OsString;

#[test]
fn qt_and_rust_share_every_hook_conformance_case() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../fixtures/qa_environment_cases.json")).unwrap();
    assert!(!cases.is_empty());
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("input.wav");
    std::fs::write(&file, b"RIFF").unwrap();
    let missing = dir.path().join("absent.wav");
    let mut failures = Vec::new();
    for case in cases {
        let vars: Vec<_> = case["env"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| {
                let value = value
                    .as_str()
                    .unwrap()
                    .replace("$FILE", file.to_str().unwrap())
                    .replace("$MISSING", missing.to_str().unwrap());
                (key.clone(), OsString::from(value))
            })
            .collect();
        let build = if case["release"].as_bool().unwrap() {
            BuildKind::Release
        } else {
            BuildKind::Debug
        };
        let result = QaEnv::from_vars(&vars, build);
        if result.is_ok() != case["accepted"].as_bool().unwrap() {
            failures.push(format!("{}: {result:?}", case["name"]));
        } else if let Err(error) = result {
            assert_eq!(
                error.variable,
                case["error_variable"].as_str().unwrap(),
                "{}",
                case["name"]
            );
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
