//! R3, R4 and R5 against the live daemon: the portal backend on a private
//! session bus binds the four actions and a press starts a session with
//! the focused window captured, a second press is ignored, a release stops
//! it, toggle and cancel work, playing MPRIS players are paused and
//! resumed while a paused one is left alone, the cues are logged through
//! the mock audio path, a caller-supplied target that moved is refused
//! with `CONFLICT` at insertion time, and a desktop without a portal
//! leaves the daemon running with the reason in `hotkeys.status`.

mod common;

use std::time::Duration;

use common::{Daemon, Tree, local_model, tree_with_tiny, wait_for};
use dettivo_hotkeys::mock::{MockPlayer, MockPortal, PrivateBus};
use dettivo_hotkeys::portal::NO_PORTAL;
use serde_json::{Value, json};

const MOCK_ENV: &[(&str, &str)] = &[("DETTIVO_QA_MODE", "1"), ("DETTIVO_MOCK_INSERT", "1")];

fn private_bus() -> Option<PrivateBus> {
    match PrivateBus::start() {
        Ok(b) => Some(b),
        Err(e) => {
            eprintln!("skip: {e}");
            None
        }
    }
}

fn active(daemon: &Daemon) -> bool {
    daemon.result("dictation.status", json!({}))["is_active"] == Value::Bool(true)
}

fn qa_file(daemon: &Daemon, name: &str) -> String {
    std::fs::read_to_string(daemon.tree.root().join("state/dettivo/qa").join(name))
        .unwrap_or_default()
}

#[test]
fn portal_presses_drive_sessions_with_the_target_media_pause_and_cues() {
    let Some((model, wav)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let Some(bus) = private_bus() else { return };
    let portal = MockPortal::serve(bus.address(), false).unwrap();
    let playing = MockPlayer::serve(bus.address(), "playing", "Playing").unwrap();
    let paused = MockPlayer::serve(bus.address(), "paused", "Paused").unwrap();
    let tree = tree_with_tiny(
        &model,
        "mode = \"deterministic_polish\"\n[hotkeys]\nbackend = \"portal\"\npause_media = true\nsounds = true\n",
    );
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
            ("DETTIVO_MOCK_INSERT", "1"),
            ("DBUS_SESSION_BUS_ADDRESS", bus.address()),
        ],
    );
    assert!(
        wait_for(Duration::from_secs(15), || {
            daemon.result("hotkeys.status", json!({}))["backend"] == "portal"
        }),
        "portal backend never started: {}",
        daemon.log()
    );
    let status = daemon.result("hotkeys.status", json!({}));
    assert_eq!(
        status["bound"],
        json!(["push_to_talk", "toggle", "cancel", "reinsert_last"])
    );
    assert_eq!(status["portal"]["available"], true);
    assert_eq!(status["requested"], "portal");
    assert!(status["error"].is_null(), "{status}");
    let caps = daemon.result("system.capabilities", json!({}));
    assert_eq!(caps["hotkeys"]["backend"], "portal");
    assert!(
        caps["hotkeys"]["available"]
            .as_array()
            .unwrap()
            .contains(&json!("portal"))
    );

    // Press: a session with the target captured at press time.
    portal.press("push_to_talk").unwrap();
    assert!(
        wait_for(Duration::from_secs(15), || active(&daemon)),
        "{}",
        daemon.log()
    );
    let status = daemon.result("dictation.status", json!({}));
    assert_eq!(status["target"]["app_id"], "org.gnome.TextEditor");
    assert_eq!(status["target"]["pid"], 4141);
    assert_eq!(status["job"]["job_id"], "job_dict_1");
    // The press is on record for the first-run Keys step (ADR 0024).
    let pressed = daemon.result("hotkeys.status", json!({}));
    assert!(
        pressed["last_press_at"]
            .as_str()
            .is_some_and(|t| t.ends_with('Z')),
        "{pressed}"
    );
    assert!(
        wait_for(Duration::from_secs(5), || playing.status() == "Paused"),
        "playing player not paused"
    );
    assert_eq!(paused.status(), "Paused");

    // A second press while the session runs is ignored, never a second session.
    portal.press("push_to_talk").unwrap();
    assert!(wait_for(Duration::from_secs(5), || {
        daemon.log().contains("hotkey press ignored")
    }));
    assert_eq!(
        daemon.result("dictation.status", json!({}))["job"]["job_id"],
        "job_dict_1"
    );

    std::thread::sleep(Duration::from_millis(1500));
    portal.release("push_to_talk").unwrap();
    assert!(
        wait_for(Duration::from_secs(60), || !active(&daemon)),
        "{}",
        daemon.log()
    );
    assert!(
        wait_for(Duration::from_secs(5), || playing.status() == "Playing"),
        "playing player not resumed"
    );
    assert_eq!(paused.status(), "Paused", "a paused player is left alone");

    let latest = daemon.result("transcripts.latest", json!({"kind": "dictation"}));
    let item = daemon.result("transcripts.get", json!({"ref": latest["ref"]}));
    assert_eq!(item["mode"], "deterministic_polish", "{item}");

    // Toggle starts, cancel drops.
    portal.press("toggle").unwrap();
    assert!(wait_for(Duration::from_secs(15), || active(&daemon)));
    assert_eq!(
        daemon.result("dictation.status", json!({}))["job"]["job_id"],
        "job_dict_2"
    );
    portal.press("cancel").unwrap();
    assert!(wait_for(Duration::from_secs(15), || !active(&daemon)));
    assert!(
        wait_for(Duration::from_secs(5), || {
            qa_file(&daemon, "sounds.txt") == "start\nstop\nstart\nstop\n"
        }),
        "sounds: {:?}",
        qa_file(&daemon, "sounds.txt")
    );
    portal.press("toggle").unwrap();
    portal.press("cancel").unwrap();
    assert!(
        wait_for(Duration::from_secs(15), || {
            qa_file(&daemon, "sounds.txt") == "start\nstop\nstart\nstop\nstart\nstop\n"
                && !active(&daemon)
        }),
        "a queued cancel must follow the toggle start: {}",
        daemon.log()
    );
    let log = daemon.stop();
    assert!(log.contains("hotkeys: backend started"), "{log}");
    assert!(log.contains("mpris: paused"), "{log}");
}

