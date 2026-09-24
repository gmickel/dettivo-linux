//! daemon/F14: `[daemon] log_level` applies on reload, so what
//! `dettivo config get` reports as in force is the level the journal
//! receives; `RUST_LOG` keeps its filter for the run.

mod common;

use std::time::Duration;

use common::{Daemon, Tree};
use serde_json::json;

fn touch(daemon: &Daemon) {
    // A connection closing logs "connection closed" at debug.
    daemon.result("system.ping", json!({}));
}

#[test]
fn a_reloaded_log_level_reaches_the_journal() {
    let tree = Tree::new();
    tree.write_config("[daemon]\nlog_level = \"warn\"\n");
    let daemon = Daemon::spawn(tree, &[]);
    touch(&daemon);
    std::thread::sleep(Duration::from_millis(200));
    let before = daemon.log();
    assert!(
        !before.contains("connection closed"),
        "warn was in force before the change:\n{before}"
    );
    daemon.result(
        "config.set",
        json!({"key": "daemon.log_level", "value": "debug"}),
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while daemon.result("config.get", json!({"key": "daemon.log_level"}))["entries"][0]["value"]
        != json!("debug")
    {
        assert!(
            std::time::Instant::now() < deadline,
            "the reload did not land"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    touch(&daemon);
    let log = daemon.stop();
    assert!(
        log.contains("connection closed"),
        "debug did not apply:\n{log}"
    );
}

#[test]
fn rust_log_keeps_its_filter_for_the_run() {
    let tree = Tree::new();
    tree.write_config("[daemon]\nlog_level = \"warn\"\n");
    let daemon = Daemon::spawn(tree, &[("RUST_LOG", "warn")]);
    daemon.result(
        "config.set",
        json!({"key": "daemon.log_level", "value": "debug"}),
    );
    std::thread::sleep(Duration::from_millis(300));
    touch(&daemon);
    let log = daemon.stop();
    assert!(!log.contains("connection closed"), "{log}");
}
