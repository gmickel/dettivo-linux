//! A start without a mode runs in the configured `[dictation] mode`, the
//! same default the portal and evdev hotkeys apply, and an explicit
//! `raw` still overrides it (ADR 0047).

mod common;

use std::time::Duration;

use common::{Daemon, local_model};
use serde_json::json;

#[test]
fn a_start_without_a_mode_runs_in_the_configured_mode() {
    let Some((model, wav)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let tree = common::tree_with_tiny(&model, "mode = \"deterministic_polish\"\n");
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
        ],
    );
    for (params, expected) in [
        (json!({"language": "en"}), "deterministic_polish"),
        (json!({"language": "en", "mode": "raw"}), "raw"),
    ] {
        let started = daemon.result("dictation.start", params.clone());
        assert_eq!(started["job"]["state"], "running", "{params}");
        std::thread::sleep(Duration::from_millis(2500));
        let stopped = daemon.result("dictation.stop", json!({}));
        assert_eq!(stopped["job"]["state"], "succeeded", "{params}");
        let item = daemon.result(
            "transcripts.get",
            json!({"ref": {"kind": "dictation", "id": stopped["ref"]["id"]}}),
        );
        assert_eq!(item["mode"], expected, "{params}: {item}");
    }
    daemon.stop();
}
