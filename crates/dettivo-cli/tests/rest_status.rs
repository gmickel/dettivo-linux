//! CLI readiness preserves HTTP failures in either hosting mode.
mod common;

use common::*;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::unix::net::UnixListener;
use std::thread;

#[test]
fn status_preserves_http_outcomes_and_failure_exits_for_both_hosts() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/rest-status.json")).unwrap();
    for daemon_hosted in [false, true] {
        for case in &cases {
            let status = case["status"].as_u64().unwrap_or(0);
            let exit = case["exit"].as_i64().unwrap() as i32;
            for json_output in [false, true] {
                let tree = Tree::new();
                std::fs::create_dir_all(tree.socket().parent().unwrap()).unwrap();
                let ipc = UnixListener::bind(tree.socket()).unwrap();
                let tcp = TcpListener::bind("127.0.0.1:0").unwrap();
                let port = tcp.local_addr().unwrap().port();
                let ipc_thread = thread::spawn(move || {
                    for stream in ipc.incoming().take(2) {
                        let mut stream = stream.unwrap();
                        let mut line = String::new();
                        BufReader::new(&stream).read_line(&mut line).unwrap();
                        let request: Value = serde_json::from_str(&line).unwrap();
                        let result = if request["method"] == "system.capabilities" {
                            json!({"rest": {"enabled": daemon_hosted, "bind": "127.0.0.1", "port": port}})
                        } else {
                            json!({"entries": [{"key": "rest.port", "value": port}]})
                        };
                        writeln!(
                            stream,
                            "{}",
                            json!({"jsonrpc":"2.0", "id":request["id"], "result": result})
                        )
                        .unwrap();
                    }
                });
                let body = case["body"].to_string();
                let http_thread = if status == 0 {
                    drop(tcp);
                    None
                } else {
                    Some(thread::spawn(move || {
                        let (mut stream, _) = tcp.accept().unwrap();
                        let mut reader = BufReader::new(&stream);
                        loop {
                            let mut line = String::new();
                            assert_ne!(reader.read_line(&mut line).unwrap(), 0);
                            if line == "\r\n" {
                                break;
                            }
                        }
                        write!(stream, "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                    }))
                };
                let args = if json_output {
                    vec!["--json", "rest", "status"]
                } else {
                    vec!["rest", "status"]
                };
                let output = cli_in(&tree, &args, &[]);
                ipc_thread.join().unwrap();
                if let Some(handle) = http_thread {
                    handle.join().unwrap();
                }
                assert_eq!(
                    output.status.code(),
                    Some(exit),
                    "host={daemon_hosted} status={status}: {} {}",
                    stdout(&output),
                    stderr(&output)
                );
                let text = stdout(&output);
                if json_output {
                    let report: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
                    assert_eq!(
                        report["status"],
                        if status == 0 {
                            Value::Null
                        } else {
                            json!(status)
                        }
                    );
                    assert_eq!(report["listening"], status != 0);
                    assert_eq!(report["authorized"], case["authorized"]);
                    if status != 0 && status != 200 {
                        assert_eq!(report["error"], case["body"]);
                    }
                    if status == 0 {
                        assert!(report["error"].as_str().unwrap().contains("connect"));
                    }
                } else if status != 0 {
                    assert!(text.contains(&status.to_string()), "{text}");
                    if status >= 500 {
                        assert!(!text.contains("refused (401)"), "{text}");
                    }
                }
            }
        }
    }
}
