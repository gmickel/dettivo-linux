//! fn-81 against the live daemon (ADR 0092, ADR 0093): with `[audio]
//! keep_microphone_ready` on, a held take starts 450 ms before its press
//! and keeps 200 ms after its release; a tap reaches no engine, plays no
//! stop cue and stores nothing; a meeting start closes the ring before the
//! meeting's capture opens and the ring reopens after that capture ended.
//! The mock microphone stands in for PipeWire. Skipped without tiny.en.

mod common;

use std::time::{Duration, Instant};

use common::{Daemon, local_model, tree_with_tiny, wait_for};
use serde_json::{Value, json};

const READY: &str =
    "[hotkeys]\nbackend = \"none\"\nsounds = true\n[audio]\nkeep_microphone_ready = true\n";

fn ready_state(daemon: &Daemon) -> Value {
    daemon.result("audio.devices", json!({}))["microphone_ready"].clone()
}

fn wait_ready(daemon: &Daemon, state: &str) {
    assert!(
        wait_for(Duration::from_secs(10), || ready_state(daemon)["state"]
            == state),
        "microphone ready never {state}: {}",
        ready_state(daemon)
    );
}

/// The `duration_ms` of each finished dictation, from the log.
fn take_lengths(log: &str) -> Vec<u64> {
    log.lines()
        .filter(|l| l.contains("dictation finished"))
        .filter_map(|l| l.split("duration_ms=").nth(1))
        .filter_map(|rest| {
            rest.split(|c: char| !c.is_ascii_digit())
                .next()?
                .parse()
                .ok()
        })
        .collect()
}

fn dictations(daemon: &Daemon) -> usize {
    daemon.result(
        "transcripts.list",
        json!({"kinds": ["dictation"], "limit": 50, "cursor": null}),
    )["items"]
        .as_array()
        .map_or(0, Vec::len)
}

#[test]
fn a_held_take_spans_the_pre_roll_and_the_tail_and_a_tap_is_dropped() {
    let Some((model, wav)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let daemon = Daemon::spawn(
        tree_with_tiny(&model, READY),
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
            ("DETTIVO_MOCK_INSERT", "1"),
        ],
    );
    wait_ready(&daemon, "open");
    // A second of speech in the ring before the press.
    std::thread::sleep(Duration::from_millis(1_000));
    let before = dictations(&daemon);

    // The press answers once the take started (a cold engine takes a
    // while); the take counts from when the press arrived.
    let pressed = Instant::now();
    assert_eq!(
        daemon.result("dictation.press", json!({}))["action"],
        "started"
    );
    std::thread::sleep(Duration::from_millis(500));
    let held = pressed.elapsed().as_millis() as u64;
    assert_eq!(
        daemon.result("dictation.release", json!({}))["action"],
        "stopped"
    );
    let lengths = take_lengths(&daemon.log());
    let length = *lengths.last().expect("a finished take");
    // 450 ms before the press, the hold, 200 ms after the release.
    assert!(
        (held + 600..=held + 720).contains(&length),
        "{length} ms for a {held} ms hold: {lengths:?}"
    );
    assert!(
        daemon
            .log()
            .contains("dictation starts from the always-ready microphone")
    );
    assert_eq!(dictations(&daemon), before + 1);

    // A tap: dropped before the engine, no stop cue, nothing stored.
    let steps = daemon.log().matches("dictation text step").count();
    assert_eq!(
        daemon.result("dictation.press", json!({}))["action"],
        "started"
    );
    assert_eq!(
        daemon.result("dictation.release", json!({}))["action"],
        "discarded"
    );
    assert!(wait_for(Duration::from_secs(5), || daemon
        .result("dictation.status", json!({}))["is_active"]
        == false));
    let log = daemon.log();
    assert!(
        log.contains("reason=\"tap\""),
        "the tap ends with reason tap"
    );
    assert_eq!(log.matches("dictation text step").count(), steps);
    assert_eq!(dictations(&daemon), before + 1);
    let sounds = std::fs::read_to_string(daemon.tree.root().join("state/dettivo/qa/sounds.txt"))
        .unwrap_or_default();
    assert_eq!(sounds, "start\nstop\nstart\n", "no stop cue for the tap");
    wait_ready(&daemon, "open");
}

/// R4a: the ring closes before the meeting's capture opens and reopens
/// after that capture ended.
#[test]
fn a_meeting_takes_the_microphone_over_from_the_ring_and_hands_it_back() {
    use common::meetings::{env, spawn, start_params, track_fixture, tree};
    let Some(tree) = tree(READY) else { return };
    let mic = track_fixture("mic");
    let daemon = spawn(tree, &env(&mic, &mic, &[]));
    wait_ready(&daemon, "open");
    let started = daemon.result("meetings.start", start_params("Handoff"));
    let id = started["ref"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        ready_state(&daemon),
        json!({"state": "closed", "reason": "a meeting is capturing"})
    );
    std::thread::sleep(Duration::from_millis(1_000));
    assert_eq!(
        ready_state(&daemon)["state"],
        "closed",
        "no reopen mid-meeting"
    );
    daemon.result("meetings.stop", json!({"meeting_id": id}));
    wait_ready(&daemon, "open");

    let log = daemon.log();
    let at = |needle: &str| {
        log.find(needle)
            .unwrap_or_else(|| panic!("{needle} not logged"))
    };
    let closed = at("microphone ready: closed why=\"a meeting is capturing\"");
    let recording = at("state=\"recording\" previous=\"idle\"");
    let ended = at("meeting capture ended");
    let reopened = log.rfind("microphone ready: open").unwrap();
    assert!(closed < recording, "closed before the meeting recorded");
    assert!(
        ended < reopened,
        "reopened after the meeting's capture ended"
    );
}
