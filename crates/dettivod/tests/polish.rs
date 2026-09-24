//! R3: the `polish.*` and `llm.*` fixtures whose answer depends on state
//! the replay cannot build on one connection — a rule the daemon minted,
//! the rule the seed wrote, and the providers this machine happens to
//! have. `crates/dettivo-qa/src/replay.rs` lists them as `STATEFUL`, and
//! this is where they are covered instead.

mod common;

use common::{Daemon, Tree};
use serde_json::{Value, json};

/// The rule the QA seed writes, as `polish/rules.*.json` names it.
const SEED_RULE: &str = "c0a80101-0000-4000-8000-000000000001";

fn daemon() -> Daemon {
    Daemon::spawn(
        Tree::new(),
        &[("DETTIVO_QA_MODE", "1"), ("DETTIVO_E2E_SEED", "1")],
    )
}

#[test]
fn a_rule_is_created_updated_and_deleted_through_the_config_file() {
    let daemon = daemon();
    // The seed rule is there and the file the daemon wrote reads back.
    let listed = daemon.result("polish.rules.list", json!({}));
    assert_eq!(listed["rules"][0]["rule_id"], SEED_RULE, "{listed}");
    assert_eq!(listed["rules"][0]["name"], "Professional Tone", "{listed}");

    let created = daemon.result(
        "polish.rules.create",
        json!({"name": "House style", "enabled": true, "content": "Keep sentences short."}),
    );
    let id = created["rule_id"]
        .as_str()
        .expect("a minted id")
        .to_string();
    assert_eq!(id.len(), 36, "the daemon mints a UUID: {created}");
    let listed = daemon.result("polish.rules.list", json!({}));
    assert_eq!(listed["rules"].as_array().unwrap().len(), 2, "{listed}");

    let updated = daemon.result(
        "polish.rules.update",
        json!({"rule_id": id, "enabled": false, "content": "updated"}),
    );
    assert_eq!(updated["updated"], true, "{updated}");
    let listed = daemon.result("polish.rules.list", json!({}));
    let rule = listed["rules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["rule_id"] == id.as_str())
        .expect("the updated rule");
    assert_eq!(rule["content"], "updated", "{rule}");
    assert_eq!(rule["enabled"], false, "{rule}");

    let deleted = daemon.result("polish.rules.delete", json!({"rule_id": id}));
    assert_eq!(deleted["deleted"], true, "{deleted}");
    let listed = daemon.result("polish.rules.list", json!({}));
    assert_eq!(listed["rules"].as_array().unwrap().len(), 1, "{listed}");

    // A rule nobody has is `NOT_FOUND`, on both verbs.
    let missing = "d3b07384-d9a0-4c8e-9f1e-5a6b7c8d9e0f";
    let err = daemon.request(
        "polish.rules.update",
        json!({"rule_id": missing, "enabled": true, "content": "x"}),
    );
    assert_eq!(err["error"]["data"]["app_code"], "NOT_FOUND", "{err}");
    let err = daemon.request("polish.rules.delete", json!({"rule_id": missing}));
    assert_eq!(err["error"]["data"]["app_code"], "NOT_FOUND", "{err}");
    daemon.stop();
}

#[test]
fn an_over_long_rule_and_an_unknown_preset_are_refused_by_name() {
    let daemon = daemon();
    let err = daemon.request(
        "polish.rules.create",
        json!({"name": "Long", "enabled": true, "content": "x".repeat(501)}),
    );
    assert_eq!(err["error"]["data"]["app_code"], "INVALID_PARAMS");
    assert!(
        err["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("500 characters")),
        "{err}"
    );
    let err = daemon.request(
        "polish.apps.set",
        json!({"bundle_id": "org.gnome.Console", "preset": "nonsense"}),
    );
    assert_eq!(err["error"]["data"]["app_code"], "INVALID_PARAMS");
    assert!(
        err["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("email, code, chat, notes, generic")),
        "{err}"
    );
    let err = daemon.request(
        "polish.test",
        json!({"input": "hi", "preset": "generic", "rules": [], "style": "shouty"}),
    );
    assert_eq!(err["error"]["data"]["app_code"], "INVALID_PARAMS");
    assert!(
        err["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("asDictated")),
        "{err}"
    );
    daemon.stop();
}

#[test]
fn a_rule_write_keeps_the_comments_a_person_put_in_the_file() {
    let tree = Tree::new();
    let path = tree.config_file();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        "# my own file\n[hotkeys]\n# the hold key\nbackend = \"none\"\n",
    )
    .unwrap();
    let daemon = Daemon::spawn(tree, &[("DETTIVO_QA_MODE", "1")]);
    daemon.result(
        "polish.rules.create",
        json!({"name": "House style", "enabled": true, "content": "Keep sentences short."}),
    );
    daemon.result(
        "polish.apps.set",
        json!({"bundle_id": "org.gnome.Console", "preset": "code"}),
    );
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("# my own file"), "{text}");
    assert!(text.contains("# the hold key"), "{text}");
    assert!(text.contains("[[polish.rules]]"), "{text}");
    assert!(text.contains("House style"), "{text}");
    assert!(text.contains("org.gnome.Console"), "{text}");
    // The file the daemon wrote is the file it reads back.
    let mappings = daemon.result("polish.apps.list", json!({}));
    let ids: Vec<&str> = mappings["mappings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["bundle_id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"org.gnome.Console"), "{mappings}");
    daemon.stop();
}

#[test]
fn the_providers_are_listed_and_a_remote_endpoint_is_trusted_once() {
    let daemon = daemon();
    let providers = daemon.result("llm.providers.list", json!({}));
    let ids: Vec<&str> = providers["providers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    // The local slot is a documented stub until the engine spec; Ollama is
    // probed on the configured loopback endpoint.
    assert_eq!(ids, ["local", "ollama"], "{providers}");
    let local = &providers["providers"][0];
    assert_eq!(local["available"], false, "{local}");
    assert!(local["hint"].is_string(), "{local}");

    let listed = daemon.result("llm.endpoints.list", json!({}));
    assert_eq!(listed["endpoints"][0]["url"], "http://localhost:11434");
    assert_eq!(listed["endpoints"][0]["loopback"], true);

    // A loopback endpoint needs no entry; a remote one is stored once, in
    // its canonical form.
    let trusted = daemon.result(
        "llm.endpoints.trust",
        json!({"url": "http://127.0.0.1:11434/v1"}),
    );
    assert_eq!(trusted["url"], "http://localhost:11434");
    let trusted = daemon.result(
        "llm.endpoints.trust",
        json!({"url": "https://LLM.example.com/v1/"}),
    );
    assert_eq!(trusted["url"], "https://llm.example.com");
    daemon.result(
        "llm.endpoints.trust",
        json!({"url": "https://llm.example.com"}),
    );
    let listed: Value = daemon.result("llm.endpoints.list", json!({}));
    let remotes: Vec<&Value> = listed["endpoints"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["loopback"] == false)
        .collect();
    assert_eq!(remotes.len(), 1, "stored once: {listed}");
    let entry = daemon.result("config.get", json!({"key": "llm.trusted_endpoints"}));
    assert_eq!(
        entry["entries"][0]["value"],
        json!(["https://llm.example.com"]),
        "{entry}"
    );

    let err = daemon.request("llm.endpoints.trust", json!({"url": "not a url"}));
    assert_eq!(err["error"]["data"]["app_code"], "INVALID_PARAMS", "{err}");
    daemon.stop();
}
