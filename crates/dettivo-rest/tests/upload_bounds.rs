//! Upload boundaries agree for process-hosted and in-process REST backends.
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use base64::Engine as _;
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_rest::{Settings, Shim, SocketBackend, backend::Backend};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

struct Transfer {
    cap: usize,
    raw: usize,
    bytes: Mutex<Vec<u8>>,
    chunks: Mutex<u64>,
}

impl Backend for Transfer {
    fn call(&self, method: &str, params: Value) -> Result<Value, JsonRpcError> {
        Ok(match method {
            "config.get" => {
                assert_eq!(params["key"], "ipc.max_line_bytes");
                json!({"entries":[{"value":self.cap}]})
            }
            "transfer.begin" => json!({"transfer_id":"upload-字", "chunk_max_bytes":self.raw}),
            "transfer.chunk" => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(params["data_b64"].as_str().unwrap())
                    .unwrap();
                assert!(bytes.len() <= self.raw);
                let mut chunks = self.chunks.lock().unwrap();
                *chunks += 1;
                assert_eq!(params["seq"], *chunks);
                self.bytes.lock().unwrap().extend_from_slice(&bytes);
                json!({"ok":true})
            }
            "transfer.commit" => {
                assert_eq!(params["total_chunks"], *self.chunks.lock().unwrap());
                assert_eq!(
                    params["sha256"],
                    format!("{:x}", Sha256::digest(&*self.bytes.lock().unwrap()))
                );
                json!({"ok":true})
            }
            "transcripts.import" => json!({"imported":true}),
            "transfer.cancel" => json!({"ok":true}),
            other => panic!("unexpected {other}"),
        })
    }
}

#[test]
fn upload_respects_raw_and_ipc_caps_in_both_hosts() {
    for socket_host in [true, false] {
        for (cap, raw) in [(1 << 20, 1 << 20), (4096, 1 << 20), (4096, 17)] {
            let dir = tempfile::tempdir().unwrap();
            let socket = dir.path().join("daemon.sock");
            let transfer = Arc::new(Transfer {
                cap,
                raw,
                bytes: Mutex::new(Vec::new()),
                chunks: Mutex::new(0),
            });
            let stop = Arc::new(AtomicBool::new(false));
            let token = "secret-字\"".repeat(20);
            let listener = UnixListener::bind(&socket).unwrap();
            listener.set_nonblocking(true).unwrap();
            let finished = stop.clone();
            let target = transfer.clone();
            let expected_token = token.clone();
            let daemon = std::thread::spawn(move || {
                while !finished.load(Ordering::Relaxed) {
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
                    assert_eq!(req["auth_token"], expected_token);
                    let response = if line.trim_end().len() > cap {
                        json!({"jsonrpc":"2.0", "id":req["id"], "error":JsonRpcError::new(AppCode::InvalidParams, "IPC request too large", ErrorDetails::empty())})
                    } else {
                        let result = target
                            .call(req["method"].as_str().unwrap(), req["params"].clone())
                            .unwrap();
                        json!({"jsonrpc":"2.0", "id":req["id"], "result":result})
                    };
                    writeln!(stream, "{response}").unwrap();
                }
            });
            let backend: Arc<dyn Backend> = if socket_host {
                Arc::new(SocketBackend {
                    socket,
                    token: Some(token),
                    timeout: Duration::from_secs(3),
                })
            } else {
                transfer.clone()
            };
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let server = runtime
                .block_on(dettivo_rest::server::start(Arc::new(Shim {
                    backend,
                    token: "rest-token".into(),
                    settings: Settings {
                        port: 0,
                        ..Settings::default()
                    },
                })))
                .unwrap();
            let body = vec![71; raw + 13];
            let reply = dettivo_rest::client::request(
                server.addr,
                "POST",
                "/v1/transcripts/import/stream",
                &[
                    ("Authorization".into(), "Bearer rest-token".into()),
                    ("Content-Type".into(), "audio/wav".into()),
                ],
                &body,
                None,
                Duration::from_secs(10),
            )
            .unwrap();
            runtime.block_on(server.stop());
            stop.store(true, Ordering::Relaxed);
            daemon.join().unwrap();
            assert_eq!(
                reply.status,
                200,
                "socket={socket_host}, cap={cap}, raw={raw}: {}",
                String::from_utf8_lossy(&reply.body)
            );
            assert_eq!(*transfer.bytes.lock().unwrap(), body);
            assert!(*transfer.chunks.lock().unwrap() > 1);
        }
    }
}
