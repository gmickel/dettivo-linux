//! The golden fixture suite: every file under `fixtures/` round-trips
//! byte-stable through the contract types, reserved methods answer
//! `NOT_IMPLEMENTED`, and every capability flag a fixture needs resolves in
//! the capability snapshot. The same files are replayed against a live
//! socket by the QA rig; `fixtures/README.md` documents the layout.

use dettivo_proto::catalog::{self, MethodStatus};
use dettivo_proto::envelope::{ErrorResponse, Request, SuccessResponse};
use dettivo_proto::error::AppCode;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

struct Fixture {
    path: PathBuf,
    namespace: String,
    stem: String,
    doc: Value,
}

/// True for an event snapshot (`<namespace>/<topic>.event.json`): one
/// `events.notify` line pinned for a Linux topic, no request or answer.
fn is_event_snapshot(path: &Path) -> bool {
    path.file_stem()
        .is_some_and(|s| s.to_string_lossy().ends_with(".event"))
}

fn load_all() -> Vec<Fixture> {
    load_where(|p| !is_event_snapshot(p))
}

fn load_where(keep: impl Fn(&Path) -> bool) -> Vec<Fixture> {
    let root = fixtures_root();
    let mut out = Vec::new();
    let mut namespaces: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("fixtures directory exists")
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.is_dir())
        .collect();
    namespaces.sort();
    for ns_dir in namespaces {
        let namespace = ns_dir.file_name().unwrap().to_string_lossy().into_owned();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&ns_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .filter(|p| keep(p))
            .collect();
        files.sort();
        for path in files {
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).unwrap();
            let doc: Value = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("{}: not JSON: {e}", path.display()));
            out.push(Fixture {
                path,
                namespace: namespace.clone(),
                stem,
                doc,
            });
        }
    }
    assert!(
        !out.is_empty(),
        "no fixtures found under {}",
        root.display()
    );
    out
}

fn canonical(value: &Value) -> String {
    serde_json::to_string(value).unwrap()
}

/// Every fixture's file name states its method: `<namespace>/<tail>.json`
/// or `<namespace>/<tail>.error-<case>.json`, and the `method` field
/// inside agrees. A directory or file outside that shape fails here.
#[test]
fn layout_names_every_namespace_and_method() {
    for f in load_all() {
        let method = f.doc["method"]
            .as_str()
            .unwrap_or_else(|| panic!("{}: missing method", f.path.display()));
        let (ns, tail) = method
            .split_once('.')
            .unwrap_or_else(|| panic!("{}: method has no namespace", f.path.display()));
        assert_eq!(
            ns,
            f.namespace,
            "{}: directory is not the namespace",
            f.path.display()
        );
        let expected_tail = f.stem.split(".error-").next().unwrap();
        assert_eq!(
            expected_tail,
            tail,
            "{}: file name does not name the method",
            f.path.display()
        );
        assert!(
            catalog::lookup(method).is_some(),
            "{}: {method} is not in the catalog",
            f.path.display()
        );
        assert!(
            f.doc.get("capabilities").is_some_and(Value::is_object),
            "{}: every fixture declares the capability flags it needs (an object, empty when none)",
            f.path.display()
        );
        let has_response = f.doc.get("response").is_some();
        let has_error = f.doc.get("error").is_some();
        assert!(
            has_response ^ has_error,
            "{}: exactly one of response or error",
            f.path.display()
        );
        assert_eq!(
            f.stem.contains(".error-"),
            has_error,
            "{}: error cases are named *.error-<case>.json",
            f.path.display()
        );
    }
}

