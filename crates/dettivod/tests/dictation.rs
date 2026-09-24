//! R2 to R6 against a live daemon with the mock microphone and the local
//! tiny.en model: a whole dictation returns the fixture's words, the event
//! stream carries the exact state sequence plus levels and engine states,
//! a subscriber that stops reading gets `events.overflow`, concurrent starts
//! let one win, the policy is frozen at start, and the errors are the
//! contract's.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use common::subscriber::Subscriber;
use common::{Daemon, Tree, local_model};
use serde_json::{Value, json};

fn tree_with_tiny(model: &Path) -> Tree {
    common::tree_with_tiny(model, "")
}

#[test]
fn a_dictation_returns_the_fixture_words_and_the_stream_shows_every_state() {
    let Some((model, wav)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let tree = tree_with_tiny(&model);
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
        ],
    );
    let mut sub = Subscriber::open(
        &daemon,
        &["dictation.state", "audio.level", "engine.state"],
        256,
    );

    let status = daemon.result("dictation.status", json!({}));
    assert_eq!(status["is_active"], false);
    let err = daemon.request("dictation.stop", json!({}));
    assert_eq!(err["error"]["data"]["app_code"], "NOT_FOUND");
    // A mode identifier the contract does not carry is refused by name.
    let err = daemon.request(
        "dictation.start",
        json!({"language": "en", "mode": "meeting"}),
    );
    assert_eq!(err["error"]["data"]["app_code"], "INVALID_PARAMS");

    let started = daemon.result("dictation.start", json!({"language": "en", "mode": "raw"}));
    assert_eq!(started["job"]["state"], "running");
    assert_eq!(started["job"]["job_id"], "job_dict_1");
    let conflict = daemon.request("dictation.start", json!({"language": "en", "mode": "raw"}));
    assert_eq!(conflict["error"]["data"]["app_code"], "CONFLICT");
    assert_eq!(
        conflict["error"]["data"]["details"]["kind"],
        "sessionActive"
    );
    let health = daemon.result("system.health", json!({}));
    assert_eq!(health["recording_state"], "dictation");
    assert_eq!(health["active_jobs"], 1);
    let status = daemon.result("dictation.status", json!({}));
    assert_eq!(status["is_active"], true);
    assert!(
        status["job"]["message"]
            .as_str()
            .unwrap()
            .contains("whisper/tiny.en")
    );

    // The policy is frozen: a model change now applies to the next session.
    daemon.result(
        "config.set",
        json!({"key": "speech.model", "value": "large-v3-turbo"}),
    );
    let status = daemon.result("dictation.status", json!({}));
    assert!(
        status["job"]["message"]
            .as_str()
            .unwrap()
            .contains("whisper/tiny.en")
    );

    // The fixture is 11 s; take most of it.
    std::thread::sleep(Duration::from_millis(6500));
    let stopped = daemon.result("dictation.stop", json!({}));
    assert_eq!(stopped["ref"]["kind"], "dictation");
    assert_eq!(stopped["ref"]["id"].as_str().unwrap().len(), 36);
    assert_eq!(stopped["job"]["state"], "succeeded");
    let health = daemon.result("system.health", json!({}));
    assert_eq!(health["recording_state"], "idle");

    let reinserted = daemon.result("dictation.reinsert_last", json!({}));
    assert_eq!(reinserted["ref"]["id"], stopped["ref"]["id"]);
    assert!(matches!(
        reinserted["insertion"]["outcome"].as_str(),
        Some("copied_to_clipboard" | "failed")
    ));

    let events = sub.collect(
        |p| p["topic"] == "dictation.state" && p["payload"]["state"] == "idle",
        Duration::from_secs(20),
    );
    let states: Vec<&str> = events
        .iter()
        .filter(|p| p["topic"] == "dictation.state")
        .map(|p| p["payload"]["state"].as_str().unwrap())
        .collect();
    assert_eq!(
        states,
        ["recording", "transcribing", "inserting", "idle"],
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|p| p["topic"] == "audio.level" && p["payload"]["source"] == "microphone")
    );
    assert!(
        events
            .iter()
            .any(|p| p["topic"] == "engine.state" && p["payload"]["state"] == "loaded"),
        "{events:?}"
    );
    // The completion transition carries the first sentence for the pill
    // (docs/api/linux-deltas.md) and the insertion outcome; nothing else
    // on the stream carries transcript text.
    let completion = events
        .iter()
        .find(|p| p["topic"] == "dictation.state" && p["payload"]["previous_state"] == "inserting")
        .expect("completion transition");
    let first_words = completion["payload"]["first_words"].as_str().unwrap();
    assert!(
        first_words.starts_with("And so my fellow Americans"),
        "{first_words}"
    );
    assert!(first_words.chars().count() <= 72);
    assert!(completion["payload"]["insertion"]["outcome"].is_string());
    assert!(completion["payload"]["insertion"]["backend"].is_string());
    // The Linux `timings` block splits the path after the stop; it rides on
    // the completion transition only.
    let timings = &completion["payload"]["timings"];
    for key in ["capture_ms", "transcribe_ms", "insert_ms"] {
        assert!(timings[key].is_u64(), "timings.{key}: {completion}");
    }
    assert!(timings["transcribe_ms"].as_u64().unwrap() > 0);
    assert!(
        events
            .iter()
            .filter(|p| p["topic"] == "dictation.state" && p["payload"]["state"] != "idle")
            .all(|p| p["payload"].get("timings").is_none()),
        "timings on a non-completion transition: {events:?}"
    );
    for p in &events {
        assert!(p["timestamp"].as_str().unwrap().ends_with('Z'));
        let mut scrubbed = p.clone();
        if let Some(payload) = scrubbed["payload"].as_object_mut() {
            payload.remove("first_words");
        }
        let text = scrubbed.to_string().to_lowercase();
        assert!(
            !text.contains("americans"),
            "transcript leaked into the stream: {p}"
        );
    }

    let sessions = daemon.tree.root().join("state/dettivo/sessions/job_dict_1");
    assert!(!sessions.join("transcript.json").exists());
    let transcript = daemon.result("transcripts.get", json!({"ref": stopped["ref"]}));
    let text = transcript["text_polish"].as_str().unwrap().to_lowercase();
    assert!(
        text.contains("fellow americans") && text.contains("country"),
        "{text}"
    );
    assert!(
        transcript["text_polish"].as_str().unwrap().contains("ASK"),
        "replacement applied: {}",
        transcript["text_polish"]
    );
    assert!(
        !sessions.join("microphone.wav").exists(),
        "take discarded by default"
    );

    let log = daemon.stop();
    assert!(log.contains("dictation started"), "{log}");
    assert!(
        !log.to_lowercase().contains("americans"),
        "transcript leaked into the log"
    );
}