#[test]
fn without_a_portal_the_daemon_runs_and_names_the_reason() {
    let Some(bus) = private_bus() else { return };
    let daemon = Daemon::spawn(Tree::new(), &[("DBUS_SESSION_BUS_ADDRESS", bus.address())]);
    let status = daemon.result("hotkeys.status", json!({}));
    assert_eq!(status["backend"], "none");
    assert_eq!(status["requested"], "auto");
    assert_eq!(status["portal"]["available"], false);
    assert_eq!(status["portal"]["reason"], NO_PORTAL);
    assert!(status["error"].is_null(), "{status}");
    assert_eq!(
        daemon.result("system.capabilities", json!({}))["hotkeys"]["backend"],
        "none"
    );
    daemon.stop();

    let tree = Tree::new();
    tree.write_config("[hotkeys]\nbackend = \"portal\"\n");
    let daemon = Daemon::spawn(tree, &[("DBUS_SESSION_BUS_ADDRESS", bus.address())]);
    assert!(wait_for(Duration::from_secs(10), || {
        daemon.result("hotkeys.status", json!({}))["error"].is_string()
    }));
    let status = daemon.result("hotkeys.status", json!({}));
    assert_eq!(status["error"], NO_PORTAL);
    assert_eq!(status["backend"], "none");
    assert_eq!(daemon.result("system.ping", json!({}))["ok"], true);
    daemon.stop();

    let tree = Tree::new();
    tree.write_config(
        "[hotkeys]\nbackend = \"evdev\"\nevdev_devices = [\"/nonexistent/event0\"]\n",
    );
    let daemon = Daemon::spawn(tree, &[]);
    assert!(wait_for(Duration::from_secs(10), || {
        daemon.result("hotkeys.status", json!({}))["error"].is_string()
    }));
    let status = daemon.result("hotkeys.status", json!({}));
    assert!(
        status["error"]
            .as_str()
            .unwrap()
            .contains("/nonexistent/event0"),
        "{status}"
    );
    assert_eq!(status["evdev"]["available"], false);
    daemon.stop();
}

#[test]
fn a_target_that_moved_is_refused_with_conflict_and_a_bad_pid_is_invalid_params() {
    let Some((model, wav)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing");
        return;
    };
    let tree = tree_with_tiny(&model, "[hotkeys]\nbackend = \"none\"\n");
    let mut env = MOCK_ENV.to_vec();
    env.push(("DETTIVO_MOCK_MIC", wav.to_str().unwrap()));
    let daemon = Daemon::spawn(tree, &env);
    let err = daemon.request(
        "dictation.start",
        json!({"language": "en", "mode": "raw", "expected_target_pid": "abc"}),
    );
    assert_eq!(err["error"]["data"]["app_code"], "INVALID_PARAMS");

    // The caller captured another window: the insertion is refused.
    daemon.result(
        "dictation.start",
        json!({"language": "en", "mode": "raw", "expected_target_pid": "9999", "expected_target_bundle_id": "org.kde.kate"}),
    );
    let status = daemon.result("dictation.status", json!({}));
    assert_eq!(status["target"]["pid"], 9999);
    assert_eq!(status["target"]["app_id"], "org.kde.kate");
    std::thread::sleep(Duration::from_millis(4000));
    let stopped = daemon.result("dictation.stop", json!({}));
    assert_eq!(stopped["job"]["state"], "succeeded");
    assert_eq!(stopped["insertion"]["outcome"], "failed", "{stopped}");
    assert!(
        stopped["insertion"]["reason"]
            .as_str()
            .unwrap()
            .starts_with("CONFLICT"),
        "{stopped}"
    );
    assert_eq!(qa_file(&daemon, "inserted.txt"), "");

    // The daemon's own capture matches the window at insertion time.
    daemon.result("dictation.start", json!({"language": "en", "mode": "raw"}));
    let status = daemon.result("dictation.status", json!({}));
    assert_eq!(status["target"]["app_id"], "org.gnome.TextEditor");
    assert_eq!(status["target"]["pid"], 4141);
    std::thread::sleep(Duration::from_millis(4000));
    let stopped = daemon.result("dictation.stop", json!({}));
    assert_eq!(stopped["insertion"]["outcome"], "inserted", "{stopped}");
    assert_eq!(stopped["insertion"]["backend"]["name"], "mock");
    assert!(!qa_file(&daemon, "inserted.txt").trim().is_empty());
    assert!(
        qa_file(&daemon, "sounds.txt").is_empty(),
        "sounds are off by default"
    );
    let log = daemon.stop();
    assert!(
        log.contains("the focus moved since the session started"),
        "{log}"
    );
}

