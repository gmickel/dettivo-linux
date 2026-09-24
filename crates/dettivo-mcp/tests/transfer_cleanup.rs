//! Failed export acknowledgments must preserve the existing destination and cancel.
use dettivo_mcp::{client::Client, tools::transfer::export_transcript};
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;
#[test]
fn failed_ack_preserves_destination_and_cancels() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("ipc");
    let out = dir.path().join("export.txt");
    std::fs::write(&out, "previous").unwrap();
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let methods = Arc::new(Mutex::new(Vec::new()));
    let seen = methods.clone();
    let (stop, stopped) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        while stopped.try_recv().is_err() {
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
            let request: Value = serde_json::from_str(&line).unwrap();
            let method = request["method"].as_str().unwrap();
            seen.lock().unwrap().push(method.to_string());
            let result = match method {
                "transfer.begin" => json!({"transfer_id":"x"}),
                "transcripts.export" => json!({}),
                "transfer.pull" => json!({"data_b64":"bmV3", "eof":true}),
                "transfer.commit" => {
                    let error = JsonRpcError::new(
                        AppCode::InternalError,
                        "ack failed",
                        ErrorDetails::empty(),
                    );
                    writeln!(
                        stream,
                        "{}",
                        json!({"jsonrpc":"2.0","id":"1","error":error})
                    )
                    .unwrap();
                    continue;
                }
                "transfer.cancel" => json!({"cancelled":true}),
                _ => panic!("{method}"),
            };
            writeln!(
                stream,
                "{}",
                json!({"jsonrpc":"2.0","id":"1","result":result})
            )
            .unwrap();
        }
    });
    let result = export_transcript(
        &Client {
            socket,
            token: None,
            timeout: Duration::from_secs(1),
        },
        &json!({"ref":{"kind":"dictation","id":"x"},"format":"txt","out_path":out}),
    );
    stop.send(()).unwrap();
    server.join().unwrap();
    assert!(result.is_err(), "ack error was ignored: {result:?}");
    assert_eq!(std::fs::read_to_string(out).unwrap(), "previous");
    assert_eq!(methods.lock().unwrap().last().unwrap(), "transfer.cancel");
}
