//! Waiting follows the item reference even behind a full page of newer rows.
mod common;

use common::*;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

const ID: &str = "00000000-0000-0000-0000-000000000001";

#[test]
fn import_and_rerun_poll_by_reference_through_completion_cancellation_and_deletion() {
    for operation in ["import", "rerun"] {
        for status in ["completed", "cancelled", "failed", "deleted"] {
            let tree = Tree::new();
            std::fs::create_dir_all(tree.socket().parent().unwrap()).unwrap();
            let listener = UnixListener::bind(tree.socket()).unwrap();
            listener.set_nonblocking(true).unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let done = stop.clone();
            let daemon = std::thread::spawn(move || {
                let mut polls = 0;
                while !done.load(Ordering::Relaxed) {
                    let (mut stream, _) = match listener.accept() {
                        Ok(pair) => pair,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(1));
                            continue;
                        }
                        Err(e) => panic!("{e}"),
                    };
                    let mut line = String::new();
                    BufReader::new(stream.try_clone().unwrap())
                        .read_line(&mut line)
                        .unwrap();
                    let req: Value = serde_json::from_str(&line).unwrap();
                    let result = match req["method"].as_str().unwrap() {
                        "config.get" => json!({"entries":[{"value":1048576}]}),
                        "transfer.begin" => {
                            json!({"transfer_id":"upload", "chunk_max_bytes":1048576})
                        }
                        "transfer.chunk" | "transfer.commit" | "events.subscribe" => json!({}),
                        "transcripts.import" | "transcripts.rerun" => {
                            json!({"ref":{"kind":"dictation", "id":ID}, "job":{"job_id":"job", "state":"running"}})
                        }
                        "transcripts.list" => {
                            json!({"items":(0..100).map(|i| json!({"ref":{"id":format!("newer-{i}")},"status":"completed"})).collect::<Vec<_>>()})
                        }
                        "transcripts.get" => {
                            assert_eq!(req["params"]["ref"], json!({"kind":"dictation", "id":ID}));
                            polls += 1;
                            if polls > 1 && status == "deleted" {
                                let error = dettivo_proto::error::JsonRpcError::new(
                                    dettivo_proto::error::AppCode::NotFound,
                                    "item deleted",
                                    dettivo_proto::error::ErrorDetails::empty(),
                                );
                                writeln!(
                                    stream,
                                    "{}",
                                    json!({"jsonrpc":"2.0", "id":req["id"], "error":error})
                                )
                                .unwrap();
                                continue;
                            }
                            json!({"ref":{"kind":"dictation", "id":ID}, "facts":{"status": if polls == 1 {"transcribing"} else {status}}, "text_raw":"finished"})
                        }
                        other => panic!("unexpected {other}"),
                    };
                    writeln!(
                        stream,
                        "{}",
                        json!({"jsonrpc":"2.0", "id":req["id"], "result":result})
                    )
                    .unwrap();
                }
                polls
            });
            let file = tree.root().join("audio.wav");
            std::fs::write(&file, b"audio").unwrap();
            let argument = if operation == "import" {
                file.to_str().unwrap()
            } else {
                ID
            };
            let output = cli_in(&tree, &["--json", "history", operation, argument], &[]);
            stop.store(true, Ordering::Relaxed);
            let polls = daemon.join().unwrap();
            assert!(
                polls >= 2,
                "{operation}/{status}: item fell off page: {}",
                stderr(&output)
            );
            assert_eq!(
                output.status.code(),
                Some(if status == "completed" { 0 } else { 1 }),
                "{}",
                stderr(&output)
            );
            let body: Value = serde_json::from_slice(&output.stdout).unwrap();
            if status == "completed" {
                assert_eq!(body["text_raw"], "finished");
                assert!(output.stderr.is_empty());
            } else {
                assert!(body["error"].is_object());
                assert!(!output.stderr.is_empty());
            }
        }
    }
}
