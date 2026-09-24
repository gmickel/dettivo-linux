//! Contract replay (R6): every fixture under `crates/dettivo-proto/fixtures`
//! sent to a live daemon as one line, the answer compared with the
//! fixture using the tolerant matching the fixture README states. A
//! fixture whose capability flag the daemon does not declare is skipped
//! and reported, never counted as passed; a fixture whose flag the daemon
//! declares `false` is still sent, and a `NOT_IMPLEMENTED` answer there is
//! a pending row the flag admits, which `--strict` accepts. A pending
//! answer behind a `true` flag is a broken promise and fails `--strict`
//! naming the flag and the method.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Values a server generates on the spot (fixtures/README.md).
pub const TOLERANT_KEYS: &[&str] = &[
    "job_id",
    "policy_hash",
    "transfer_id",
    "subscription_id",
    "expires_at",
    "build",
    "app_version",
    "uptime_seconds",
    "path",
    "latency_ms",
];

/// One fixture's verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The answer matched.
    Pass,
    /// The answer differed; `detail` says how.
    Fail,
    /// A required capability flag is not declared; `detail` names it.
    Skip,
    /// The daemon answered `NOT_IMPLEMENTED` for a method its spec has not
    /// landed yet; a failure under `--strict` (the release gate) unless
    /// the row's `admitted_by` names the flag the daemon declares `false`.
    Pending,
}

/// One row of the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// Fixture path relative to the fixtures root.
    pub fixture: String,
    /// The method.
    pub method: String,
    /// Pass, fail or skip.
    pub verdict: Verdict,
    /// Why, for a fail or skip.
    pub detail: Option<String>,
    /// The capability flag the daemon declares `false` for a pending row,
    /// which makes the gap an admitted one rather than a broken promise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admitted_by: Option<String>,
}

/// The whole report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Report {
    /// Rows in fixture order.
    pub rows: Vec<Row>,
    /// The MCP section: one row per harness step and framing (ADR 0019),
    /// empty when the server binary was not built.
    #[serde(default)]
    pub mcp: Vec<dettivo_mcp::harness::Row>,
    /// The REST section: one row per fixture (ADR 0028), empty when the
    /// daemon hosted no listener.
    #[serde(default)]
    pub rest: Vec<dettivo_rest::harness::Row>,
}

impl Report {
    /// Counts by verdict: (pass, fail, skip, pending).
    pub fn counts(&self) -> (usize, usize, usize, usize) {
        let mut c = (0, 0, 0, 0);
        for r in &self.rows {
            match r.verdict {
                Verdict::Pass => c.0 += 1,
                Verdict::Fail => c.1 += 1,
                Verdict::Skip => c.2 += 1,
                Verdict::Pending => c.3 += 1,
            }
        }
        c
    }

    /// True when no row failed, the MCP section included; under `strict`
    /// a pending row fails too, unless a `false` flag admits it.
    pub fn ok(&self, strict: bool) -> bool {
        self.rows.iter().all(|r| {
            r.verdict != Verdict::Fail
                && !(strict && r.verdict == Verdict::Pending && r.admitted_by.is_none())
        }) && self
            .mcp
            .iter()
            .all(|r| r.verdict != dettivo_mcp::harness::Verdict::Fail)
            && self
                .rest
                .iter()
                .all(|r| r.verdict != dettivo_rest::harness::Verdict::Fail)
    }

    /// The pending rows a `false` flag admits.
    pub fn admitted(&self) -> Vec<&Row> {
        self.rows
            .iter()
            .filter(|r| r.verdict == Verdict::Pending && r.admitted_by.is_some())
            .collect()
    }

    /// The pending rows behind a `true` flag or no flag: what `--strict`
    /// fails on, each naming its method and the flag that promised it.
    pub fn broken_promises(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter(|r| r.verdict == Verdict::Pending && r.admitted_by.is_none())
            .map(|r| {
                format!(
                    "{} ({})",
                    r.method,
                    r.detail.as_deref().unwrap_or("pending")
                )
            })
            .collect()
    }

    /// The human rendering: one line per fixture, then the totals.
    pub fn human(&self) -> String {
        let mut out = String::new();
        for r in &self.rows {
            let tag = match r.verdict {
                Verdict::Pass => "pass",
                Verdict::Fail => "FAIL",
                Verdict::Skip => "skip",
                Verdict::Pending => "pend",
            };
            out.push_str(&format!(
                "{tag:4}  {:<40} {}\n",
                r.fixture,
                r.detail.as_deref().unwrap_or("")
            ));
        }
        let (p, f, s, n) = self.counts();
        let admitted = self.admitted().len();
        out.push_str(&format!(
            "contract: {p} passed, {f} failed, {s} skipped, {n} pending (not implemented yet; {admitted} behind a false flag)\n"
        ));
        if !self.mcp.is_empty() {
            out.push_str(&dettivo_mcp::harness::human(&self.mcp));
        }
        if !self.rest.is_empty() {
            out.push_str(&dettivo_rest::harness::human(&self.rest));
        }
        out
    }
}

