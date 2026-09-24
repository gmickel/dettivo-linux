//! The fixtures harness: every file under `fixtures/<set>/` is one HTTP
//! request and the answer it must get, replayed against a live shim in
//! front of a seeded daemon. The crate's own test, `dettivo-qa rest` and
//! the contract replay run the same rows. Sets run in a fixed order
//! (`routes`, `auth`, `errors`, `stream`, `vscode`) so the export of the
//! seed precedes the import that adds an item.
//!
//! A fixture:
//!
//! ```json
//! {
//!   "request": {"method": "GET", "path": "/v1/system/health", "auth": "bearer",
//!               "headers": {}, "body": {}},
//!   "expect": {"status": 200, "content_type": "application/json", "body": {...}}
//! }
//! ```
//!
//! `auth` is `bearer` (default), `header` (`X-Dettivo-Token`), `wrong`
//! or `none`. `body` is a JSON value, `body_text` a string, `body_file`
//! a path relative to the fixtures root or `$AUDIO` for the harness's
//! clip; `declared_length` sends that `Content-Length` with no bytes.
//! `expect.body` is compared after the volatile keys are stripped,
//! `body_keys` checks the keys only, `body_file` the exact bytes, and
//! `error` the `app_code` and `code` of an error body, `array_len` the
//! length of a named array. `needs: "audio"`
//! skips the fixture with the reason when no clip is available;
//! `after: "settle_and_delete"` waits for the imported item and removes
//! it so the seed stays the newest row.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::client::{self, Reply};

/// The sets, in the order they run.
pub const SETS: &[&str] = &["routes", "auth", "errors", "stream", "vscode"];

/// A row's error starting with this is a skip, not a failure.
pub const SKIP: &str = "skip: ";

/// The fixtures directory of this crate.
pub fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// Keys a live server generates on the spot, stripped before comparing.
pub const TOLERANT_KEYS: &[&str] = &[
    "job_id",
    "policy_hash",
    "transfer_id",
    "expires_at",
    "build",
    "app_version",
    "uptime_seconds",
    "path",
    "latency_ms",
    "platform",
    "port",
    "created_at",
    "updated_at",
    "started_at",
    "ended_at",
    "id",
];

/// One step's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The answer matched.
    Pass,
    /// It did not; `detail` says how.
    Fail,
    /// The fixture needs something this machine lacks; never a pass.
    Skip,
}

