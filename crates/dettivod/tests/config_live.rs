//! R5 against the live daemon: the file is loaded at start, changes are
//! picked up without a restart, environment overrides win, `config.set`
//! keeps comments and ordering, state goes only to the state file, and an
//! invalid file refuses the start by key and line, and a broken edit while
//! the daemon runs keeps the values in force with the failure visible
//! through `system.health` and `config.validate`.

mod common;

use std::time::{Duration, Instant};

use common::{Daemon, Tree};
use serde_json::{Value, json};

const FILE: &str = "# my dettivo config\n\n[daemon]\n# quiet by default\nlog_level = \"warn\"\n\n[qa]\nmode = false\n";

fn wait_for(daemon: &Daemon, key: &str, want: &Value) -> Value {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let entry = daemon.result("config.get", json!({"key": key}))["entries"][0].clone();
        if &entry["value"] == want || Instant::now() > deadline {
            return entry;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn set_preserves_comments_and_ordering_and_writes_only_the_config_file() {
    let tree = Tree::new();
    tree.write_config(FILE);
    let daemon = Daemon::spawn(tree, &[]);
    assert_eq!(
        std::fs::read_to_string(daemon.tree.config_file()).unwrap(),
        FILE,
        "starting the daemon does not rewrite the file"
    );
    let state_before = std::fs::read_to_string(daemon.tree.state_file()).unwrap();
    assert!(state_before.contains("last_started_unix"));

    let set = daemon.result(
        "config.set",
        json!({"key":"daemon.log_level","value":"debug"}),
    );
    assert_eq!(
        set,
        json!({"key":"daemon.log_level","value":"debug","source":"file"})
    );
    assert_eq!(
        std::fs::read_to_string(daemon.tree.config_file()).unwrap(),
        "# my dettivo config\n\n[daemon]\n# quiet by default\nlog_level = \"debug\"\n\n[qa]\nmode = false\n"
    );
    let set = daemon.result(
        "config.set",
        json!({"key":"ipc.max_line_bytes","value":"65536"}),
    );
    assert_eq!(set["value"], 65536);
    assert!(
        std::fs::read_to_string(daemon.tree.config_file())
            .unwrap()
            .ends_with("[qa]\nmode = false\n\n[ipc]\nmax_line_bytes = 65536\n")
    );
    assert_eq!(
        std::fs::read_to_string(daemon.tree.state_file()).unwrap(),
        state_before,
        "configuration writes never touch the state file"
    );

    let unset = daemon.result("config.unset", json!({"key":"daemon.log_level"}));
    assert_eq!(
        unset,
        json!({"key":"daemon.log_level","value":"info","source":"default"})
    );
    // The removed key's own comment goes with it when no key follows in
    // the section; the file comment, the sections and the other keys stay.
    assert_eq!(
        std::fs::read_to_string(daemon.tree.config_file()).unwrap(),
        "# my dettivo config\n\n[daemon]\n\n[qa]\nmode = false\n\n[ipc]\nmax_line_bytes = 65536\n"
    );
    daemon.stop();
}

#[test]
fn invalid_key_or_value_fails_set_and_validate_naming_the_key_and_never_writes() {
    let tree = Tree::new();
    tree.write_config(FILE);
    let daemon = Daemon::spawn(tree, &[]);
    let bad_value = daemon.request(
        "config.set",
        json!({"key":"daemon.log_level","value":"loud"}),
    );
    assert_eq!(bad_value["error"]["data"]["app_code"], "INVALID_PARAMS");
    assert_eq!(
        bad_value["error"]["data"]["details"]["key"],
        "daemon.log_level"
    );
    let bad_key = daemon.request("config.set", json!({"key":"daemon.colour","value":"blue"}));
    assert_eq!(bad_key["error"]["data"]["details"]["key"], "daemon.colour");
    assert_eq!(bad_key["error"]["message"], "daemon.colour: unknown key");
    let bad_type = daemon.request("config.set", json!({"key":"qa.mode","value":"maybe"}));
    assert_eq!(bad_type["error"]["data"]["details"]["key"], "qa.mode");
    assert_eq!(
        std::fs::read_to_string(daemon.tree.config_file()).unwrap(),
        FILE,
        "a refused set leaves the file untouched"
    );
    let validate = daemon.result("config.validate", json!({}));
    assert_eq!(validate["ok"], true);
    daemon.stop();
}

#[test]
fn environment_overrides_win_over_the_file_and_are_labelled() {
    let tree = Tree::new();
    tree.write_config("[qa]\nmode = false\n[ipc]\nsocket = \"/nowhere/file.sock\"\n");
    let socket = tree.socket();
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA", "1"),
            ("DETTIVO_IPC_SOCKET", socket.to_str().unwrap()),
        ],
    );
    let entries = daemon.result("config.get", json!({}))["entries"].clone();
    let find = |k: &str| {
        entries
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["key"] == k)
            .unwrap()
            .clone()
    };
    assert_eq!(
        find("qa.mode"),
        json!({"key":"qa.mode","value":true,"source":"environment"})
    );
    assert_eq!(find("ipc.socket")["source"], "environment");
    assert_eq!(find("ipc.socket")["value"], socket.to_str().unwrap());
    assert_eq!(find("daemon.log_level")["source"], "default");
    let path = daemon.result("config.path", json!({}));
    assert_eq!(path["socket"], socket.to_str().unwrap());
    daemon.stop();
}