pub use crate::replay_flags::{Flags, flags};
pub use crate::socket::{answers_ping, call, call_within};

/// The fixtures root relative to a repository root.
pub fn fixtures_root(repo_root: &Path) -> PathBuf {
    repo_root.join("crates/dettivo-proto/fixtures")
}

fn load(root: &Path) -> std::io::Result<Vec<(String, Value)>> {
    let mut out = Vec::new();
    for ns in std::fs::read_dir(root)?.flatten() {
        if !ns.path().is_dir() {
            continue;
        }
        let mut files: Vec<PathBuf> = std::fs::read_dir(ns.path())?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            // Event snapshots (`<topic>.event.json`) pin a notification's
            // shape; there is no request to send.
            .filter(|p| {
                !p.file_stem()
                    .is_some_and(|s| s.to_string_lossy().ends_with(".event"))
            })
            .collect();
        files.sort();
        for path in files {
            let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            out.push((rel, doc));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn strip(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|k, _| !TOLERANT_KEYS.contains(&k.as_str()));
            map.values_mut().for_each(strip);
        }
        Value::Array(items) => items.iter_mut().for_each(strip),
        _ => {}
    }
}

pub use crate::replay_shape::{shape_of, shapes_agree};
pub use crate::stateful::STATEFUL;

/// Methods whose results describe the machine rather than the contract.
pub const MACHINE_SPECIFIC: &[&str] = &[
    "hotkeys.status",
    "llm.providers.list",
    "audio.devices",
    "speech.engines",
    "speech.providers.list",
    "speech.selection.get",
    "speech.selection.set",
    "speech.models.status",
    "speech.models.download",
    "speech.models.cancel",
    "speech.models.delete",
    "transcripts.stats",
];

/// Methods whose results describe what the store holds (the seeded rows
/// under the replay) rather than the contract; compared by shape.
pub const STORE_SPECIFIC: &[&str] = &["transcripts.list", "transcripts.search", "meetings.search"];

/// Compares only normalised machine/store subtrees by shape. The reply
/// envelope, errors and fixed capability values remain exact.
pub(crate) fn replies_agree(method: &str, want: &Value, got: &Value) -> bool {
    let paths: &[&str] = if method == "system.capabilities" {
        &["/result/hotkeys", "/result/rest"]
    } else if MACHINE_SPECIFIC.contains(&method) || STORE_SPECIFIC.contains(&method) {
        &["/result"]
    } else {
        &[]
    };
    let (mut want, mut got) = (want.clone(), got.clone());
    for path in paths {
        match (want.pointer_mut(path), got.pointer_mut(path)) {
            (Some(w), Some(g)) => {
                if !shapes_agree(w, g) {
                    return false;
                }
                *w = Value::Null;
                *g = Value::Null;
            }
            (None, None) => {}
            _ => return false,
        }
    }
    want == got
}

/// Method-aware normalisation: machine-specific blocks are levelled.
pub fn normalise(method: &str, mut value: Value) -> Value {
    if method == "system.capabilities" {
        if let Some(platform) = value["result"].get_mut("platform") {
            *platform = json!({"os": platform["os"].clone()});
        }
        if let Some(hotkeys) = value["result"].get_mut("hotkeys") {
            *hotkeys = shape_of(hotkeys);
        }
        // The REST block is the listener's state on this daemon (ADR 0028).
        if let Some(rest) = value["result"].get_mut("rest") {
            *rest = shape_of(rest);
        }
    }
    if method == "config.path" {
        if let Some(result) = value["result"].as_object_mut() {
            result.values_mut().for_each(|v| *v = Value::Null);
        }
    }
    if MACHINE_SPECIFIC.contains(&method) || STORE_SPECIFIC.contains(&method) {
        // These results describe the machine (its devices, engines, models
        // and selection); the replay compares the type shape of the result
        // instead of the values.
        if let Some(result) = value.get_mut("result") {
            *result = shape_of(result);
        }
    }
    strip(&mut value);
    value
}

