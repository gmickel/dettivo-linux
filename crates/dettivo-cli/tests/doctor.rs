//! `dettivo doctor` as the FR-D5 snapshot (fn-27 R1): the JSON carries
//! every row in the shape `tests/goldens/doctor.json` records, with the
//! machine-specific values levelled the way the contract replay levels
//! them (objects keep their keys, lists and leaves become markers); the
//! human report groups the same rows; an unreachable daemon still prints
//! the machine rows and exits 1. `FN27_WRITE_GOLDENS=1` rewrites the
//! golden from this machine.

mod common;

use std::path::Path;

use common::{Daemon, Tree, cli_in, stderr, stdout};
use serde_json::Value;

const GOLDEN: &str = "tests/goldens/doctor.json";

/// The shape of a value, as `dettivo_qa::replay::shape_of` levels it.
fn shape_of(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            Value::Object(map.iter().map(|(k, v)| (k.clone(), shape_of(v))).collect())
        }
        Value::Array(_) => Value::String("list".into()),
        _ => Value::String("leaf".into()),
    }
}

/// The rows every doctor report carries, reachable daemon or not.
const MACHINE_ROWS: &[&str] = &[
    "socket",
    "service",
    "compositor",
    "session_type",
    "portals",
    "environment",
];

#[test]
fn the_json_matches_the_golden_by_shape_and_the_human_report_groups_the_rows() {
    // The compositor is pinned so the snippet block has one shape on
    // every machine (a desktop without one would report no snippet).
    let desktop = [("XDG_CURRENT_DESKTOP", "Hyprland")];
    let d = Daemon::spawn(Tree::new(), &desktop);
    let out = cli_in(&d.tree, &["--json", "doctor"], &desktop);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let report: Value = serde_json::from_str(&stdout(&out)).unwrap();

    let golden_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLDEN);
    if std::env::var_os("FN27_WRITE_GOLDENS").is_some() {
        std::fs::write(
            &golden_path,
            serde_json::to_string_pretty(&report).unwrap() + "\n",
        )
        .unwrap();
    }
    let golden: Value =
        serde_json::from_str(&std::fs::read_to_string(&golden_path).unwrap()).unwrap();
    assert_eq!(
        shape_of(&report),
        shape_of(&golden),
        "the doctor's shape drifted from {GOLDEN}; FN27_WRITE_GOLDENS=1 rewrites it"
    );

    // The FR-D5 rows by name, with the values a fresh daemon gives.
    for row in [
        "compositor",
        "session_type",
        "portals",
        "insertion",
        "audio",
        "engines",
        "models",
        "llm",
        "socket",
        "service",
        "rest",
        "mcp",
        "tier",
    ] {
        assert!(!report[row].is_null(), "{row}: {report}");
    }
    assert_eq!(report["compositor"]["name"], "Hyprland");
    assert_eq!(report["compositor"]["source"], "daemon");
    assert!(
        report["tier"]["tier"] == "gpu" || report["tier"]["tier"] == "cpu",
        "{}",
        report["tier"]
    );
    assert!(report["tier"]["reason"].is_string());
    assert_eq!(report["tier"]["tier"], report["platform"]["tier"]);
    let engines = report["engines"].as_array().unwrap();
    assert_eq!(engines.len(), 4);
    assert!(
        engines
            .iter()
            .all(|e| e.get("memory_bytes").is_some() && e.get("reason").is_some()),
        "{engines:?}"
    );
    assert_eq!(report["models"]["providers"][0]["provider"], "parakeet");
    assert_eq!(report["llm"]["provider_order"][0]["id"], "local");
    assert_eq!(report["portals"]["available"], false);
    assert_eq!(report["mcp"]["server"]["name"], "dettivo-mcp");

    let text = stdout(&cli_in(&d.tree, &["doctor"], &desktop));
    for heading in [
        "-- desktop --",
        "-- daemon --",
        "-- insertion and hotkeys --",
        "-- engines and models --",
        "-- clients --",
    ] {
        assert!(text.contains(heading), "{text}");
    }
    for row in [
        "session   Hyprland on ",
        "portals   not available",
        "tier      ",
        "engine    dettivo-engine-whisper",
        "models    ",
        "llm       provider ",
        "diarize   model diarize/diarization",
        "mcp       dettivo-mcp",
        "rest      ",
    ] {
        assert!(text.contains(row), "{row:?} missing in:\n{text}");
    }
}

