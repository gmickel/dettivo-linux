//! fn-91 R1, R2 and R4 against the daemon binary: a stand-in Hyprland
//! instance and a stand-in Wayland compositor on private sockets under the
//! test's own runtime directory stand in for the desktop. The compositor
//! restarts under a running daemon whose environment keeps the dead
//! instance's signature, and a daemon starts with a stale signature; both
//! type the next insertion through the pinned virtual keyboard.

mod common;

use std::time::{Duration, Instant};

use common::{Daemon, Tree};
use dettivo_insert::stand_in::{Global, HyprlandStandIn, WaylandStandIn};
use serde_json::json;

const DISPLAY: &str = "wl-0";
const GLOBALS: &[Global] = &[Global::Seat, Global::VirtualKeyboard];

fn pinned_tree() -> Tree {
    let tree = Tree::new();
    tree.write_config("[insert]\nbackend = \"virtual_keyboard\"\n");
    std::fs::create_dir_all(tree.root().join("run")).unwrap();
    tree
}

fn spawn(tree: Tree, signature: &str) -> Daemon {
    Daemon::spawn(
        tree,
        &[
            ("WAYLAND_DISPLAY", DISPLAY),
            ("HYPRLAND_INSTANCE_SIGNATURE", signature),
        ],
    )
}

fn insert(daemon: &Daemon, text: &str) -> serde_json::Value {
    daemon.result("insert.perform", json!({"mode": "raw", "text": text}))
}

fn assert_typed(result: &serde_json::Value) {
    assert_eq!(result["outcome"], "inserted", "{result}");
    assert_eq!(result["backend"]["name"], "virtual_keyboard", "{result}");
}

/// R1: the compositor restarts (both sockets die, a new instance comes up
/// on the same display) and the very next insertion types through the
/// pinned virtual keyboard within 2 s, with no daemon restart; the log
/// names the switch once.
#[test]
fn the_next_insertion_after_a_compositor_restart_types_again() {
    let tree = pinned_tree();
    let run = tree.root().join("run");
    let mut wayland = WaylandStandIn::start(&run.join(DISPLAY), GLOBALS).unwrap();
    let mut first = HyprlandStandIn::start(&run, "first", DISPLAY).unwrap();
    let daemon = spawn(tree, "first");

    assert_typed(&insert(&daemon, "hello"));
    assert_eq!(wayland.presses(), 5);
    assert!(first.queries() > 0);

    first.crash();
    wayland.crash();
    let wayland = WaylandStandIn::start(&run.join(DISPLAY), GLOBALS).unwrap();
    let second = HyprlandStandIn::start(&run, "second", DISPLAY).unwrap();
    let up = Instant::now();

    assert_typed(&insert(&daemon, "world"));
    assert!(
        up.elapsed() < Duration::from_secs(2),
        "typed {:?} after the compositor came back",
        up.elapsed()
    );
    assert_eq!(wayland.presses(), 5);
    assert!(second.queries() > 0);
    assert_typed(&insert(&daemon, "again"));

    let target = daemon.result("insert.target", json!({}));
    assert_eq!(target["compositor"]["signature"], "second", "{target}");
    assert_eq!(target["compositor"]["environment_signature"], "first");
    assert_eq!(target["compositor"]["environment_stale"], true);
    let log = daemon.stop();
    assert_eq!(
        log.matches("switched to the live instance").count(),
        1,
        "{log}"
    );
}

/// R2: a daemon whose environment names an instance that no longer
/// answers finds the live one for its display and types normally; an
/// instance of another display is never used.
#[test]
fn a_daemon_started_with_a_stale_signature_types_through_the_live_instance() {
    let tree = pinned_tree();
    let run = tree.root().join("run");
    let _wayland = WaylandStandIn::start(&run.join(DISPLAY), GLOBALS).unwrap();
    let mut stale = HyprlandStandIn::start(&run, "stale", DISPLAY).unwrap();
    stale.crash();
    let other_seat = HyprlandStandIn::start(&run, "other", "wl-9").unwrap();
    let live = HyprlandStandIn::start(&run, "live", DISPLAY).unwrap();
    let daemon = spawn(tree, "stale");

    assert_typed(&insert(&daemon, "hello"));
    assert!(live.queries() > 0);
    assert_eq!(other_seat.queries(), 0, "another display's compositor");
    let target = daemon.result("insert.target", json!({}));
    assert_eq!(target["probe"], "hyprland");
    assert_eq!(target["compositor"]["signature"], "live", "{target}");
    assert_eq!(target["compositor"]["environment_signature"], "stale");
    assert_eq!(target["compositor"]["environment_stale"], true);
    daemon.stop();
}

/// The boundary holds for the environment's instance too: a live
/// instance of another display named by the environment is never queried,
/// and the instance of the daemon's own display types the take.
#[test]
fn an_environment_naming_another_display_uses_this_displays_instance() {
    let tree = pinned_tree();
    let run = tree.root().join("run");
    let wayland = WaylandStandIn::start(&run.join(DISPLAY), GLOBALS).unwrap();
    let other_seat = HyprlandStandIn::start(&run, "other", "wl-9").unwrap();
    let live = HyprlandStandIn::start(&run, "live", DISPLAY).unwrap();
    let daemon = spawn(tree, "other");

    assert_typed(&insert(&daemon, "hello"));
    assert_eq!(wayland.presses(), 5);
    assert!(live.queries() > 0);
    assert_eq!(other_seat.queries(), 0, "another display's compositor");
    let target = daemon.result("insert.target", json!({}));
    assert_eq!(target["compositor"]["signature"], "live", "{target}");
    assert_eq!(target["compositor"]["environment_signature"], "other");
    assert_eq!(target["compositor"]["environment_stale"], true);
    daemon.stop();
}

/// A fresh environment reports its own instance, not stale.
#[test]
fn a_current_signature_is_reported_as_in_use() {
    let tree = pinned_tree();
    let run = tree.root().join("run");
    let _wayland = WaylandStandIn::start(&run.join(DISPLAY), GLOBALS).unwrap();
    let _live = HyprlandStandIn::start(&run, "live", DISPLAY).unwrap();
    let daemon = spawn(tree, "live");
    let target = daemon.result("insert.target", json!({}));
    assert_eq!(target["compositor"]["signature"], "live", "{target}");
    assert_eq!(target["compositor"]["environment_stale"], false);
    daemon.stop();
}
