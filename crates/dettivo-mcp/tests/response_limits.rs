//! Whole-envelope limits and refused transport switches over fragmented input.
use std::io::{BufRead, BufReader, Cursor, Read, Write};
use std::os::unix::net::UnixListener;
use std::time::Duration;

use dettivo_mcp::client::Client;
use dettivo_mcp::protocol::{Config, Server};
use dettivo_mcp::transport::{self, Framing, Reader};
use serde_json::{Value, json};

fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/transport/limits.json")).unwrap()
}

fn config(socket: std::path::PathBuf, cap: u64) -> Config {
    Config {
        client: Client {
            socket,
            token: None,
            timeout: Duration::from_secs(1),
        },
        max_message_bytes: cap,
        debug: false,
    }
}

#[test]
fn complete_responses_obey_the_byte_cap_in_both_framings() {
    for framing in [Framing::Line, Framing::ContentLength] {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("daemon.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let daemon = std::thread::spawn(move || {
            for stream in listener.incoming().take(3) {
                let mut stream = stream.unwrap();
                let mut line = String::new();
                BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut line)
                    .unwrap();
                let req: Value = serde_json::from_str(&line).unwrap();
                let result = match req["method"].as_str().unwrap() {
                    "system.capabilities" => json!({}),
                    "transcripts.get" => json!({"text_raw": "字".repeat(500)}),
                    "transcripts.list" => {
                        json!({"items": [{"ref": {"kind": "dictation", "id": "x"}, "title": "字".repeat(500)}]})
                    }
                    other => panic!("unexpected {other}"),
                };
                writeln!(
                    stream,
                    "{}",
                    json!({"jsonrpc":"2.0", "id":req["id"], "result": result})
                )
                .unwrap();
            }
        });
        let requests = [
            ("initialize", json!({"protocolVersion": "é".repeat(180)})),
            ("tools/list", json!({})),
            (
                "tools/call",
                json!({"name":"get_transcript", "arguments":{"id":"00000000-0000-0000-0000-000000000001"}}),
            ),
            ("resources/list", json!({})),
            ("resources/templates/list", json!({})),
            ("ping", json!({})),
        ];
        let input: Vec<u8> = requests
            .iter()
            .enumerate()
            .flat_map(|(id, (method, params))| {
                transport::encode(
                    framing,
                    &json!({"jsonrpc":"2.0", "id":id, "method":method, "params":params}),
                )
            })
            .collect();
        let mut output = Vec::new();
        Server::new(config(socket, 512))
            .run(Cursor::new(input), &mut output)
            .unwrap();
        daemon.join().unwrap();
        let mut reader = Reader::new(Cursor::new(output), usize::MAX);
        for id in 0..requests.len() {
            let response = reader.read_message().unwrap().unwrap();
            assert!(
                serde_json::to_vec(&response).unwrap().len() <= 512,
                "oversized {response}"
            );
            assert_eq!(response["id"], id);
            if id < requests.len() - 1 {
                assert_eq!(response["error"], fixture()["outgoing_error"], "{response}");
            } else {
                assert_eq!(response["result"], json!({}));
            }
        }
        assert!(reader.read_message().unwrap().is_none());
    }
}

struct Fragmented {
    bytes: Cursor<Vec<u8>>,
    size: usize,
}
impl Read for Fragmented {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let n = self.size.min(output.len());
        self.bytes.read(&mut output[..n])
    }
}

#[test]
fn refused_fragmented_switches_never_dispatch_their_payload() {
    for (original, switched) in [
        (Framing::Line, Framing::ContentLength),
        (Framing::ContentLength, Framing::Line),
    ] {
        for size in [1, 3, 65536] {
            let mut input =
                transport::encode(original, &json!({"jsonrpc":"2.0", "id":1,"method":"ping"}));
            if switched == Framing::ContentLength {
                input.extend_from_slice(b"X-Trace: ignored\r\n");
            }
            input.extend(transport::encode(switched, &json!({"jsonrpc":"2.0", "id":99,"method":"tools/call", "params":{"name":"get_status"}})));
            input.extend(transport::encode(
                original,
                &json!({"jsonrpc":"2.0", "id":3,"method":"ping"}),
            ));
            let mut output = Vec::new();
            Server::new(config("/nonexistent/dettivo.sock".into(), 4096))
                .run(
                    Fragmented {
                        bytes: Cursor::new(input),
                        size,
                    },
                    &mut output,
                )
                .unwrap();
            let mut reader = Reader::new(Cursor::new(output), 4096);
            assert_eq!(reader.read_message().unwrap().unwrap()["id"], 1);
            let refused = reader.read_message().unwrap().unwrap();
            assert_eq!(
                refused["error"],
                fixture()["framing_switch_error"],
                "{original:?}, {size}: {refused}"
            );
            assert_eq!(
                reader.read_message().unwrap().unwrap()["id"],
                3,
                "rejected payload executed"
            );
            assert!(reader.read_message().unwrap().is_none());
        }
    }
}

#[test]
fn a_small_cap_uses_the_compact_error_and_retains_the_id() {
    for framing in [Framing::Line, Framing::ContentLength] {
        let input = transport::encode(
            framing,
            &json!({"jsonrpc":"2.0", "id":"字", "method":"initialize"}),
        );
        let mut output = Vec::new();
        Server::new(config("/nonexistent/dettivo.sock".into(), 180))
            .run(Cursor::new(input), &mut output)
            .unwrap();
        let result = Reader::new(Cursor::new(output), 180)
            .read_message()
            .unwrap()
            .unwrap();
        assert_eq!(result["id"], "字");
        assert_eq!(result["error"], fixture()["compact_error"]);
    }
}

#[test]
fn a_cap_too_small_for_the_error_closes_without_oversized_output() {
    let input = transport::encode(
        Framing::Line,
        &json!({"jsonrpc":"2.0", "id":"x".repeat(100), "method":"initialize"}),
    );
    let mut output = Vec::new();
    let result = Server::new(config(
        "/nonexistent/dettivo.sock".into(),
        (input.len() - 1) as u64,
    ))
    .run(Cursor::new(input), &mut output);
    assert!(result.is_err());
    assert!(output.is_empty());
}
