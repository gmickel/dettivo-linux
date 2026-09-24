//! Single dispatch decoding, aggregated diagnostics, and serialized native toggles.

mod common;
use common::{Daemon, Tree};
use serde_json::json;

#[test]
fn dispatch_keeps_contract_field_diagnostics() {
    let daemon = Daemon::spawn(Tree::new(), &[]);
    for (method, value) in [
        ("system.ping", json!({"stray": 1})),
        ("dictation.start", json!({"mode": "bad"})),
        ("meetings.get", json!({"meeting_id": "bad"})),
        ("hotkeys.snippet", json!(null)),
    ] {
        let expected = dettivo_proto::catalog::round_trip_params(method, &value).unwrap_err();
        let dettivo_proto::catalog::CatalogError::Shape { message, .. } = expected else {
            panic!()
        };
        let response = daemon.request(method, value);
        assert_eq!(response["error"]["message"], message, "{method}");
        assert_eq!(response["error"]["data"]["app_code"], "INVALID_PARAMS");
    }
    daemon.stop();
}

#[test]
fn daemon_aggregates_required_diagnostic_probes() {
    let daemon = Daemon::spawn(Tree::new(), &[]);
    let result = daemon.result("system.diagnostics", json!({}));
    assert_eq!(result["probes"].as_object().unwrap().len(), 13);
    for method in [
        "system.health",
        "config.validate",
        "speech.engines",
        "llm.engine.status",
    ] {
        assert_eq!(
            result["probes"][method]["state"], "success",
            "{method}: {result}"
        );
        assert_eq!(
            result["probes"][method]["result"],
            daemon.result(method, json!({}))
        );
        assert!(result["probes"][method]["error"].is_null());
    }
    daemon.stop();
}

#[test]
fn toggles_serialize_and_preserve_configured_mode() {
    let (model, wav) = common::local_model().expect("native dictation fixtures required");
    let daemon = Daemon::spawn(
        common::tree_with_tiny(&model, "mode = \"deterministic_polish\"\n"),
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
        ],
    );
    assert_eq!(
        daemon.result("dictation.toggle", json!({}))["job"]["state"],
        "running"
    );
    std::thread::sleep(std::time::Duration::from_millis(2500));
    let responses = std::thread::scope(|s| {
        let a = s.spawn(|| daemon.result("dictation.toggle", json!({})));
        let b = s.spawn(|| daemon.result("dictation.toggle", json!({})));
        [a.join().unwrap(), b.join().unwrap()]
    });
    let stopped = responses
        .iter()
        .find(|r| r["job"]["state"] == "succeeded")
        .expect("one stopped");
    assert!(responses.iter().all(|r| r["job"] == stopped["job"]));
    let item = daemon.result("transcripts.get", json!({"ref": stopped["ref"]}));
    assert_eq!(item["mode"], "deterministic_polish");
    assert_eq!(
        daemon.result("dictation.status", json!({}))["is_active"],
        false
    );
    daemon.stop();
}