#[test]
fn a_subscriber_that_stops_reading_overflows_while_another_gets_everything() {
    let Some((model, wav)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing");
        return;
    };
    let tree = tree_with_tiny(&model);
    // A fast meter floods the stream for nine seconds, more than the
    // connection's 256-line channel and its bounded socket buffer hold
    // while the lazy reader sleeps.
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
        ],
    );
    daemon.result(
        "config.set",
        json!({"key": "audio.level_interval_ms", "value": "10"}),
    );
    let mut lazy = Subscriber::open(&daemon, &["audio.level", "dictation.state"], 256);
    let mut eager = Subscriber::open(&daemon, &["dictation.state"], 256);
    daemon.result("dictation.start", json!({"language": "en", "mode": "raw"}));
    std::thread::sleep(Duration::from_millis(9000));
    daemon.result("dictation.cancel", json!({}));
    let eager_events = eager.collect(|p| p["payload"]["state"] == "idle", Duration::from_secs(10));
    let eager_states: Vec<&str> = eager_events
        .iter()
        .map(|p| p["payload"]["state"].as_str().unwrap())
        .collect();
    assert_eq!(
        eager_states,
        ["recording", "cancelled", "idle"],
        "{eager_events:?}"
    );
    // The lazy reader now drains: it must see an overflow with a count.
    let lazy_events = lazy.collect(|p| p["topic"] == "events.overflow", Duration::from_secs(10));
    let overflow = lazy_events.iter().find(|p| p["topic"] == "events.overflow");
    assert!(
        overflow.is_some_and(|o| o["payload"]["dropped"].as_u64().unwrap() > 0),
        "{}",
        lazy_events.len()
    );
    daemon.stop();
}

