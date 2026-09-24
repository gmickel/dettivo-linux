//! R1 to R5 end to end: the fixtures replayed against the shim the daemon
//! hosts (`[rest] enabled`, an ephemeral port reported in
//! `system.capabilities.rest`), the same fixtures against a shim hosted
//! by this process over the Unix socket (what `dettivo rest serve`
//! runs), a second server on the port failing naming the first, and a
//! daemon with `[rest]` enabled but no token hosting nothing.

mod common;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use common::*;
use dettivo_rest::harness::{self, Row, Target, Verdict};
use dettivo_rest::{Settings, Shim, SocketBackend, StartError};
use serde_json::json;

fn fixtures() -> PathBuf {
    harness::fixtures_root()
}

fn target(port: u16, audio: Option<&Path>) -> Target {
    Target {
        addr: SocketAddr::from(([127, 0, 0, 1], port)),
        token: TOKEN.into(),
        audio: audio.map(Path::to_path_buf),
    }
}

fn assert_all_pass(rows: &[Row], audio: bool) {
    let failed: Vec<&Row> = rows.iter().filter(|r| r.verdict == Verdict::Fail).collect();
    assert!(failed.is_empty(), "{}", harness::human(rows));
    let import = rows
        .iter()
        .find(|r| r.fixture == "stream/import.wav.json")
        .expect("the import fixture ran");
    if audio {
        assert_eq!(import.verdict, Verdict::Pass, "{import:?}");
    } else {
        assert_eq!(import.verdict, Verdict::Skip, "{import:?}");
        assert!(
            import
                .detail
                .as_deref()
                .unwrap_or("")
                .contains("fetch-test-model")
        );
    }
    for set in harness::SETS {
        assert!(
            rows.iter()
                .any(|r| r.fixture.starts_with(&format!("{set}/"))),
            "no rows for {set}"
        );
    }
}

#[test]
fn the_daemon_hosts_the_shim_and_every_fixture_passes() {
    let local = local_model();
    let tree = rest_tree(local.as_ref().map(|(m, _)| m.as_path()));
    let d = Daemon::spawn(tree, &[("DETTIVO_IPC_TOKEN", TOKEN)]);
    let caps = d.rest_caps();
    assert_eq!(caps["enabled"], true, "{caps}");
    assert_eq!(caps["bind"], "127.0.0.1");
    let port = u16::try_from(caps["port"].as_u64().unwrap()).unwrap();
    assert_ne!(port, 0);
    let audio = local.as_ref().map(|(_, wav)| wav.as_path());
    let rows = harness::run(&target(port, audio), &fixtures());
    assert_all_pass(&rows, audio.is_some());

    // A second server on the daemon's port fails naming the port (R4).
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let second = runtime.block_on(dettivo_rest::server::start(Arc::new(Shim {
        backend: Arc::new(SocketBackend {
            socket: d.tree.socket(),
            token: None,
            timeout: Duration::from_secs(5),
        }),
        token: TOKEN.into(),
        settings: Settings {
            port,
            ..Settings::default()
        },
    })));
    match second {
        Err(StartError::AddrInUse(addr)) => {
            assert_eq!(addr.port(), port);
            let text = StartError::AddrInUse(addr).to_string();
            assert!(text.contains("dettivo rest status"), "{text}");
        }
        Ok(_) => panic!("a second server bound the port"),
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn a_process_hosted_shim_over_the_socket_passes_the_same_fixtures() {
    // The daemon has [rest] off (the default); this process hosts the
    // shim the way `dettivo rest serve` does, over the Unix socket.
    let tree = Tree::new();
    tree.write_config("[hotkeys]\nbackend = \"none\"\n");
    let d = Daemon::spawn(tree, &[]);
    assert_eq!(d.rest_caps()["enabled"], false);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = runtime
        .block_on(dettivo_rest::server::start(Arc::new(Shim {
            backend: Arc::new(SocketBackend {
                socket: d.tree.socket(),
                token: None,
                timeout: Duration::from_secs(30),
            }),
            token: TOKEN.into(),
            settings: Settings {
                port: 0,
                ..Settings::default()
            },
        })))
        .unwrap();
    let rows = harness::run(&target(server.port(), None), &fixtures());
    assert_all_pass(&rows, false);

    // Keep-alive: two requests over one connection answer in order.
    let mut stream = std::net::TcpStream::connect(server.addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    use std::io::{Read, Write};
    let one = format!(
        "GET /v1/system/ping HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {TOKEN}\r\n\r\n"
    );
    stream.write_all(one.as_bytes()).unwrap();
    stream.write_all(one.as_bytes()).unwrap();
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    assert_eq!(raw.matches("HTTP/1.1 200 OK").count(), 2, "{raw}");
    assert!(raw.contains("Connection: keep-alive"));

    // The daemon gone: APP_NOT_RUNNING as 503 with the systemd hint.
    drop(d);
    let reply = dettivo_rest::client::request(
        server.addr,
        "GET",
        "/v1/system/ping",
        &[("Authorization".into(), format!("Bearer {TOKEN}"))],
        b"",
        None,
        Duration::from_secs(30),
    )
    .unwrap();
    assert_eq!(
        reply.status,
        503,
        "{}",
        String::from_utf8_lossy(&reply.body)
    );
    let body = reply.json().unwrap();
    assert_eq!(body["error"]["data"]["app_code"], "APP_NOT_RUNNING");
    assert_eq!(body["error"]["code"], -32017);
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("dettivod.socket")
    );
    runtime.block_on(server.stop());
}

#[test]
fn no_token_means_no_listener_and_a_bad_bind_is_refused_at_validation() {
    let tree = rest_tree(None);
    let d = Daemon::spawn(tree, &[]);
    let caps = d.rest_caps();
    assert_eq!(caps["enabled"], false, "{caps}");
    assert_eq!(caps["port"], 0, "the configured port is reported");
    drop(d);

    // A bind that is not loopback in the file at start: the daemon does
    // not start, names the key, and nothing listens (ADR 0049).
    let tree = Tree::new();
    tree.write_config("[rest]\nenabled = true\nbind = \"0.0.0.0\"\n");
    let output = Daemon::command(&tree, &[("DETTIVO_IPC_TOKEN", TOKEN)])
        .output()
        .expect("run dettivod");
    assert!(
        !output.status.success(),
        "a refused file must not start the daemon"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("rest.bind"), "{stderr}");
    assert!(!tree.socket().exists(), "nothing listens");

    // The same edit while the daemon runs: refused at validation naming
    // the key, and the values in force stay (no listener on the refused
    // address, the shim still off).
    let tree = Tree::new();
    tree.write_config("[rest]\nenabled = false\n");
    let d = Daemon::spawn(tree, &[("DETTIVO_IPC_TOKEN", TOKEN)]);
    let set = d.request(
        "config.set",
        json!({"key": "rest.bind", "value": "0.0.0.0"}),
    );
    assert!(
        set["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("rest.bind"),
        "{set}"
    );
    d.tree
        .write_config("[rest]\nenabled = true\nbind = \"0.0.0.0\"\n");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let validate = loop {
        let v = d.request("config.validate", json!({}));
        if v["result"]["ok"] == false || std::time::Instant::now() > deadline {
            break v;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    assert_eq!(validate["result"]["ok"], false, "{validate}");
    assert_eq!(
        validate["result"]["errors"][0]["key"], "rest.bind",
        "{validate}"
    );
    assert_eq!(
        d.rest_caps()["enabled"],
        false,
        "the values in force stay: no listener"
    );
}
