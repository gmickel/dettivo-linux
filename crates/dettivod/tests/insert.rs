//! R3 and R5 against the live daemon: the QA mock backend records what
//! `insert.perform` typed, the guards refuse a mismatch with `CONFLICT`
//! before anything is written, a Dettivo window as target fails with
//! `target_is_self` and nothing is written, clipboard-only mode reports
//! `copied_to_clipboard`, an unverifiable target never pastes, and
//! `insert.undo` names why it cannot undo.

mod common;

use common::{Daemon, Tree};
use serde_json::json;

const MOCK_ENV: &[(&str, &str)] = &[("DETTIVO_QA_MODE", "1"), ("DETTIVO_MOCK_INSERT", "1")];

fn inserted(daemon: &Daemon) -> String {
    std::fs::read_to_string(daemon.tree.root().join("state/dettivo/qa/inserted.txt"))
        .unwrap_or_default()
}

#[test]
fn perform_writes_through_the_mock_and_reports_the_backend() {
    let daemon = Daemon::spawn(Tree::new(), MOCK_ENV);
    let result = daemon.result(
        "insert.perform",
        json!({"mode": "raw", "text": "Hello team", "expected_target_pid": "4141"}),
    );
    assert_eq!(result["outcome"], "inserted");
    assert_eq!(result["method"], "paste");
    assert_eq!(result["backend"]["name"], "mock");
    assert_eq!(result["backend"]["undo_supported"], false);
    assert_eq!(result["target_app"]["bundle_id"], "org.gnome.TextEditor");
    assert_eq!(inserted(&daemon), "Hello team\n");

    let copied = daemon.result(
        "insert.perform",
        json!({"mode": "clipboard_only", "text": "Copy me"}),
    );
    assert_eq!(copied["outcome"], "copied_to_clipboard");
    assert_eq!(copied["method"], "clipboard_only");
    assert_eq!(copied["backend"]["name"], "mock_clipboard");
    assert_eq!(inserted(&daemon), "Hello team\nCopy me\n");

    let undo = daemon.result("insert.undo", json!({}));
    assert_eq!(undo["undone"], false);
    assert_eq!(undo["reason"], "unsupported_backend");
    let target = daemon.result("insert.target", json!({}));
    assert_eq!(target["probe"], "mock");
    assert_eq!(target["chosen"], "mock");
    assert_eq!(target["target"]["pid"], 4141);
    let log = daemon.stop();
    assert!(
        !log.contains("Hello team") && !log.contains("Copy me"),
        "{log}"
    );
}

#[test]
fn guard_mismatch_is_conflict_and_nothing_is_written() {
    let daemon = Daemon::spawn(Tree::new(), MOCK_ENV);
    for params in [
        json!({"mode": "raw", "text": "never", "expected_target_pid": "4242"}),
        json!({"mode": "raw", "text": "never", "expected_target_bundle_id": "org.kde.kate"}),
    ] {
        let resp = daemon.request("insert.perform", params);
        assert_eq!(resp["error"]["data"]["app_code"], "CONFLICT", "{resp}");
        assert_eq!(resp["error"]["code"], -32013);
    }
    assert_eq!(inserted(&daemon), "");
    daemon.stop();
}

#[test]
fn a_dettivo_window_as_target_fails_with_target_is_self_and_nothing_is_written() {
    let tree = Tree::new();
    tree.write_config("[insert]\nself_app_ids = [\"dettivo-app\", \"org.gnome.TextEditor\"]\n");
    let daemon = Daemon::spawn(tree, MOCK_ENV);
    let result = daemon.result(
        "insert.perform",
        json!({"mode": "raw", "text": "into myself"}),
    );
    assert_eq!(result["outcome"], "failed");
    assert_eq!(result["reason"], "target_is_self");
    assert!(result.get("backend").is_none(), "{result}");
    assert_eq!(inserted(&daemon), "");
    daemon.stop();
}

#[test]
fn a_pinned_backend_that_is_unavailable_fails_with_its_reason() {
    let tree = Tree::new();
    tree.write_config("[insert]\nbackend = \"ydotool\"\n");
    let daemon = Daemon::spawn(tree, MOCK_ENV);
    let result = daemon.result("insert.perform", json!({"mode": "raw", "text": "pinned"}));
    assert_eq!(result["outcome"], "failed");
    assert!(
        result["reason"]
            .as_str()
            .unwrap()
            .contains("pinned backend ydotool"),
        "{result}"
    );
    assert_eq!(inserted(&daemon), "");
    daemon.stop();
}