#[test]
fn external_edits_are_picked_up_without_a_restart() {
    let tree = Tree::new();
    tree.write_config(FILE);
    // The file asks for `warn`; RUST_LOG keeps the reload line visible.
    let daemon = Daemon::spawn(tree, &[("RUST_LOG", "info")]);
    assert_eq!(
        daemon.result("config.get", json!({"key":"daemon.log_level"}))["entries"][0]["value"],
        "warn"
    );
    std::thread::sleep(Duration::from_millis(20));
    std::fs::write(
        daemon.tree.config_file(),
        "[daemon]\nlog_level = \"trace\"\n",
    )
    .unwrap();
    let entry = wait_for(&daemon, "daemon.log_level", &json!("trace"));
    assert_eq!(entry["value"], "trace");
    assert_eq!(entry["source"], "file");
    assert!(daemon.log().contains("configuration reloaded"));
    daemon.stop();
}

#[test]
fn an_invalid_file_refuses_the_start_naming_the_key_and_line_never_the_value() {
    let tree = Tree::new();
    tree.write_config("[daemon]\nlog_level = \"info\"\n\n[ipc]\ncolour = \"blue\"\n");
    let output = tree.command().output().expect("run dettivod");
    assert!(
        !output.status.success(),
        "an invalid file must not start the daemon"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("key ipc.colour line 5"), "{stderr}");
    assert!(stderr.contains("dettivo config edit"), "{stderr}");
    assert!(
        !stderr.contains("blue"),
        "values never reach the log: {stderr}"
    );
    assert!(!tree.socket().exists(), "nothing listens");
}

/// A request line with the shared token on it.
fn with_token(method: &str, params: Value) -> String {
    json!({"jsonrpc":"2.0","id":"1","method":method,"params":params,"auth_token":"s3cret"})
        .to_string()
}