/// The first-run Keys step's daemon side (ADR 0024): `hotkeys.snippet`
/// renders the same text `dettivo setup --stdout` prints for the same
/// keys, `hotkeys.setup` reports the state without writing, writes the
/// snippet once and leaves an identical file alone, sees the include
/// line once the main configuration carries it, never edits that file,
/// and an unknown compositor is `INVALID_PARAMS` naming the supported set.
#[test]
fn snippet_and_setup_match_the_cli_and_write_idempotently() {
    let daemon = Daemon::spawn(Tree::new(), &[("XDG_CURRENT_DESKTOP", "sway")]);
    let golden = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../dettivo-cli/tests/goldens/hyprland.conf"),
    )
    .unwrap();
    let snippet = daemon.result("hotkeys.snippet", json!({"compositor": "hyprland-conf"}));
    assert_eq!(snippet["text"], golden);
    assert_eq!(snippet["compositor"], "hyprland");
    assert_eq!(
        snippet["include_line"],
        "source = ~/.config/hypr/dettivo.conf"
    );
    assert!(
        snippet["path"]
            .as_str()
            .unwrap()
            .ends_with("cfg/hypr/dettivo.conf"),
        "{snippet}"
    );
    assert!(snippet["notes"].as_array().unwrap().len() >= 2);
    assert!(snippet["last_press_at"].is_null());
    assert!(daemon.result("hotkeys.status", json!({}))["last_press_at"].is_null());

    // The session's compositor (XDG_CURRENT_DESKTOP) when none is named.
    let detected = daemon.result("hotkeys.snippet", json!({}));
    assert_eq!(detected["compositor"], "sway");
    assert!(
        detected["text"]
            .as_str()
            .unwrap()
            .contains("bindsym F9 exec")
    );

    let check = daemon.result(
        "hotkeys.setup",
        json!({"compositor": "sway", "write": false}),
    );
    assert_eq!(check["written"], false);
    assert_eq!(check["sourced"], false);
    let path = std::path::PathBuf::from(check["path"].as_str().unwrap());
    let main = std::path::PathBuf::from(check["main_config"]["path"].as_str().unwrap());
    assert_eq!(check["main_config"]["exists"], false);
    assert!(!path.exists());
    assert_eq!(check["include_line"], "include ~/.config/sway/dettivo");

    let written = daemon.result(
        "hotkeys.setup",
        json!({"compositor": "sway", "write": true}),
    );
    assert_eq!(written["written"], true);
    assert_eq!(written["sourced"], false);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), detected["text"]);
    let first = std::fs::metadata(&path).unwrap().modified().unwrap();
    std::thread::sleep(Duration::from_millis(20));
    let again = daemon.result(
        "hotkeys.setup",
        json!({"compositor": "sway", "write": true}),
    );
    assert_eq!(again["written"], true);
    assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), first);
    assert!(!main.exists(), "the main configuration is never written");

    std::fs::write(&main, "# mine\ninclude ~/.config/sway/dettivo\n").unwrap();
    let sourced = daemon.result(
        "hotkeys.setup",
        json!({"compositor": "sway", "write": false}),
    );
    assert_eq!(sourced["sourced"], true);
    assert_eq!(sourced["main_config"]["exists"], true);
    assert_eq!(
        std::fs::read_to_string(&main).unwrap(),
        "# mine\ninclude ~/.config/sway/dettivo\n"
    );

    let refused = daemon.request("hotkeys.snippet", json!({"compositor": "gnome"}));
    assert_eq!(refused["error"]["data"]["app_code"], "INVALID_PARAMS");
    assert_eq!(
        refused["error"]["message"],
        "unknown compositor \"gnome\"; supported: hyprland, sway, niri"
    );
    let refused = daemon.request(
        "hotkeys.setup",
        json!({"compositor": "gnome", "write": true}),
    );
    assert_eq!(refused["error"]["data"]["app_code"], "INVALID_PARAMS");
    daemon.stop();
}
