//! fn-73 R1, R2, R4 and R10 against a live daemon with the mock tracks and
//! the local tiny.en: a client polling `meetings.segments` with its
//! cursor from the start of the recording to the completed row reads
//! every live final once and in order, then one reset to the stored
//! transcript; and `dettivo meetings segments --follow --json` started
//! mid-meeting prints the backlog it attached late to, then streams
//! without a gap or a duplicate. Skipped without the model.

mod common;

use std::collections::HashSet;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use common::meetings::{env, spawn, start_params, track_fixture, tree};
use common::{DAEMON, Daemon};
use serde_json::{Value, json};

/// What the polling client collected.
#[derive(Default)]
struct Poller {
    cursor: Option<String>,
    live: Vec<Value>,
    stored: Vec<Value>,
    resets: u32,
    seen: Vec<(String, String)>,
}

impl Poller {
    /// One poll; true once the completed meeting's stored transcript
    /// answered.
    fn poll(&mut self, daemon: &Daemon, id: &str) -> bool {
        let mut params = json!({"meeting_id": id});
        if let Some(c) = &self.cursor {
            params["since"] = json!(c);
        }
        let r = daemon.result("meetings.segments", params);
        let transcript = r["transcript"].as_str().unwrap().to_string();
        let status = r["status"].as_str().unwrap().to_string();
        for p in r["provisional"].as_array().unwrap() {
            assert_eq!(p["provisional"], true, "{r}");
            assert!(p["source_type"].is_string(), "{r}");
        }
        let into = if transcript == "live" {
            assert_eq!(r["reset"], false, "{r}");
            assert!(self.stored.is_empty(), "live after stored: {:?}", self.seen);
            &mut self.live
        } else {
            if r["reset"] == true {
                self.resets += 1;
                self.stored.clear();
            }
            &mut self.stored
        };
        for s in r["segments"].as_array().unwrap() {
            assert_eq!(s["provisional"], false, "{r}");
            assert!(s["source_type"].is_string(), "{r}");
            assert_eq!(
                s["index"],
                json!(into.len()),
                "no final skipped or repeated: {r}"
            );
            into.push(s.clone());
        }
        self.cursor = Some(r["cursor"].as_str().unwrap().to_string());
        let done = status == "completed" && transcript == "stored";
        self.seen.push((status, transcript));
        done
    }
}

/// The CLI binary beside the daemon, built when this test runs alone.
fn cli() -> std::path::PathBuf {
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["build", "-q", "-p", "dettivo-cli"])
        .status()
        .expect("cargo build");
    assert!(status.success());
    std::path::Path::new(DAEMON).with_file_name("dettivo")
}

#[test]
fn a_poller_and_a_late_follower_read_the_whole_meeting_without_a_gap_or_a_duplicate() {
    let Some(tree) = tree("") else { return };
    let cli = cli();
    let mic = track_fixture("segments");
    let daemon = spawn(tree, &env(&mic, &mic, &[]));
    let started = daemon.result("meetings.start", start_params("Live copilot"));
    let id = started["ref"]["id"].as_str().unwrap().to_string();
    let root = daemon.tree.root();
    let attach = AtomicBool::new(false);
    let (poller, follower) = std::thread::scope(|scope| {
        let polling = scope.spawn(|| {
            let mut poller = Poller::default();
            let deadline = Instant::now() + Duration::from_secs(180);
            while !poller.poll(&daemon, &id) {
                assert!(Instant::now() < deadline, "{:?}", poller.seen);
                // The first live final lets the follower attach late.
                if !poller.live.is_empty() {
                    attach.store(true, Ordering::SeqCst);
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            poller
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        while !attach.load(Ordering::SeqCst) {
            assert!(Instant::now() < deadline, "no live final within 30 s");
            std::thread::sleep(Duration::from_millis(50));
        }
        let child = Command::new(&cli)
            .args(["--json", "meetings", "segments", &id, "--follow"])
            .env_remove("DETTIVO_IPC_SOCKET")
            .env_remove("DETTIVO_IPC_TOKEN")
            .env_remove("DETTIVO_CONFIG")
            .env("HOME", &root)
            .env("XDG_CONFIG_HOME", root.join("cfg"))
            .env("XDG_STATE_HOME", root.join("state"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_RUNTIME_DIR", root.join("run"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // Let the live path run on while the follower streams.
        std::thread::sleep(Duration::from_secs(4));
        daemon.result("meetings.stop", json!({"meeting_id": id}));
        (polling.join().unwrap(), child.wait_with_output().unwrap())
    });
    assert!(
        follower.status.success(),
        "{}",
        String::from_utf8_lossy(&follower.stderr)
    );

    // The poller: every status of the lifecycle, live until the stored
    // row replaced it, then exactly one reset to the stored transcript.
    let statuses: HashSet<&str> = poller.seen.iter().map(|(s, _)| s.as_str()).collect();
    for wanted in ["recording", "transcribing", "completed"] {
        assert!(statuses.contains(wanted), "{wanted}: {:?}", poller.seen);
    }
    assert_eq!(poller.resets, 1, "{:?}", poller.seen);
    let ids: HashSet<&str> = poller
        .live
        .iter()
        .map(|s| s["segment_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), poller.live.len(), "a live final repeated");
    let got = daemon.result("meetings.get", json!({"meeting_id": id}));
    let stored: Vec<(Value, Value, Value)> = got["segments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["text"].clone(),
                s["start_ms"].clone(),
                s["source_type"].clone(),
            )
        })
        .collect();
    let polled: Vec<(Value, Value, Value)> = poller
        .stored
        .iter()
        .map(|s| {
            (
                s["text"].clone(),
                s["start_ms"].clone(),
                s["source_type"].clone(),
            )
        })
        .collect();
    assert_eq!(polled, stored);

    // The follower: the backlog it attached late to, then the stream,
    // then the stored transcript; every live final once.
    let out = String::from_utf8_lossy(&follower.stdout);
    let lines: Vec<Value> = out
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let backlog = &lines[0];
    assert_eq!(backlog["transcript"], "live", "{backlog}");
    assert!(
        !backlog["segments"].as_array().unwrap().is_empty(),
        "attached late, it still reads what was said before: {backlog}"
    );
    assert_eq!(lines.last().unwrap()["transcript"], "stored");
    let mut finals: Vec<&str> = backlog["segments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["segment_id"].as_str().unwrap())
        .collect();
    for event in &lines[1..lines.len() - 1] {
        assert!(event["source_type"].is_string(), "{event}");
        if event["provisional"] == false {
            finals.push(event["segment_id"].as_str().unwrap());
        }
    }
    let unique: HashSet<&str> = finals.iter().copied().collect();
    assert_eq!(unique.len(), finals.len(), "a duplicate: {finals:?}");
    assert_eq!(unique, ids, "the follower saw every final the poller did");
    daemon.stop();
}
