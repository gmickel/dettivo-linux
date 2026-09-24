//! daemon/F6: a dictation start and a meeting start racing over two
//! connections never both pass their exclusion checks; one wins, the other
//! answers `CONFLICT`, and only one session is active afterwards. Skipped
//! without the local tiny.en.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Barrier};

use common::meetings::{env, spawn, start_params, track_fixture, tree};
use serde_json::{Value, json};

fn fire(
    socket: std::path::PathBuf,
    line: String,
    go: Arc<Barrier>,
) -> std::thread::JoinHandle<Value> {
    std::thread::spawn(move || {
        let mut s = UnixStream::connect(socket).unwrap();
        go.wait();
        s.write_all(line.as_bytes()).unwrap();
        s.write_all(b"\n").unwrap();
        let mut answer = String::new();
        BufReader::new(s).read_line(&mut answer).unwrap();
        serde_json::from_str(answer.trim_end()).unwrap()
    })
}

#[test]
fn a_dictation_start_and_a_meeting_start_racing_let_exactly_one_win() {
    let Some(tree) = tree("") else { return };
    let mic = track_fixture("mic");
    let daemon = spawn(tree, &env(&mic, &mic, &[]));
    let socket = daemon.tree.socket();
    let dictation = json!({"jsonrpc":"2.0","id":"d","method":"dictation.start","params":{"language":"en","mode":"raw"}}).to_string();
    let meeting =
        json!({"jsonrpc":"2.0","id":"m","method":"meetings.start","params":start_params("Raced")})
            .to_string();
    for round in 0..8 {
        let go = Arc::new(Barrier::new(2));
        let a = fire(socket.clone(), dictation.clone(), go.clone());
        let b = fire(socket.clone(), meeting.clone(), go);
        let (a, b) = (a.join().unwrap(), b.join().unwrap());
        let dictation_won = a.get("result").is_some();
        let meeting_won = b.get("result").is_some();
        assert!(
            dictation_won ^ meeting_won,
            "round {round}: dictation {a}, meeting {b}"
        );
        let loser = if dictation_won { &b } else { &a };
        assert_eq!(
            loser["error"]["data"]["app_code"], "CONFLICT",
            "round {round}: {loser}"
        );
        let dictation_active =
            daemon.result("dictation.status", json!({}))["is_active"] == json!(true);
        assert_eq!(
            dictation_active, dictation_won,
            "round {round}: the dictation session follows its start"
        );
        if dictation_won {
            daemon.result("dictation.cancel", json!({}));
        } else {
            let id = b["result"]["ref"]["id"].as_str().unwrap().to_string();
            daemon.result("meetings.cancel", json!({"meeting_id": id}));
        }
    }
    daemon.stop();
}