/// Every request, response and error round-trips byte-stable through the
/// envelope and the method's typed params and result.
#[test]
fn every_fixture_round_trips_byte_stable() {
    for f in load_all() {
        let method = f.doc["method"].as_str().unwrap();
        let request: Request = serde_json::from_value(f.doc["request"].clone())
            .unwrap_or_else(|e| panic!("{}: request envelope: {e}", f.path.display()));
        assert_eq!(request.method, method, "{}", f.path.display());
        assert_eq!(
            canonical(&serde_json::to_value(&request).unwrap()),
            canonical(&f.doc["request"])
        );
        // An INVALID_PARAMS case documents a request the server rejects, so
        // its params need not fit the typed shape; every other request's
        // params round-trip byte-stable.
        let rejects_params = f
            .doc
            .get("error")
            .is_some_and(|e| e["error"]["data"]["app_code"] == "INVALID_PARAMS");
        match catalog::round_trip_params(method, &request.params) {
            Ok(params) => assert_eq!(
                canonical(&params),
                canonical(&request.params),
                "{}: params changed",
                f.path.display()
            ),
            Err(e) if rejects_params => {
                assert!(!e.to_string().is_empty(), "{}", f.path.display());
            }
            Err(e) => panic!("{}: {e}", f.path.display()),
        }

        if let Some(response) = f.doc.get("response") {
            let typed: SuccessResponse = serde_json::from_value(response.clone())
                .unwrap_or_else(|e| panic!("{}: response envelope: {e}", f.path.display()));
            assert_eq!(
                canonical(&serde_json::to_value(&typed).unwrap()),
                canonical(response)
            );
            let result = catalog::round_trip_result(method, &typed.result)
                .unwrap_or_else(|e| panic!("{}: {e}", f.path.display()));
            assert_eq!(
                canonical(&result),
                canonical(&typed.result),
                "{}: result changed",
                f.path.display()
            );
        }
        if let Some(error) = f.doc.get("error") {
            let typed: ErrorResponse = serde_json::from_value(error.clone())
                .unwrap_or_else(|e| panic!("{}: error envelope: {e}", f.path.display()));
            assert_eq!(
                canonical(&serde_json::to_value(&typed).unwrap()),
                canonical(error)
            );
            assert!(
                typed.error.app_code().is_server_code(),
                "{}",
                f.path.display()
            );
        }
    }
}

/// Every reserved method has a fixture that expects `NOT_IMPLEMENTED`, and
/// every implemented method has at least one success fixture.
#[test]
fn reserved_methods_expect_not_implemented_and_implemented_have_success() {
    let all = load_all();
    for spec in catalog::METHODS {
        let mine: Vec<&Fixture> = all
            .iter()
            .filter(|f| f.doc["method"] == spec.name)
            .collect();
        assert!(!mine.is_empty(), "{} has no fixture", spec.name);
        match spec.status {
            MethodStatus::Reserved => {
                for f in mine {
                    let typed: ErrorResponse = serde_json::from_value(f.doc["error"].clone())
                        .unwrap_or_else(|_| {
                            panic!("{}: reserved method must expect an error", f.path.display())
                        });
                    assert_eq!(
                        typed.error.app_code(),
                        AppCode::NotImplemented,
                        "{}",
                        f.path.display()
                    );
                    assert_eq!(typed.error.code, -32014, "{}", f.path.display());
                }
            }
            MethodStatus::Implemented => {
                assert!(
                    mine.iter().any(|f| f.doc.get("response").is_some()),
                    "{} has no success fixture",
                    spec.name
                );
            }
        }
    }
}

/// The capability snapshot fixture is complete, and every flag another
/// fixture requires resolves in it with the expected value, or is a flag
/// the snapshot declares `false` where the fixture needs `true`: an
/// admitted gap (the contract replay reports the method as pending
/// behind that flag) rather than a promise the daemon breaks.
#[test]
fn capability_flags_resolve_in_the_snapshot() {
    let all = load_all();
    let snapshot = all
        .iter()
        .find(|f| f.doc["method"] == "system.capabilities" && f.doc.get("response").is_some())
        .expect("system.capabilities fixture");
    let caps = &snapshot.doc["response"]["result"];
    let _typed: dettivo_proto::capabilities::Capabilities = serde_json::from_value(caps.clone())
        .expect("capability snapshot carries every required flag");
    let mut required: BTreeMap<String, usize> = BTreeMap::new();
    for f in &all {
        for (path, expected) in f.doc["capabilities"].as_object().unwrap() {
            let mut cursor = caps;
            for segment in path.split('.') {
                cursor = cursor.get(segment).unwrap_or_else(|| {
                    panic!(
                        "{}: capability flag {path} is not in the snapshot",
                        f.path.display()
                    )
                });
            }
            let admitted = expected == &Value::Bool(true) && cursor == &Value::Bool(false);
            assert!(
                cursor == expected || admitted,
                "{}: {path} does not hold the value the fixture needs",
                f.path.display()
            );
            *required.entry(path.clone()).or_default() += 1;
        }
    }
    assert!(required.contains_key("automation.dictation_macros"));
    assert!(required.contains_key("speech.methods"));
}