#[test]
fn concurrent_starts_over_two_connections_let_one_win_and_a_missing_model_is_named() {
    let Some((model, wav)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing");
        return;
    };
    let tree = tree_with_tiny(&model);
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
        ],
    );
    let socket = daemon.tree.socket();
    let mut handles = Vec::new();
    for _ in 0..3 {
        let socket = socket.clone();
        handles.push(std::thread::spawn(move || {
            let mut s = UnixStream::connect(socket).unwrap();
            s.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":\"c\",\"method\":\"dictation.start\",\"params\":{\"language\":\"en\",\"mode\":\"raw\"}}\n").unwrap();
            let mut line = String::new();
            BufReader::new(s).read_line(&mut line).unwrap();
            let v: Value = serde_json::from_str(line.trim_end()).unwrap();
            v.get("result").is_some()
        }));
    }
    let wins: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(wins.iter().filter(|w| **w).count(), 1, "{wins:?}");
    daemon.result("dictation.cancel", json!({}));
    let err = daemon.request("dictation.cancel", json!({}));
    assert_eq!(err["error"]["data"]["app_code"], "NOT_FOUND");

    daemon.result(
        "config.set",
        json!({"key": "speech.model", "value": "base"}),
    );
    let err = daemon.request("dictation.start", json!({"language": "en", "mode": "raw"}));
    assert_eq!(err["error"]["data"]["app_code"], "NOT_FOUND");
    let message = err["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("whisper/base")
            && message.contains("dettivo speech download --model base"),
        "{message}"
    );
    let err = daemon.request(
        "events.subscribe",
        json!({"topics": ["bogus.topic"], "buffer": 256}),
    );
    assert_eq!(err["error"]["data"]["app_code"], "INVALID_PARAMS");
    let sub = daemon.result(
        "events.subscribe",
        json!({"topics": ["dictation.state"], "buffer": 256}),
    );
    let unsub = daemon.result(
        "events.unsubscribe",
        json!({"subscription_id": sub["subscription_id"]}),
    );
    // The subscription lived on the request's own connection, which the
    // helper closed; so it was already gone when unsubscribe arrived.
    assert!(unsub["unsubscribed"].is_boolean());
    daemon.stop();
}

