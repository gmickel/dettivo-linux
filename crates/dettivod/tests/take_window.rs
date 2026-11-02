//! fn-81 against the live daemon (ADR 0092, ADR 0093): with `[audio]
//! keep_microphone_ready` on, a held take starts 450 ms before its press
//! and keeps 200 ms after its release; a tap reaches no engine, plays no
//! stop cue and stores nothing; a meeting start closes the ring before the
//! meeting's capture opens and the ring reopens after that capture ended.
//! The mock microphone stands in for PipeWire. Skipped without tiny.en.

mod common;

use std::time::{Duration, Instant};

use common::{Daemon, Tree, local_model, tree_with_tiny, wait_for};
use serde_json::{Value, json};

const READY: &str =
    "[hotkeys]\nbackend = \"none\"\nsounds = true\n[audio]\nkeep_microphone_ready = true\n";

fn ready_state(daemon: &Daemon) -> Value {
    daemon.result("audio.devices", json!({}))["microphone_ready"].clone()
}

/// The ring's state and reason; its lock signal depends on the host.
fn state_and_reason(daemon: &Daemon) -> (Option<String>, Option<String>) {
    let ready = ready_state(daemon);
    let text = |key: &str| ready[key].as_str().map(str::to_string);
    (text("state"), text("reason"))
}

/// How long the engine may take on a take or a meeting's live tail: a
/// debug engine on a loaded runner needs seconds where a release build
/// needs milliseconds, and a hang still fails well inside the job timeout.
const ENGINE: Duration = Duration::from_secs(60);

/// The daemon's log once it holds `needle`. A line the daemon wrote before
/// it answered still has to cross the stderr pipe to the test's reader.
fn logged(daemon: &Daemon, needle: &str, budget: Duration) -> String {
    assert!(
        wait_for(budget, || daemon.log().contains(needle)),
        "{needle} never logged:\n{}",
        daemon.log()
    );
    daemon.log()
}

fn idle(daemon: &Daemon) -> bool {
    daemon.result("dictation.status", json!({}))["is_active"] == false
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
    let finished = daemon.log().matches("dictation finished").count();

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
    // The release answers once the take stopped; the engine transcribes
    // it after that, and the take is stored and logged before it idles.
    assert!(
        wait_for(ENGINE, || idle(&daemon)),
        "the held take never ended"
    );
    assert!(
        wait_for(Duration::from_secs(10), || daemon
            .log()
            .matches("dictation finished")
            .count()
            > finished),
        "the held take was not logged:\n{}",
        daemon.log()
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
    assert!(wait_for(Duration::from_secs(10), || idle(&daemon)));
    // The tap ends with reason tap.
    let log = logged(&daemon, "reason=\"tap\"", Duration::from_secs(10));
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
        state_and_reason(&daemon),
        (Some("closed".into()), Some("a meeting is capturing".into()))
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

/// Ten seconds of 16 kHz mono silence for the mock microphone.
fn silent_wav(path: &std::path::Path) {
    let data: u32 = 16_000 * 2 * 10;
    let mut wav = b"RIFF".to_vec();
    wav.extend((36 + data).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    for (value, width) in [
        (16, 4),
        (1, 2),
        (1, 2),
        (16_000, 4),
        (32_000, 4),
        (2, 2),
        (16, 2),
    ] {
        wav.extend(&u32::to_le_bytes(value)[..width]);
    }
    wav.extend(b"data");
    wav.extend(data.to_le_bytes());
    wav.resize(wav.len() + data as usize, 0);
    std::fs::write(path, wav).unwrap();
}

/// QA81-2 (R5 "pause"): `dictation.pause` closes the always-ready
/// microphone before it answers and keeps it closed until
/// `dictation.resume`; both are idempotent.
#[test]
fn pause_closes_the_ready_microphone_until_resume() {
    let tree = Tree::new();
    let wav = tree.root().join("silence.wav");
    silent_wav(&wav);
    tree.write_config(READY);
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
        ],
    );
    wait_ready(&daemon, "open");
    for _ in 0..2 {
        assert_eq!(
            daemon.result("dictation.pause", json!({})),
            json!({"paused": true})
        );
        assert_eq!(
            state_and_reason(&daemon),
            (
                Some("closed".into()),
                Some("paused until dettivo dictation resume".into())
            )
        );
    }
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(
        ready_state(&daemon)["state"],
        "closed",
        "no reopen while paused"
    );
    for _ in 0..2 {
        assert_eq!(
            daemon.result("dictation.resume", json!({})),
            json!({"paused": false})
        );
    }
    wait_ready(&daemon, "open");
}
