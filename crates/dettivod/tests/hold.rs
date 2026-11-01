//! The hold verbs against the live daemon (ADR 0083): `dictation.press`
//! and `dictation.release` are idempotent, a repeated press inside a
//! second is ignored, a press more than a second into a hold whose release
//! was lost finishes the take, and a tap followed by a press within 300 ms
//! locks recording: the release leaves it running, the pill is told, and
//! the next press finishes it.

mod common;

use std::time::Duration;

use common::{Daemon, local_model, tree_with_tiny, wait_for};
use serde_json::{Value, json};

fn active(daemon: &Daemon) -> bool {
    daemon.result("dictation.status", json!({}))["is_active"] == Value::Bool(true)
}

fn edge(daemon: &Daemon, method: &str) -> (String, String) {
    let r = daemon.result(method, json!({}));
    (
        r["action"].as_str().unwrap().to_string(),
        r["hold"].as_str().unwrap().to_string(),
    )
}

fn pair(action: &str, hold: &str) -> (String, String) {
    (action.to_string(), hold.to_string())
}

#[test]
fn the_hold_verbs_survive_a_lost_release_and_lock_on_a_double_tap() {
    let Some((model, wav)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let tree = tree_with_tiny(&model, "[hotkeys]\nbackend = \"none\"\n");
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
            ("DETTIVO_MOCK_INSERT", "1"),
        ],
    );

    // Idempotent: a release with nothing held does nothing.
    assert_eq!(edge(&daemon, "dictation.release"), pair("none", "idle"));

    // A press starts; the key repeating is ignored; a press after a second
    // finishes the take whose release never came, and its release is a no-op.
    let started = daemon.result("dictation.press", json!({}));
    assert_eq!(started["action"], "started");
    assert_eq!(started["job_id"], "job_dict_1");
    assert_eq!(edge(&daemon, "dictation.press"), pair("none", "held"));
    assert!(active(&daemon));
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(edge(&daemon, "dictation.press"), pair("stopped", "idle"));
    assert!(!active(&daemon), "the take is settled before the answer");
    assert_eq!(edge(&daemon, "dictation.release"), pair("none", "idle"));

    // A tap drops its take; a press within 300 ms starts the take that
    // the release locks; the next press finishes it.
    assert_eq!(edge(&daemon, "dictation.press"), pair("started", "held"));
    assert_eq!(
        edge(&daemon, "dictation.release"),
        pair("discarded", "tapped")
    );
    assert!(!active(&daemon));
    assert_eq!(edge(&daemon, "dictation.press"), pair("started", "held"));
    std::thread::sleep(Duration::from_millis(500));
    let locked = daemon.result("dictation.release", json!({}));
    assert_eq!(locked["action"], "locked");
    assert_eq!(locked["hold"], "locked");
    assert!(active(&daemon), "releasing does not stop a locked take");
    assert!(
        daemon.log().contains("dictation locked"),
        "{}",
        daemon.log()
    );
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(edge(&daemon, "dictation.release"), pair("none", "locked"));
    assert!(active(&daemon));
    assert_eq!(edge(&daemon, "dictation.press"), pair("stopped", "idle"));
    assert!(!active(&daemon));

    // A take that ended elsewhere returns the key to idle: the next press
    // starts a fresh take rather than stopping a gone one.
    assert_eq!(edge(&daemon, "dictation.press"), pair("started", "held"));
    daemon.result("dictation.cancel", json!({}));
    assert!(wait_for(Duration::from_secs(15), || !active(&daemon)));
    std::thread::sleep(Duration::from_millis(1200));
    assert_eq!(edge(&daemon, "dictation.press"), pair("started", "held"));
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(edge(&daemon, "dictation.release"), pair("stopped", "idle"));
    daemon.stop();
}