/// A field the contract does not know is rejected with its name, and an
/// id that is not a lowercase UUID names the field too.
#[test]
fn drift_is_rejected_with_the_field_named() {
    let err = catalog::round_trip_params(
        "meetings.get",
        &serde_json::json!({"meeting_id": "0f8fad5b-d9cb-469f-a165-70867728950e", "extra": 1}),
    )
    .unwrap_err();
    assert!(err.to_string().contains("extra"), "{err}");
    let err =
        catalog::round_trip_params("meetings.get", &serde_json::json!({"meeting_id": "NOPE"}))
            .unwrap_err();
    assert!(err.to_string().contains("meeting_id"), "{err}");
}

/// Every event snapshot is one `events.notify` notification whose topic
/// names the file, and its payload round-trips byte-stable through the
/// topic's typed payload.
#[test]
fn event_snapshots_round_trip_their_typed_payloads() {
    use dettivo_proto::events::{
        AudioLevelPayload, DictationStatePayload, EngineStatePayload, MeetingSegmentPayload,
        ModelDownloadPayload, NOTIFY_METHOD, NotifyParams, Topic,
    };
    let snapshots = load_where(is_event_snapshot);
    assert!(
        !snapshots.is_empty(),
        "fixtures/events holds at least one *.event.json"
    );
    for f in &snapshots {
        assert_eq!(f.namespace, "events", "{}", f.path.display());
        let topic_name = f.stem.trim_end_matches(".event");
        let topic = Topic::parse(topic_name)
            .unwrap_or_else(|| panic!("{}: {topic_name} is not a topic", f.path.display()));
        assert_eq!(f.doc["topic"], topic_name, "{}", f.path.display());
        let notification = &f.doc["notification"];
        assert_eq!(notification["jsonrpc"], "2.0");
        assert_eq!(notification["method"], NOTIFY_METHOD);
        assert!(
            notification.get("id").is_none(),
            "a notification carries no id"
        );
        let params: NotifyParams = serde_json::from_value(notification["params"].clone())
            .unwrap_or_else(|e| panic!("{}: params: {e}", f.path.display()));
        assert_eq!(params.topic, topic);
        let payload = params.payload.clone();
        let back = match topic {
            Topic::MeetingSegment => serde_json::to_value(
                serde_json::from_value::<MeetingSegmentPayload>(payload.clone()).unwrap(),
            ),
            Topic::AudioLevel => serde_json::to_value(
                serde_json::from_value::<AudioLevelPayload>(payload.clone()).unwrap(),
            ),
            Topic::EngineState => serde_json::to_value(
                serde_json::from_value::<EngineStatePayload>(payload.clone()).unwrap(),
            ),
            Topic::ModelDownload => serde_json::to_value(
                serde_json::from_value::<ModelDownloadPayload>(payload.clone()).unwrap(),
            ),
            // The completion transition the pill and the Omarchy plugin
            // consume: `idle` from `inserting` with the insertion fields.
            Topic::DictationState => serde_json::to_value(
                serde_json::from_value::<DictationStatePayload>(payload.clone()).unwrap(),
            ),
            other => panic!(
                "{}: no typed payload for {}",
                f.path.display(),
                other.as_str()
            ),
        }
        .unwrap();
        assert_eq!(
            canonical(&back),
            canonical(&payload),
            "{}",
            f.path.display()
        );
        assert_eq!(
            canonical(&serde_json::to_value(&params).unwrap()),
            canonical(&notification["params"]),
            "{}",
            f.path.display()
        );
    }
}