#[test]
fn an_unreachable_daemon_still_prints_the_machine_rows_and_exits_1() {
    let tree = Tree::new();
    let out = cli_in(
        &tree,
        &["--json", "doctor"],
        &[("XDG_CURRENT_DESKTOP", "sway")],
    );
    assert_eq!(out.status.code(), Some(1));
    let report: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(report["daemon"]["reachable"], false);
    for row in MACHINE_ROWS {
        assert!(!report[*row].is_null(), "{row}: {report}");
    }
    assert_eq!(report["compositor"]["name"], "sway");
    assert_eq!(report["compositor"]["source"], "environment");
    assert!(report["tier"].is_object() && report["tier"]["tier"].is_null());
    assert!(report["engines"].is_null());

    let text = stdout(&cli_in(
        &tree,
        &["doctor"],
        &[("XDG_CURRENT_DESKTOP", "sway")],
    ));
    for row in [
        "socket    ",
        "service   ",
        "session   sway on ",
        "portals   ",
        "daemon    unreachable",
        "engine    not reported",
        "env       ",
    ] {
        assert!(text.contains(row), "{row:?} missing in:\n{text}");
    }
}

#[test]
fn required_probe_errors_after_good_health_fail_doctor_and_keep_the_error() {
    use serde_json::json;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::thread;
    use std::time::Duration;

    let mut outcomes = Vec::new();
    for (method, state, app_code, code, message) in [
        (
            "config.validate",
            "failure",
            "UNAUTHORIZED_CLIENT",
            -32011,
            "configuration denied",
        ),
        (
            "speech.engines",
            "unknown",
            "INTERNAL_ERROR",
            -32015,
            "engine query timed out",
        ),
        (
            "llm.engine.status",
            "failure",
            "INTERNAL_ERROR",
            -32015,
            "language engine query failed",
        ),
    ] {
        let tree = Tree::new();
        std::fs::create_dir_all(tree.socket().parent().unwrap()).unwrap();
        let listener = UnixListener::bind(tree.socket()).unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let server = thread::spawn(move || {
            let error = json!({"code":code,"message":message,"data":{"app_code":app_code,"retryable":false,"details":{}}});
            let mut probes = serde_json::Map::new();
            for name in [
                "system.version",
                "system.health",
                "system.capabilities",
                "insert.target",
                "hotkeys.status",
                "audio.devices",
                "config.validate",
                "speech.engines",
                "speech.models.status",
                "llm.providers.list",
                "llm.engine.status",
                "llm.models.status",
                "transcripts.stats",
            ] {
                let result = match name {
                    "system.health" | "config.validate" => json!({"ok":true}),
                    "speech.engines" => json!({"engines":[]}),
                    _ => json!({}),
                };
                probes.insert(
                    name.into(),
                    if name == method {
                        json!({"state":state,"result":null,"error":error})
                    } else {
                        json!({"state":"success","result":result,"error":null})
                    },
                );
            }
            while !stopped.load(Ordering::Relaxed) {
                let Ok((mut socket, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                };
                let mut line = String::new();
                BufReader::new(&socket).read_line(&mut line).unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                let name = request["method"].as_str().unwrap();
                let answer = if name == "system.diagnostics" {
                    json!({"jsonrpc":"2.0","id":request["id"],"result":{"probes":probes}})
                } else if name == method {
                    json!({"jsonrpc":"2.0","id":request["id"],"error":error})
                } else {
                    json!({"jsonrpc":"2.0","id":request["id"],"result":probes[name]["result"]})
                };
                writeln!(socket, "{answer}").unwrap();
            }
        });
        let out = cli_in(&tree, &["--json", "doctor"], &[]);
        stop.store(true, Ordering::Relaxed);
        server.join().unwrap();
        let report: Value = serde_json::from_str(&stdout(&out)).unwrap();
        outcomes.push((
            method,
            out.status.code(),
            report.to_string().contains(message),
        ));
        assert_eq!(report["daemon"]["health"]["ok"], true);
    }
    assert!(
        outcomes
            .iter()
            .all(|(_, code, error)| *code == Some(1) && *error),
        "{outcomes:?}"
    );
}

#[test]
fn a_diagnostics_deadline_is_unknown_and_never_a_healthy_report() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixListener;
    use std::{thread, time::Duration};

    let tree = Tree::new();
    std::fs::create_dir_all(tree.socket().parent().unwrap()).unwrap();
    let listener = UnixListener::bind(tree.socket()).unwrap();
    let server = thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        let mut request = String::new();
        BufReader::new(&socket).read_line(&mut request).unwrap();
        let request: Value = serde_json::from_str(&request).unwrap();
        assert_eq!(request["method"], "system.diagnostics");
        thread::sleep(Duration::from_millis(150));
    });
    let out = cli_in(&tree, &["--timeout-ms", "20", "--json", "doctor"], &[]);
    server.join().unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    let report: Value = serde_json::from_str(&stdout(&out)).unwrap();
    let probe = &report["daemon"]["probes"]["system.version"];
    assert_eq!(probe["state"], "unknown");
    assert!(
        probe["error"]["message"]
            .as_str()
            .unwrap()
            .contains("timed out")
    );
}