#[test]
fn a_broken_edit_while_running_keeps_the_values_and_the_token_requirement_in_force() {
    let tree = Tree::new();
    tree.write_config(
        "[daemon]\nlog_level = \"warn\"\n[ipc]\nauth_mode = \"peer_token\"\n[history]\naudio_retention_days = 365\n",
    );
    let daemon = Daemon::spawn(tree, &[("DETTIVO_IPC_TOKEN", "s3cret")]);
    let refused = daemon.request("system.health", json!({}));
    assert_eq!(refused["error"]["data"]["app_code"], "UNAUTHORIZED_CLIENT");
    let health = daemon.call(&with_token("system.health", json!({})));
    assert_eq!(health["result"]["ok"], true);

    std::thread::sleep(Duration::from_millis(20));
    std::fs::write(
        daemon.tree.config_file(),
        "[daemon]\nlog_level = \"warn\"\n[ipc]\nauth_mode = \"peer_token\"\n[history]\naudio_retention_days = \n",
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while daemon.call(&with_token("system.health", json!({})))["result"]["ok"] != false
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    let validate = daemon.call(&with_token("config.validate", json!({})));
    assert_eq!(validate["result"]["ok"], false);
    assert_eq!(validate["result"]["errors"][0]["line"], 6);
    // The requirement and the values the file asked for stay in force.
    let refused = daemon.request("system.health", json!({}));
    assert_eq!(
        refused["error"]["data"]["app_code"], "UNAUTHORIZED_CLIENT",
        "a broken file never drops the token requirement: {refused}"
    );
    let entry = daemon.call(&with_token(
        "config.get",
        json!({"key":"history.audio_retention_days"}),
    ));
    assert_eq!(entry["result"]["entries"][0]["value"], 365);
    assert_eq!(entry["result"]["entries"][0]["source"], "file");
    let log = daemon.log();
    assert!(log.contains("keeping the values in force"), "{log}");

    std::fs::write(
        daemon.tree.config_file(),
        "[daemon]\nlog_level = \"warn\"\n[ipc]\nauth_mode = \"peer_token\"\n[history]\naudio_retention_days = 90\n",
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while daemon.call(&with_token("system.health", json!({})))["result"]["ok"] != true
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    let entry = daemon.call(&with_token(
        "config.get",
        json!({"key":"history.audio_retention_days"}),
    ));
    assert_eq!(entry["result"]["entries"][0]["value"], 90);
    daemon.stop();
}

#[test]
fn a_missing_file_is_the_default_configuration_and_set_creates_it() {
    let daemon = Daemon::spawn(Tree::new(), &[]);
    assert!(!daemon.tree.config_file().exists());
    let all = daemon.result("config.get", json!({}));
    assert!(all["entries"].as_array().unwrap().len() >= 9);
    assert!(
        all["entries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["source"] == "default")
    );
    daemon.result("config.set", json!({"key":"qa.mode","value":true}));
    assert_eq!(
        std::fs::read_to_string(daemon.tree.config_file()).unwrap(),
        "[qa]\nmode = true\n"
    );
    let text = daemon.result("config.print_default", json!({}))["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.starts_with("# Dettivo configuration"));
    daemon.stop();
}

/// `config.keys` is the registry the settings routes and the coverage
/// lint read (ADR 0033): every key the fixture lists, in the same shape,
/// and `config.changed` names the keys a `config.set` and a hand edit
/// moved, so an open editor refreshes the rows it shows.
#[test]
fn keys_match_the_fixture_and_changes_are_published_by_key() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;

    let tree = Tree::new();
    tree.write_config(FILE);
    let daemon = Daemon::spawn(tree, &[]);
    let keys = daemon.result("config.keys", json!({}));
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../dettivo-proto/fixtures/config/keys.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(keys, fixture["response"]["result"]);
    let names: Vec<&str> = keys["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["key"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"engines.whisper.backend"));
    assert!(names.contains(&"hotkeys.hold"));

    let stream = UnixStream::connect(daemon.tree.socket()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut writer = stream;
    let subscribe = json!({"jsonrpc": "2.0", "id": "s", "method": "events.subscribe", "params": {"topics": ["config.changed"], "buffer": 16}});
    writer
        .write_all(format!("{subscribe}\n").as_bytes())
        .unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let answer: Value = serde_json::from_str(line.trim_end()).unwrap();
    assert!(answer["result"]["subscription_id"].is_string(), "{answer}");

    daemon.result("config.set", json!({"key": "hotkeys.hold", "value": "F10"}));
    line.clear();
    reader.read_line(&mut line).unwrap();
    let event: Value = serde_json::from_str(line.trim_end()).unwrap();
    assert_eq!(event["params"]["topic"], "config.changed");
    assert_eq!(event["params"]["payload"]["ok"], true);
    assert_eq!(event["params"]["payload"]["keys"], json!(["hotkeys.hold"]));
    assert_eq!(
        event["params"]["payload"]["path"].as_str().unwrap(),
        daemon.tree.config_file().to_string_lossy()
    );

    // A hand edit on disk reaches the watcher and names its keys too: the
    // level it set and the `[qa]` key it dropped back to its default.
    std::fs::write(
        daemon.tree.config_file(),
        "[daemon]\nlog_level = \"debug\"\n[hotkeys]\nhold = \"F10\"\n",
    )
    .unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let event: Value = serde_json::from_str(line.trim_end()).unwrap();
    assert_eq!(event["params"]["topic"], "config.changed");
    assert_eq!(
        event["params"]["payload"]["keys"],
        json!(["daemon.log_level", "qa.mode"])
    );
    daemon.stop();
}

#[test]
fn concurrent_rule_creates_and_trusts_never_lose_each_other() {
    let tree = Tree::new();
    tree.write_config(FILE);
    let daemon = std::sync::Arc::new(Daemon::spawn(tree, &[]));
    let workers: Vec<_> = (0..8)
        .map(|w| {
            let daemon = daemon.clone();
            std::thread::spawn(move || {
                for i in 0..4 {
                    daemon.result(
                        "polish.rules.create",
                        json!({"name": format!("rule {w}-{i}"), "enabled": true, "content": "Keep it short."}),
                    );
                    daemon.result(
                        "llm.endpoints.trust",
                        json!({"url": format!("https://llm-{w}-{i}.example.com")}),
                    );
                }
            })
        })
        .collect();
    for w in workers {
        w.join().unwrap();
    }
    let rules = daemon.result("polish.rules.list", json!({}));
    assert_eq!(rules["rules"].as_array().unwrap().len(), 32, "{rules}");
    let trusted = daemon.result("config.get", json!({"key":"llm.trusted_endpoints"}));
    assert_eq!(
        trusted["entries"][0]["value"].as_array().unwrap().len(),
        32,
        "{trusted}"
    );
    let daemon = std::sync::Arc::into_inner(daemon).unwrap();
    daemon.stop();
}
