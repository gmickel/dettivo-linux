//! R3: the `system.*`, `config.*`, `audio.*`, `insert.*`, `transcripts.*`,
//! `meetings.*`, `transfer.*`, `polish.*`, `llm.*` and `hotkeys.*` fixtures
//! pass against the live socket
//! (insertion through the QA mock backend and target, the seeded daemon
//! answering the history ones), every other contract method answers `NOT_IMPLEMENTED` in
//! the reserved shape, a malformed line is `INVALID_PARAMS`, and an
//! oversized line is rejected without dropping the connection.

mod common;
#[path = "contract/normalise.rs"]
mod normalise;

use std::path::{Path, PathBuf};

use common::{Conn, Daemon, Tree};
use normalise::normalise;
use serde_json::{Value, json};

/// Namespaces the daemon answers today; everything else is `NOT_IMPLEMENTED`.
const IMPLEMENTED: &[&str] = &[
    "system",
    "config",
    "audio",
    "speech",
    "dictation",
    "meetings",
    "events",
    "insert",
    "transcripts",
    "transfer",
    "hotkeys",
    "polish",
    "llm",
];
/// Namespaces whose fixtures replay verbatim on a fresh connection; speech,
/// dictation and events answers are machine-specific or stateful and their
/// own tests cover them.
const REPLAYED: &[&str] = &[
    "system",
    "config",
    "audio",
    "insert",
    "transcripts",
    "meetings",
    "transfer",
    "hotkeys",
    "polish",
    "llm",
];

/// The QA mock and the seed: insertion writes to a file, the focus probe
/// answers with the fixed mock target the fixtures expect, and the history
/// store holds the twelve seeded items.
const MOCK_ENV: &[(&str, &str)] = &[
    ("DETTIVO_QA_MODE", "1"),
    ("DETTIVO_MOCK_INSERT", "1"),
    ("DETTIVO_E2E_SEED", "1"),
];

/// Fixtures whose answer needs state an earlier request built (an upload,
/// an item with retained audio, a running import, a frontmost app, a
/// language model on disk, an active or partial meeting); the tests in
/// `history.rs`, `history_model.rs`, `llm_models.rs`, `meetings.rs`,
/// `meetings_import.rs`, `meetings_diarize.rs` and `meetings_notes.rs` cover
/// them by name, as the QA replay's `STATEFUL` list records.
const STATEFUL: &[&str] = &[
    "polish/rules.create.json",
    "polish/rules.update.json",
    "polish/rules.delete.json",
    "llm/providers.list.json",
    "llm/models.status.json",
    "llm/models.download.json",
    "llm/models.cancel.json",
    "llm/models.delete.json",
    "llm/engine.status.json",
    "transcripts/rerun.json",
    "transcripts/cancel.json",
    "transcripts/import.json",
    "transcripts/import.error-conflict-already-in-progress.json",
    "transcripts/export.json",
    "transfer/chunk.json",
    "transfer/chunk.error-rate-limited.json",
    "transfer/commit.json",
    "transfer/cancel.json",
    "transfer/pull.json",
    "meetings/diarize.json",
    "meetings/diarize.error-not-found-model-missing.json",
    "meetings/diarize.error-conflict-not-completed.json",
    "meetings/diarize.error-conflict-running.json",
    "meetings/speakers.rename.json",
    "meetings/speakers.suggest.json",
    "meetings/start.json",
    "meetings/start.error-conflict-engine-without-timestamps.json",
    "meetings/stop.json",
    "meetings/cancel.json",
    "meetings/status.json",
    "meetings/delete.json",
    "meetings/recover.json",
    "meetings/discard.json",
    "meetings/disclosure.acknowledge.json",
    "meetings/notes.set.json",
    "meetings/analyze.json",
];

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../dettivo-proto/fixtures")
}

struct Fixture {
    path: PathBuf,
    doc: Value,
}

impl Fixture {
    fn rel(&self) -> String {
        let root = fixtures_root().canonicalize().unwrap();
        self.path
            .canonicalize()
            .unwrap()
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }
}