/// Replays every fixture against `socket`.
pub fn run(repo_root: &Path, socket: &Path) -> std::io::Result<Report> {
    let caps = call(
        socket,
        r#"{"jsonrpc":"2.0","id":"caps","method":"system.capabilities","params":{}}"#,
    )?["result"]
        .clone();
    let mut report = Report::default();
    for (rel, doc) in load(&fixtures_root(repo_root))? {
        let method = doc["method"].as_str().unwrap_or("").to_string();
        let expected = doc
            .get("response")
            .or(doc.get("error"))
            .cloned()
            .unwrap_or(Value::Null);
        if expected["error"]["data"]["app_code"] == "UNAUTHORIZED_CLIENT" {
            report.rows.push(Row {
                fixture: rel,
                method,
                verdict: Verdict::Skip,
                detail: Some("needs a connection from another uid".into()),
                admitted_by: None,
            });
            continue;
        }
        let (admitted_by, promised) = match flags(&doc, &caps) {
            Flags::Missing(why) => {
                report.rows.push(Row {
                    fixture: rel,
                    method,
                    verdict: Verdict::Skip,
                    detail: Some(why),
                    admitted_by: None,
                });
                continue;
            }
            Flags::Admitted(flag) => (Some(flag), Vec::new()),
            Flags::Declared { promised } => (None, promised),
        };
        if let Some((_, why)) = STATEFUL.iter().find(|(name, _)| *name == rel) {
            report.rows.push(Row {
                fixture: rel,
                method,
                verdict: Verdict::Skip,
                detail: Some(format!("{why}; covered by the daemon's tests")),
                admitted_by: None,
            });
            continue;
        }
        let actual = match call(socket, &doc["request"].to_string()) {
            Ok(a) => a,
            Err(e) => {
                report.rows.push(Row {
                    fixture: rel,
                    method,
                    verdict: Verdict::Fail,
                    detail: Some(format!("transport: {e}")),
                    admitted_by: None,
                });
                continue;
            }
        };
        let (got, want) = (
            normalise(&method, actual.clone()),
            normalise(&method, expected.clone()),
        );
        let same = if method == "system.diagnostics" {
            crate::replay_diagnostics::agrees(&expected, &actual, |probe| {
                call(
                    socket,
                    &json!({"jsonrpc":"2.0", "id":"probe", "method":probe, "params":{}})
                        .to_string(),
                )
                .ok()
            })
        } else {
            replies_agree(&method, &want, &got)
        };
        if same {
            report.rows.push(Row {
                fixture: rel,
                method,
                verdict: Verdict::Pass,
                detail: None,
                admitted_by: None,
            });
        } else if actual["error"]["data"]["app_code"] == "NOT_IMPLEMENTED"
            && expected["error"]["data"]["app_code"] != "NOT_IMPLEMENTED"
        {
            let detail = match (&admitted_by, promised.first()) {
                (Some(flag), _) => format!("not implemented yet; {flag} = false admits it"),
                (None, Some(flag)) => {
                    format!(
                        "not implemented yet behind {flag} = true (the daemon promises {method})"
                    )
                }
                (None, None) => "not implemented yet".into(),
            };
            report.rows.push(Row {
                fixture: rel,
                method,
                verdict: Verdict::Pending,
                detail: Some(detail),
                admitted_by,
            });
        } else {
            report.rows.push(Row {
                fixture: rel,
                method,
                verdict: Verdict::Fail,
                detail: Some(format!("expected {want} got {got}")),
                admitted_by: None,
            });
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalisation_levels_tolerant_and_machine_values() {
        let a = json!({"result": {"build": "abc", "ok": true, "platform": {"os": "linux", "gpu": "vulkan"}}});
        let b = json!({"result": {"build": "xyz", "ok": true, "platform": {"os": "linux", "gpu": null}}});
        assert_eq!(
            normalise("system.capabilities", a),
            normalise("system.capabilities", b)
        );
        let c = json!({"result": {"config": "/a", "socket": "/b"}});
        let d = json!({"result": {"config": "/x", "socket": "/y"}});
        assert_eq!(normalise("config.path", c), normalise("config.path", d));
    }

    #[test]
    fn stateful_fixtures_exist() {
        let root = fixtures_root(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .as_path(),
        );
        for (name, _) in STATEFUL {
            assert!(root.join(name).is_file(), "{name} is not a fixture");
        }
    }

    #[test]
    fn report_counts_and_renders() {
        let report = Report {
            rows: vec![
                Row {
                    fixture: "a".into(),
                    method: "m".into(),
                    verdict: Verdict::Pass,
                    detail: None,
                    admitted_by: None,
                },
                Row {
                    fixture: "b".into(),
                    method: "m".into(),
                    verdict: Verdict::Skip,
                    detail: Some("x".into()),
                    admitted_by: None,
                },
            ],
            mcp: Vec::new(),
            rest: Vec::new(),
        };
        assert_eq!(report.counts(), (1, 0, 1, 0));
        assert!(report.ok(true));
        assert!(report.human().contains("1 passed, 0 failed, 1 skipped"));
    }
}