/// One row of the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// `<set>/<file>`.
    pub fixture: String,
    /// The verdict.
    pub verdict: Verdict,
    /// Why, for a failure or a skip.
    pub detail: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Fixture {
    request: FixtureRequest,
    expect: Expect,
    #[serde(default)]
    needs: Option<String>,
    #[serde(default)]
    after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FixtureRequest {
    method: String,
    path: String,
    #[serde(default)]
    auth: Option<String>,
    #[serde(default)]
    headers: serde_json::Map<String, Value>,
    #[serde(default)]
    body: Option<Value>,
    #[serde(default)]
    body_text: Option<String>,
    #[serde(default)]
    body_file: Option<String>,
    #[serde(default)]
    declared_length: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct Expect {
    status: u16,
    #[serde(default)]
    content_type: Option<String>,
    #[serde(default)]
    headers: serde_json::Map<String, Value>,
    #[serde(default)]
    body: Option<Value>,
    #[serde(default)]
    body_keys: Option<Vec<String>>,
    #[serde(default)]
    body_file: Option<String>,
    #[serde(default)]
    array_len: serde_json::Map<String, Value>,
    #[serde(default)]
    error: Option<Value>,
}

/// What the harness runs against.
pub struct Target {
    /// The shim's address.
    pub addr: SocketAddr,
    /// The token the shim expects.
    pub token: String,
    /// The audio clip for `$AUDIO`, when this machine has one.
    pub audio: Option<PathBuf>,
}

impl Target {
    fn auth_headers(&self, auth: Option<&str>) -> Vec<(String, String)> {
        match auth.unwrap_or("bearer") {
            "header" => vec![("X-Dettivo-Token".into(), self.token.clone())],
            "wrong" => vec![("Authorization".into(), format!("Bearer {}x", self.token))],
            "none" => vec![],
            _ => vec![("Authorization".into(), format!("Bearer {}", self.token))],
        }
    }

    /// One request with the bearer token.
    pub fn call(&self, method: &str, target: &str, body: Option<&Value>) -> Result<Reply, String> {
        let mut headers = self.auth_headers(None);
        if body.is_some() {
            headers.push(("Content-Type".into(), "application/json".into()));
        }
        let bytes = body.map(|b| b.to_string().into_bytes()).unwrap_or_default();
        client::request(
            self.addr,
            method,
            target,
            &headers,
            &bytes,
            None,
            Duration::from_secs(60),
        )
    }
}

/// Runs every fixture under `root` against `target`, one row each.
pub fn run(target: &Target, root: &Path) -> Vec<Row> {
    let mut rows = Vec::new();
    for set in SETS {
        let dir = root.join(set);
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|d| d.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        files.retain(|p| p.extension().is_some_and(|e| e == "json"));
        files.sort();
        for file in files {
            let name = format!(
                "{set}/{}",
                file.file_name().unwrap_or_default().to_string_lossy()
            );
            let outcome = run_one(target, root, &file);
            let verdict = match &outcome {
                Ok(()) => Verdict::Pass,
                Err(e) if e.starts_with(SKIP) => Verdict::Skip,
                Err(_) => Verdict::Fail,
            };
            rows.push(Row {
                fixture: name,
                verdict,
                detail: outcome
                    .err()
                    .map(|e| e.strip_prefix(SKIP).map(str::to_string).unwrap_or(e)),
            });
        }
    }
    rows
}

fn run_one(target: &Target, root: &Path, file: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("read: {e}"))?;
    let fixture: Fixture = serde_json::from_str(&text).map_err(|e| format!("fixture: {e}"))?;
    if fixture.needs.as_deref() == Some("audio") && target.audio.is_none() {
        return Err(format!(
            "{SKIP}no test model or clip on this machine (scripts/models/fetch-test-model.sh)"
        ));
    }
    let req = &fixture.request;
    let mut headers = target.auth_headers(req.auth.as_deref());
    for (k, v) in &req.headers {
        headers.push((k.clone(), v.as_str().unwrap_or_default().to_string()));
    }
    let body: Vec<u8> = if let Some(b) = &req.body {
        if !headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        {
            headers.push(("Content-Type".into(), "application/json".into()));
        }
        b.to_string().into_bytes()
    } else if let Some(t) = &req.body_text {
        t.clone().into_bytes()
    } else if let Some(f) = &req.body_file {
        let path = if f == "$AUDIO" {
            target.audio.clone().ok_or("no audio clip")?
        } else {
            root.join(f)
        };
        std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?
    } else {
        Vec::new()
    };
    let reply = client::request(
        target.addr,
        &req.method,
        &req.path,
        &headers,
        &body,
        req.declared_length,
        Duration::from_secs(180),
    )?;
    check(&fixture.expect, &reply, root)?;
    if fixture.after.as_deref() == Some("settle_and_delete") {
        settle_and_delete(target, &reply)?;
    }
    Ok(())
}

fn check(expect: &Expect, reply: &Reply, root: &Path) -> Result<(), String> {
    if reply.status != expect.status {
        return Err(format!(
            "status {} (want {}): {}",
            reply.status,
            expect.status,
            String::from_utf8_lossy(&reply.body)
                .chars()
                .take(300)
                .collect::<String>()
        ));
    }
    if let Some(ct) = &expect.content_type {
        let got = reply.header("content-type").unwrap_or("");
        if !got.starts_with(ct.as_str()) {
            return Err(format!("content-type {got:?} (want {ct:?})"));
        }
    }
    for (k, v) in &expect.headers {
        let got = reply.header(&k.to_ascii_lowercase()).unwrap_or("");
        if got != v.as_str().unwrap_or_default() {
            return Err(format!("header {k}: {got:?} (want {v})"));
        }
    }
    if let Some(file) = &expect.body_file {
        let want = std::fs::read(root.join(file)).map_err(|e| format!("read {file}: {e}"))?;
        if reply.body != want {
            return Err(format!(
                "body differs from {file}: {}",
                String::from_utf8_lossy(&reply.body)
                    .chars()
                    .take(200)
                    .collect::<String>()
            ));
        }
    }
    if expect.body.is_some() || expect.body_keys.is_some() || expect.error.is_some() {
        let got = reply
            .json()
            .ok_or_else(|| format!("body is not JSON: {}", String::from_utf8_lossy(&reply.body)))?;
        if let Some(want) = &expect.body {
            let (mut g, mut w) = (got.clone(), want.clone());
            strip(&mut g);
            strip(&mut w);
            if g != w {
                return Err(format!("body {g} (want {w})"));
            }
        }
        if let Some(keys) = &expect.body_keys {
            let object = got.as_object().ok_or("body is not an object")?;
            for k in keys {
                if !object.contains_key(k) {
                    return Err(format!("body lacks {k}: {got}"));
                }
            }
        }
        for (k, want) in &expect.array_len {
            let len = got[k].as_array().map(Vec::len).unwrap_or(usize::MAX);
            if want.as_u64() != u64::try_from(len).ok() {
                return Err(format!("{k} has {len} items (want {want})"));
            }
        }
        if let Some(error) = &expect.error {
            let e = &got["error"];
            if e["data"]["app_code"] != error["app_code"] || e["code"] != error["code"] {
                return Err(format!("error {e} (want {error})"));
            }
        }
    }
    Ok(())
}

/// Removes the volatile keys everywhere.
pub fn strip(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|k, _| !TOLERANT_KEYS.contains(&k.as_str()));
            map.values_mut().for_each(strip);
        }
        Value::Array(items) => items.iter_mut().for_each(strip),
        _ => {}
    }
}