#[test]
fn without_a_display_nothing_is_typed_and_a_guard_cannot_be_verified() {
    let daemon = Daemon::spawn(Tree::new(), &[]);
    let guarded = daemon.request(
        "insert.perform",
        json!({"mode": "raw", "text": "x", "expected_target_bundle_id": "foot"}),
    );
    assert_eq!(
        guarded["error"]["data"]["app_code"], "CONFLICT",
        "{guarded}"
    );
    let bare = daemon.result("insert.perform", json!({"mode": "raw", "text": "x"}));
    assert_eq!(bare["outcome"], "failed");
    assert!(
        bare["reason"]
            .as_str()
            .unwrap()
            .contains("no backend available"),
        "{bare}"
    );
    let target = daemon.result("insert.target", json!({}));
    assert_eq!(target["probe"], "none");
    assert!(target["target"].is_null());
    assert_eq!(target["backends"].as_array().unwrap().len(), 6);
    let missing = daemon.request(
        "insert.perform",
        json!({"mode": "raw", "source_ref": {"kind": "meeting", "id": "0f8fad5b-d9cb-469f-a165-70867728950e"}}),
    );
    assert_eq!(missing["error"]["data"]["app_code"], "NOT_FOUND");
    let caps = daemon.result("system.capabilities", json!({}));
    assert_eq!(caps["platform"]["insertion_backend"], "none");
    assert_eq!(caps["insert"]["methods"][0], "insert.undo");
    daemon.stop();
}

/// The self-target allowance (ADR 0024): the first-run Try it step arms
/// it on its connection, an `insert.perform` that asks for it then lands
/// in a Dettivo window, one that does not ask keeps the refusal, and the
/// allowance ends with the connection that armed it; outside QA mode a
/// peer that is not `dettivo-app` cannot arm it at all.
#[test]
fn the_self_target_allowance_is_held_by_the_step_and_refused_outside_it() {
    let tree = Tree::new();
    tree.write_config("[insert]\nself_app_ids = [\"dettivo-app\", \"org.gnome.TextEditor\"]\n");
    let daemon = Daemon::spawn(tree, MOCK_ENV);
    let ask = json!({"mode": "raw", "text": "into myself", "allow_self_target": true});
    let refused = daemon.result("insert.perform", ask.clone());
    assert_eq!(refused["reason"], "target_is_self");
    assert_eq!(inserted(&daemon), "");

    let mut step = common::Conn::open(&daemon.tree.socket());
    let armed = step.send(
        &json!({"jsonrpc": "2.0", "id": "a", "method": "insert.allow_self_target", "params": {"enabled": true}}).to_string(),
    );
    assert_eq!(armed["result"]["enabled"], true);
    assert_eq!(
        armed["result"]["app_ids"],
        json!(["dettivo", "dettivo-app"])
    );
    let landed = daemon.result("insert.perform", ask.clone());
    assert_eq!(landed["outcome"], "inserted", "{landed}");
    assert_eq!(inserted(&daemon), "into myself\n");
    // Without the request flag the chain's refusal stands even while armed.
    let plain = daemon.result("insert.perform", json!({"mode": "raw", "text": "no flag"}));
    assert_eq!(plain["reason"], "target_is_self");
    let disarmed = step.send(
        &json!({"jsonrpc": "2.0", "id": "b", "method": "insert.allow_self_target", "params": {"enabled": false}}).to_string(),
    );
    assert_eq!(disarmed["result"]["enabled"], false);
    assert_eq!(
        daemon.result("insert.perform", ask.clone())["reason"],
        "target_is_self"
    );
    step.send(
        &json!({"jsonrpc": "2.0", "id": "c", "method": "insert.allow_self_target", "params": {"enabled": true}}).to_string(),
    );
    assert_eq!(
        daemon.result("insert.perform", ask.clone())["outcome"],
        "inserted"
    );
    drop(step);
    // The daemon notices the closed connection on its own time, so the
    // wait reads the allowance instead of performing: a disarm from a
    // connection that never armed it changes nothing and answers with the
    // live state. Polling with `insert.perform` would land a take in the
    // window between the drop and the daemon's disarm.
    assert!(
        common::wait_for(std::time::Duration::from_secs(5), || {
            daemon.result("insert.allow_self_target", json!({"enabled": false}))["enabled"] == false
        }),
        "the allowance outlived the connection that armed it"
    );
    assert_eq!(
        daemon.result("insert.perform", ask.clone())["reason"],
        "target_is_self"
    );
    assert_eq!(inserted(&daemon), "into myself\ninto myself\n");
    daemon.stop();

    // Outside QA mode only the app's own process may arm it.
    let daemon = Daemon::spawn(Tree::new(), &[]);
    let refused = daemon.request("insert.allow_self_target", json!({"enabled": true}));
    assert_eq!(refused["error"]["data"]["app_code"], "CONFLICT");
    assert_eq!(refused["error"]["data"]["details"]["kind"], "notTheApp");
    assert!(
        refused["error"]["message"]
            .as_str()
            .unwrap()
            .contains("only dettivo-app may allow insertion into itself"),
        "{refused}"
    );
    daemon.stop();
}
