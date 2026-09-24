//! R7: logs at every level carry no transcript text, audio paths, audio
//! content or prompts. The daemon runs at `trace`, every request path is
//! driven with marker strings in the places content can appear (ids,
//! params, method names, malformed and oversized lines, configuration
//! values, tokens), and the captured log must contain none of them.

mod common;

use common::{Conn, Daemon, Tree};
use serde_json::json;

const MARKERS: &[&str] = &[
    "MARKER_TRANSCRIPT",
    "MARKER_AUDIO_PATH",
    "MARKER_PROMPT",
    "MARKER_ID",
    "MARKER_METHOD",
    "MARKER_MALFORMED",
    "MARKER_BIG",
    "MARKER_SET_VALUE",
    "MARKER_BAD_VALUE",
    "MARKER_TOKEN",
    "MARKER_CONFIG_VALUE",
    "MARKER_UNKNOWN_KEY",
    "MARKER_CANCEL_REASON",
    "MARKER_ENDPOINT",
];

#[test]
fn no_log_level_leaks_content_markers() {
    let tree = Tree::new();
    tree.write_config(
        "[paths]\nmodels_dir = \"/tmp/MARKER_AUDIO_PATH/models\"\n[ipc]\nmax_line_bytes = 2048\n[daemon]\nlog_level = \"trace\"\n",
    );
    let daemon = Daemon::spawn(tree, &[("RUST_LOG", "trace")]);
    let mut conn = Conn::open(&daemon.tree.socket());

    // Every request path, each carrying a marker somewhere content could sit.
    conn.send(r#"{"jsonrpc":"2.0","id":"MARKER_ID","method":"system.ping","params":{}}"#);
    conn.send(r#"{"jsonrpc":"2.0","id":"1","method":"system.ping","params":{"text":"MARKER_TRANSCRIPT"}}"#);
    conn.send(r#"{"jsonrpc":"2.0","id":"2","method":"insert.perform","params":{"text":"MARKER_TRANSCRIPT","mode":"raw"}}"#);
    conn.send(r#"{"jsonrpc":"2.0","id":"2b","method":"insert.perform","params":{"text":"MARKER_TRANSCRIPT","mode":"clipboard_only","expected_target_bundle_id":"MARKER_ID","expected_target_pid":"MARKER_ID"}}"#);
    conn.send(r#"{"jsonrpc":"2.0","id":"2c","method":"insert.perform","params":{"mode":"raw","source_ref":{"kind":"dictation","id":"0f8fad5b-d9cb-469f-a165-70867728950e"}}}"#);
    conn.send(r#"{"jsonrpc":"2.0","id":"2d","method":"insert.undo","params":{}}"#);
    conn.send(r#"{"jsonrpc":"2.0","id":"2e","method":"insert.target","params":{}}"#);
    conn.send(
        r#"{"jsonrpc":"2.0","id":"3","method":"polish.test","params":{"prompt":"MARKER_PROMPT"}}"#,
    );
    conn.send(r#"{"jsonrpc":"2.0","id":"4","method":"MARKER_METHOD.run","params":{}}"#);
    conn.send(
        r#"{"jsonrpc":"2.0","id":"5","method":"knowledge.ask","params":{"q":"MARKER_PROMPT"}}"#,
    );
    conn.send("MARKER_MALFORMED{");
    conn.send(r#"{"jsonrpc":"1.5","id":"MARKER_ID","method":"system.ping","params":{}}"#);
    let mut big =
        br#"{"jsonrpc":"2.0","id":"MARKER_ID","method":"system.ping","params":{"x":"MARKER_BIG"#
            .to_vec();
    big.extend(std::iter::repeat_n(b'b', 4096));
    big.extend_from_slice(b"\"}}\n");
    conn.write_raw(&big);
    if let Err(e) = conn.try_read() {
        panic!(
            "no answer to the oversized line: {e}\ndaemon log:\n{}",
            daemon.log()
        );
    }
    conn.write_raw(br#"{"jsonrpc":"2.0","id":"6","method":"system.ping","params":{},"auth_token":"MARKER_TOKEN"}"#);
    conn.write_raw(b"\n");
    if let Err(e) = conn.try_read() {
        panic!(
            "no answer to the token request: {e}\ndaemon log:\n{}",
            daemon.log()
        );
    }
    // A notification gets no reply; the next request proves it was consumed.
    conn.write_raw(
        br#"{"jsonrpc":"2.0","method":"system.ping","params":{"n":"MARKER_TRANSCRIPT"}}"#,
    );
    conn.write_raw(b"\n");
    conn.send(r#"{"jsonrpc":"2.0","id":"7","method":"system.ping","params":{}}"#);
    daemon.result(
        "config.set",
        json!({"key":"paths.data_dir","value":"/tmp/MARKER_SET_VALUE"}),
    );
    daemon.request(
        "config.set",
        json!({"key":"daemon.log_level","value":"MARKER_BAD_VALUE"}),
    );
    daemon.request(
        "config.set",
        json!({"key":"daemon.MARKER_UNKNOWN_KEY","value":"x"}),
    );
    daemon.result("config.get", json!({}));
    daemon.result("config.validate", json!({}));
    daemon.result("config.path", json!({}));
    daemon.result("system.health", json!({}));
    daemon.result("system.version", json!({}));
    daemon.result("system.capabilities", json!({}));
    let transfer = daemon.result(
        "transfer.begin",
        json!({"direction": "upload", "content_type": "audio/wav", "size_hint": 0}),
    );
    assert_eq!(
        daemon.result(
            "transfer.cancel",
            json!({"transfer_id": transfer["transfer_id"], "reason": "MARKER_CANCEL_REASON"}),
        )["cancelled"],
        true
    );
    assert_eq!(
        daemon.result(
            "llm.endpoints.trust",
            json!({"url": "https://example.invalid/MARKER_ENDPOINT"}),
        )["trusted"],
        true
    );
    // An external edit that makes the file invalid, with a marker value.
    std::fs::write(
        daemon.tree.config_file(),
        "[daemon]\nlog_level = \"MARKER_CONFIG_VALUE\"\n",
    )
    .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !daemon
        .log()
        .contains("configuration changed but is invalid")
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let log = daemon.stop();

    assert!(log.contains("listening"), "trace log is populated:\n{log}");
    assert!(log.contains("transfer cancelled"), "{log}");
    assert!(log.contains("llm: remote endpoint trusted"), "{log}");
    assert!(
        log.contains("configuration written"),
        "config write path was logged:\n{log}"
    );
    assert!(
        log.contains("request refused") || log.contains("unknown method"),
        "{log}"
    );
    assert!(
        log.contains("configuration changed but is invalid"),
        "{log}"
    );
    for marker in MARKERS {
        assert!(
            !log.contains(marker),
            "{marker} leaked into the log:\n{log}"
        );
    }
}

/// A source-level backstop: no log macro in the daemon formats a request,
/// its params, a client-supplied id or method string, or a config value.
#[test]
fn log_sites_never_format_request_or_config_content() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let forbidden = [
        "request.params",
        "request.method",
        "request.id",
        "line =",
        "params =",
        "value =",
        "text =",
        "token =",
        "%line",
        "?line",
        "?request",
        "?params",
        "?value",
    ];
    let mut checked = 0;
    for entry in walk(&src) {
        let text = std::fs::read_to_string(&entry).unwrap();
        // A log site spans from the line holding `tracing::` to the line
        // that balances its parentheses, so multi-line invocations are
        // scanned whole.
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            if !lines[i].contains("tracing::") {
                i += 1;
                continue;
            }
            let start = i;
            let mut depth: i64 = 0;
            let mut site = String::new();
            loop {
                site.push_str(lines[i]);
                site.push('\n');
                depth +=
                    lines[i].matches('(').count() as i64 - lines[i].matches(')').count() as i64;
                i += 1;
                if depth <= 0 || i >= lines.len() {
                    break;
                }
            }
            checked += 1;
            for f in forbidden {
                assert!(
                    !site.contains(f),
                    "{}:{}: log site carries {f:?}:\n{}",
                    entry.display(),
                    start + 1,
                    site.trim()
                );
            }
        }
    }
    assert!(checked > 10, "found only {checked} log sites");
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
    out
}