/// Waits for the imported item to leave `transcribing`, checks it kept
/// text, then deletes it through the shim.
fn settle_and_delete(target: &Target, reply: &Reply) -> Result<(), String> {
    let id = reply
        .json()
        .and_then(|v| v["ref"]["id"].as_str().map(str::to_string));
    let Some(id) = id else {
        return Err("import result carries no ref.id".into());
    };
    let reference = json!({"ref": {"kind": "dictation", "id": id}});
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        let listed = target
            .call(
                "POST",
                "/v1/transcripts/list",
                Some(&json!({"kinds": ["dictation"], "limit": 50})),
            )?
            .json()
            .ok_or("transcripts.list answered no JSON")?;
        let row = listed["items"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| r["ref"]["id"] == id)
            .cloned()
            .ok_or_else(|| format!("{id} not listed"))?;
        if row["status"] != "transcribing" {
            break;
        }
        if Instant::now() > deadline {
            return Err(format!("{id} still transcribing after 180 s"));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let item = target
        .call("POST", "/v1/transcripts/get", Some(&reference))?
        .json()
        .ok_or("transcripts.get answered no JSON")?;
    let text = item["text_raw"].as_str().unwrap_or_default().to_string();
    let deleted = target.call("POST", "/v1/transcripts/delete", Some(&reference))?;
    if deleted.status != 200 {
        return Err(format!(
            "delete of {id} answered {}: {}",
            deleted.status,
            String::from_utf8_lossy(&deleted.body)
        ));
    }
    if text.trim().is_empty() {
        return Err(format!("{id} imported with no text"));
    }
    Ok(())
}

/// The human report: one line per row, then the totals.
pub fn human(rows: &[Row]) -> String {
    let mut out = String::new();
    for r in rows {
        let tag = match r.verdict {
            Verdict::Pass => "pass",
            Verdict::Fail => "FAIL",
            Verdict::Skip => "skip",
        };
        out.push_str(&format!(
            "{tag}  {:<40} {}\n",
            r.fixture,
            r.detail.as_deref().unwrap_or("")
        ));
    }
    let failed = rows.iter().filter(|r| r.verdict == Verdict::Fail).count();
    let skipped = rows.iter().filter(|r| r.verdict == Verdict::Skip).count();
    out.push_str(&format!(
        "rest: {} passed, {failed} failed, {skipped} skipped\n",
        rows.len() - failed - skipped
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fixture_parses_and_names_a_known_set() {
        let root = fixtures_root();
        let mut count = 0;
        for set in SETS {
            for entry in std::fs::read_dir(root.join(set)).unwrap().flatten() {
                let text = std::fs::read_to_string(entry.path()).unwrap();
                let fixture: Fixture = serde_json::from_str(&text)
                    .unwrap_or_else(|e| panic!("{}: {e}", entry.path().display()));
                assert!(fixture.request.path.starts_with('/'), "{:?}", entry.path());
                count += 1;
            }
        }
        assert!(count >= 20, "{count} fixtures");
        let rows = vec![Row {
            fixture: "routes/x.json".into(),
            verdict: Verdict::Fail,
            detail: Some("x".into()),
        }];
        assert!(human(&rows).ends_with("rest: 0 passed, 1 failed, 0 skipped\n"));
    }
}