fn load(namespace: &str) -> Vec<Fixture> {
    let dir = fixtures_root().join(namespace);
    let mut out: Vec<Fixture> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        // Event snapshots (`<topic>.event.json`) pin a notification's
        // shape; there is no request to send (the proto suite checks them).
        .filter(|p| {
            !p.file_stem()
                .is_some_and(|s| s.to_string_lossy().ends_with(".event"))
        })
        .map(|path| {
            let doc = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            Fixture { path, doc }
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn load_all() -> Vec<Fixture> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(fixtures_root())
        .unwrap()
        .filter_map(|e| e.ok())
    {
        if entry.path().is_dir() {
            out.extend(load(entry.file_name().to_str().unwrap()));
        }
    }
    out
}

fn caps_match(fixture: &Value, caps: &Value) -> bool {
    fixture["capabilities"]
        .as_object()
        .unwrap()
        .iter()
        .all(|(path, expected)| {
            let mut cursor = caps;
            for segment in path.split('.') {
                cursor = &cursor[segment];
            }
            cursor == expected
        })
}

#[test]
fn implemented_fixtures_pass_against_the_live_socket() {
    let tree = Tree::new();
    let daemon = Daemon::spawn(tree, MOCK_ENV);
    let caps = daemon.result("system.capabilities", json!({}));
    let mut replayed = 0;
    for namespace in REPLAYED {
        for f in load(namespace) {
            let method = f.doc["method"].as_str().unwrap();
            let expected = f
                .doc
                .get("response")
                .or(f.doc.get("error"))
                .unwrap()
                .clone();
            if expected["error"]["data"]["app_code"] == "UNAUTHORIZED_CLIENT" {
                eprintln!("skip {}: needs a connection from another uid", f.rel());
                continue;
            }
            if STATEFUL.contains(&f.rel().as_str()) {
                eprintln!("skip {}: stateful, covered by history tests", f.rel());
                continue;
            }
            if !caps_match(&f.doc, &caps) {
                panic!("{}: capability flags not declared by this daemon", f.rel());
            }
            let actual = daemon.call(&f.doc["request"].to_string());
            if method == "system.diagnostics" {
                let diagnostic: dettivo_proto::methods::system::DiagnosticsResult =
                    serde_json::from_value(actual["result"].clone()).unwrap();
                assert_eq!(diagnostic.probes.len(), 13);
                assert!(
                    diagnostic
                        .probes
                        .values()
                        .all(|p| p.state == dettivo_proto::methods::system::ProbeState::Success)
                );
                replayed += 1;
                continue;
            }
            assert_eq!(
                normalise(method, actual.clone(), &expected),
                normalise(method, expected.clone(), &expected),
                "{}:\n actual   {actual}\n expected {expected}",
                f.rel()
            );
            replayed += 1;
        }
    }
    assert!(replayed >= 22, "only {replayed} fixtures replayed");
    daemon.stop();
}

#[test]
fn every_other_contract_method_is_not_implemented_in_the_reserved_shape() {
    let daemon = Daemon::spawn(Tree::new(), &[]);
    let mut checked = 0;
    for f in load_all() {
        let method = f.doc["method"].as_str().unwrap();
        if IMPLEMENTED
            .iter()
            .any(|ns| method.starts_with(&format!("{ns}.")))
        {
            continue;
        }
        let actual = daemon.call(&f.doc["request"].to_string());
        assert_eq!(actual["id"], f.doc["request"]["id"], "{}", f.path.display());
        assert_eq!(
            actual["error"],
            json!({
                "code": -32014,
                "message": format!("{method} is not implemented"),
                "data": {"app_code": "NOT_IMPLEMENTED", "retryable": false, "details": {}}
            }),
            "{}: {actual}",
            f.path.display()
        );
        checked += 1;
    }
    assert!(checked > 8, "only {checked} methods checked");
    daemon.stop();
}

#[test]
fn malformed_and_oversized_lines_are_invalid_params_and_keep_the_connection() {
    let tree = Tree::new();
    tree.write_config("[ipc]\nmax_line_bytes = 4096\n");
    let daemon = Daemon::spawn(tree, &[]);
    let mut conn = Conn::open(&daemon.tree.socket());

    let malformed = conn.send("{not json");
    assert_eq!(malformed["error"]["data"]["app_code"], "INVALID_PARAMS");
    assert_eq!(malformed["id"], Value::Null);

    let array = conn.send("[1,2]");
    assert_eq!(array["error"]["data"]["app_code"], "INVALID_PARAMS");

    let bad_envelope =
        conn.send(r#"{"jsonrpc":"1.0","id":"e","method":"system.ping","params":{}}"#);
    assert_eq!(bad_envelope["error"]["data"]["app_code"], "INVALID_PARAMS");
    assert_eq!(bad_envelope["id"], "e");

    let mut big = br#"{"jsonrpc":"2.0","id":"big","method":"system.ping","params":{"x":""#.to_vec();
    big.extend(std::iter::repeat_n(b'a', 8192));
    big.extend_from_slice(b"\"}}\n");
    conn.write_raw(&big);
    let oversized = conn.read();
    assert_eq!(oversized["error"]["data"]["app_code"], "INVALID_PARAMS");
    assert!(
        oversized["error"]["message"]
            .as_str()
            .unwrap()
            .contains("4096"),
        "{oversized}"
    );

    let after = conn.send(r#"{"jsonrpc":"2.0","id":"after","method":"system.ping","params":{}}"#);
    assert_eq!(after["result"]["ok"], true);
    assert_eq!(after["id"], "after");

    let unknown = conn.send(r#"{"jsonrpc":"2.0","id":7,"method":"nope.method","params":{}}"#);
    assert_eq!(unknown["error"]["data"]["app_code"], "INVALID_PARAMS");
    assert_eq!(unknown["id"], 7);
    daemon.stop();
}

#[test]
fn notifications_get_no_reply_and_do_not_block_the_next_request() {
    let daemon = Daemon::spawn(Tree::new(), &[]);
    let mut conn = Conn::open(&daemon.tree.socket());
    conn.write_raw(br#"{"jsonrpc":"2.0","method":"system.ping","params":{}}"#);
    conn.write_raw(b"\n");
    let reply = conn.send(r#"{"jsonrpc":"2.0","id":"n","method":"system.ping","params":{}}"#);
    assert_eq!(reply["id"], "n");
    daemon.stop();
}

/// `speech.engines` (Linux addition) lists every known engine binary with
/// the fixture's shape; the whisper row names the built binary when the
/// engines directory points at the build output.
#[test]
fn speech_engines_reports_the_whisper_binary() {
    let tree = Tree::new();
    let bin_dir = Path::new(env!("CARGO_BIN_EXE_dettivod"))
        .parent()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(tree.config_file().parent().unwrap()).unwrap();
    std::fs::write(
        tree.config_file(),
        format!(
            "[engines]\ndirectory = \"{}\"\n",
            bin_dir.replace('\\', "\\\\")
        ),
    )
    .unwrap();
    let daemon = Daemon::spawn(tree, &[]);
    let result = daemon.result("speech.engines", json!({}));
    let rows = result["engines"].as_array().unwrap();
    let names: Vec<&str> = rows.iter().map(|r| r["binary"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "dettivo-engine-whisper",
            "dettivo-engine-parakeet",
            "dettivo-engine-llm",
            "dettivo-engine-diarize"
        ]
    );
    let whisper = &rows[0];
    assert_eq!(whisper["running"], false);
    assert_eq!(whisper["degraded"], false);
    assert_eq!(whisper["crashes"], 0);
    if Path::new(&bin_dir).join("dettivo-engine-whisper").is_file() {
        assert!(
            whisper["path"]
                .as_str()
                .unwrap()
                .ends_with("dettivo-engine-whisper")
        );
    }
    assert_eq!(rows[1]["running"], false);
    for f in load("speech")
        .into_iter()
        .filter(|f| f.doc["method"] == "speech.engines")
    {
        let shape: dettivo_proto::methods::speech::EnginesResult =
            serde_json::from_value(f.doc["response"]["result"].clone()).unwrap();
        assert_eq!(shape.engines.len(), 3, "{}", f.path.display());
    }
    daemon.stop();
}