/// R4: a dictation in `deterministic_polish` inserts the Polish text, the
/// same dictation in `enhanced` with `DETTIVO_MOCK_LLM=echo` inserts it
/// with the notice, and the completion event and the history item carry
/// the mode, the notice and the policy hash.
#[test]
fn the_polish_modes_insert_the_polished_text_and_record_the_policy() {
    let Some((model, wav)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let tree = tree_with_tiny(&model);
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
            // The Enhanced pass reaches the QA mock instead of a model, so
            // the run is deterministic and never touches the network.
            ("DETTIVO_MOCK_LLM", "echo"),
        ],
    );
    for mode in ["deterministic_polish", "enhanced"] {
        let mut sub = Subscriber::open(&daemon, &["dictation.state"], 256);
        let started = daemon.result("dictation.start", json!({"language": "en", "mode": mode}));
        assert_eq!(started["job"]["state"], "running", "{mode}");
        std::thread::sleep(Duration::from_millis(3000));
        let stopped = daemon.result("dictation.stop", json!({}));
        assert_eq!(stopped["job"]["state"], "succeeded", "{mode}");
        let events = sub.collect(
            |p| p["topic"] == "dictation.state" && p["payload"]["state"] == "idle",
            Duration::from_secs(20),
        );
        let completion = events
            .iter()
            .find(|p| p["payload"]["previous_state"] == "inserting")
            .unwrap_or_else(|| panic!("{mode}: no completion transition in {events:?}"));
        let payload = &completion["payload"];
        assert_eq!(payload["mode"], mode, "{payload}");
        let hash = payload["policy_hash"].as_str().unwrap_or_default();
        assert_eq!(hash.len(), 8, "{payload}");
        // The echo mock hands back exactly the Polish text, so there is
        // no fallback to notice (R2).
        assert!(payload["notice"].is_null(), "{payload}");
        let item = daemon.result(
            "transcripts.get",
            json!({"ref": {"kind": "dictation", "id": stopped["ref"]["id"]}}),
        );
        assert_eq!(item["mode"], mode, "{item}");
        assert_eq!(item["policy_hash"], hash, "{item}");
        // The Polish pass always hands back a finished sentence: a
        // capital at the front and terminal punctuation at the end.
        let polished = item["text_polish"].as_str().unwrap();
        assert!(polished.contains("fellow Americans"), "{item}");
        assert!(
            polished.chars().next().is_some_and(char::is_uppercase),
            "{item}"
        );
        assert!(
            polished.ends_with('.') || polished.ends_with('?') || polished.ends_with('!'),
            "{item}"
        );
    }
    // A start that names no mode and no language runs on the configured
    // ones, as the hotkey path does, so a screen that shows the
    // configured mode starts that mode.
    daemon.result(
        "config.set",
        json!({"key": "dictation.mode", "value": "enhanced"}),
    );
    let mut sub = Subscriber::open(&daemon, &["dictation.state"], 256);
    let started = daemon.result("dictation.start", json!({}));
    assert_eq!(started["job"]["state"], "running");
    std::thread::sleep(Duration::from_millis(3000));
    daemon.result("dictation.stop", json!({}));
    let events = sub.collect(
        |p| p["topic"] == "dictation.state" && p["payload"]["state"] == "idle",
        Duration::from_secs(20),
    );
    let completion = events
        .iter()
        .find(|p| p["payload"]["previous_state"] == "inserting")
        .unwrap_or_else(|| panic!("no completion transition in {events:?}"));
    assert_eq!(
        completion["payload"]["mode"], "enhanced",
        "the configured mode ran: {}",
        completion["payload"]
    );
    daemon.stop();
}

/// R4 (error branch): `enhanced` with a provider that never answers
/// inserts the Polish text and says why, inside the time budget.
#[test]
fn enhanced_without_an_answer_inserts_the_polish_text_and_says_why() {
    let Some((model, wav)) = local_model() else {
        eprintln!("skip: tiny.en or jfk.wav missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let fixtures = tempfile::tempdir().unwrap();
    std::fs::write(fixtures.path().join("default.txt"), "__TIMEOUT__").unwrap();
    let tree = tree_with_tiny(&model);
    let daemon = Daemon::spawn(
        tree,
        &[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
            (
                "DETTIVO_MOCK_LLM",
                Box::leak(format!("fixture:{}", fixtures.path().display()).into_boxed_str()),
            ),
        ],
    );
    let mut sub = Subscriber::open(&daemon, &["dictation.state"], 256);
    daemon.result(
        "dictation.start",
        json!({"language": "en", "mode": "enhanced"}),
    );
    std::thread::sleep(Duration::from_millis(3000));
    let stopped = daemon.result("dictation.stop", json!({}));
    assert_eq!(stopped["job"]["state"], "succeeded");
    let events = sub.collect(
        |p| p["topic"] == "dictation.state" && p["payload"]["state"] == "idle",
        Duration::from_secs(20),
    );
    let payload = &events
        .iter()
        .find(|p| p["payload"]["previous_state"] == "inserting")
        .expect("completion transition")["payload"];
    assert_eq!(payload["mode"], "enhanced", "{payload}");
    assert_eq!(
        payload["notice"]["kind"], "provider_unavailable",
        "{payload}"
    );
    assert!(
        payload["notice"]["reason"]
            .as_str()
            .is_some_and(|r| !r.is_empty()),
        "{payload}"
    );
    let item = daemon.result(
        "transcripts.get",
        json!({"ref": {"kind": "dictation", "id": stopped["ref"]["id"]}}),
    );
    assert_eq!(item["notice"]["kind"], "provider_unavailable", "{item}");
    assert_eq!(item["mode"], "enhanced", "{item}");
    daemon.stop();
}
