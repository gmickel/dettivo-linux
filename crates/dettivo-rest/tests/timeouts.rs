//! agent-surfaces/F7 and F8: one deadline bounds a request from its first
//! byte to its last, a handler that outruns it answers with the outcome
//! marked unknown, and the listener serves a bounded number of
//! connections at once and reaps the ones that closed.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use dettivo_proto::error::JsonRpcError;
use dettivo_rest::backend::Backend;
use dettivo_rest::server::{self, MAX_CONNECTIONS};
use dettivo_rest::{Settings, Shim};
use serde_json::{Value, json};

const TOKEN: &str = "test-token";

/// A daemon that answers `system.ping` after `delay`.
struct SlowBackend {
    delay: Duration,
    calls: AtomicUsize,
}

impl Backend for SlowBackend {
    fn call(&self, _method: &str, _params: Value) -> Result<Value, JsonRpcError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(self.delay);
        Ok(json!({}))
    }
}

fn start(
    delay: Duration,
    timeout_ms: u64,
) -> (tokio::runtime::Runtime, server::Server, Arc<SlowBackend>) {
    let backend = Arc::new(SlowBackend {
        delay,
        calls: AtomicUsize::new(0),
    });
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = runtime
        .block_on(server::start(Arc::new(Shim {
            backend: backend.clone(),
            token: TOKEN.into(),
            settings: Settings {
                port: 0,
                request_timeout_ms: timeout_ms,
                ..Settings::default()
            },
        })))
        .unwrap();
    (runtime, server, backend)
}

fn ping(addr: std::net::SocketAddr) -> String {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let request = format!(
        "GET /v1/system/ping HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {TOKEN}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    raw
}

#[test]
fn unicode_auth_prefix_is_refused_and_the_listener_keeps_serving() {
    let (_runtime, server, backend) = start(Duration::ZERO, 3000);
    for prefix in ["aaaaaaé", "💬💬", "bearér token"] {
        let reply = dettivo_rest::client::request(
            server.addr,
            "GET",
            "/v1/system/ping",
            &[("Authorization".into(), prefix.into())],
            b"",
            None,
            Duration::from_secs(3),
        )
        .unwrap();
        assert_eq!(reply.status, 401, "{prefix}: {reply:?}");
        assert_eq!(
            reply.json().unwrap()["error"]["data"]["app_code"],
            "UNAUTHORIZED_CLIENT"
        );
        assert!(ping(server.addr).starts_with("HTTP/1.1 200"));
    }
    assert_eq!(backend.calls.load(Ordering::SeqCst), 3);
}

#[test]
fn a_handler_past_the_deadline_answers_with_the_outcome_unknown() {
    let (_runtime, server, backend) = start(Duration::from_millis(1500), 300);
    let started = Instant::now();
    let raw = ping(server.addr);
    let elapsed = started.elapsed();
    assert!(raw.starts_with("HTTP/1.1 500"), "{raw}");
    assert!(raw.contains("its outcome is unknown"), "{raw}");
    assert!(
        elapsed < Duration::from_millis(1200),
        "the answer waited for the handler: {elapsed:?}"
    );
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn a_slow_reader_does_not_hold_the_request_past_the_deadline() {
    let (_runtime, server, _backend) = start(Duration::ZERO, 300);
    // A request that never completes its head: the read side of the same
    // deadline closes the connection.
    let mut stream = TcpStream::connect(server.addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .write_all(b"GET /v1/system/ping HTTP/1.1\r\n")
        .unwrap();
    let started = Instant::now();
    let mut buf = Vec::new();
    let _ = stream.read_to_end(&mut buf);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "the half-sent request stayed open: {:?}",
        started.elapsed()
    );
}

#[test]
fn the_listener_caps_open_connections_and_reaps_closed_ones() {
    let (_runtime, server, _backend) = start(Duration::ZERO, 2000);
    let mut idle: Vec<TcpStream> = (0..MAX_CONNECTIONS)
        .map(|_| TcpStream::connect(server.addr).unwrap())
        .collect();
    // Give the accept loop a moment to take them all.
    std::thread::sleep(Duration::from_millis(200));
    let mut refused = TcpStream::connect(server.addr).unwrap();
    refused
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut buf = [0u8; 16];
    let started = Instant::now();
    assert_eq!(
        refused.read(&mut buf).unwrap_or(0),
        0,
        "the connection past the cap is closed unanswered"
    );
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "closed at once, not at its deadline: {:?}",
        started.elapsed()
    );
    // Closing the idle ones frees their slots: the next request answers.
    idle.clear();
    std::thread::sleep(Duration::from_millis(200));
    let raw = ping(server.addr);
    assert!(raw.starts_with("HTTP/1.1 200"), "{raw}");
}
