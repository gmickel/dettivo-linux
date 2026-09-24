//! The complete contract run shared by the command line and release pack.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::{
    profile::{ModelSource, Profile},
    replay,
    scenarios::{self, daemon::DaemonHandle},
};

/// Authentication shared by the fixture clients and daemon.
pub const REST_TOKEN: &str = "dettivo-qa-rest-token";

/// Runs MCP, REST and socket fixtures once, in their existing mutation order.
pub fn run(
    repo: &Path,
    models: Option<PathBuf>,
    timeout: Duration,
) -> Result<replay::Report, String> {
    let binary = scenarios::binary(repo, "dettivod")?;
    let server = scenarios::binary(repo, "dettivo-mcp")?;
    let source = models.map(|real| ModelSource::new(real, repo));
    let mut profile = Profile::create("contract", source.as_ref()).map_err(|e| e.to_string())?;
    let env = BTreeMap::from([
        ("DETTIVO_E2E_SEED".into(), "1".into()),
        ("DETTIVO_MOCK_A11Y".into(), "1".into()),
        ("DETTIVO_IPC_TOKEN".into(), REST_TOKEN.into()),
    ]);
    let daemon = DaemonHandle::spawn(
        &binary,
        &mut profile,
        "[rest]\nenabled = true\nport = 0\n",
        &env,
        timeout,
    )?;
    let fixture = super::scenarios::daemon::local_model(&profile).map(|(_, wav)| wav);
    let mcp = harness_with_test_model(&server, &daemon.socket, fixture.as_deref(), true);
    let selected = fixture.is_some() && config_key(&daemon.socket, "speech.model", Some("tiny.en"));
    let rest = rest_rows(repo, &daemon.socket, fixture.as_deref());
    if selected {
        config_key(&daemon.socket, "speech.model", None);
    }
    let mut report = replay::run(repo, &daemon.socket).map_err(|e| e.to_string())?;
    report.mcp = mcp;
    report.rest = rest;
    validate_required(&report)?;
    Ok(report)
}

/// Validates the transport sections required by a fresh-profile contract run.
pub fn validate_required(report: &replay::Report) -> Result<(), String> {
    if report.rows.is_empty() {
        return Err("required socket contract section is empty".into());
    }
    for framing in ["line-delimited", "content-length"] {
        if !report.mcp.iter().any(|r| r.framing == framing) {
            return Err(format!("required MCP {framing} contract section is empty"));
        }
    }
    if report.rest.is_empty() {
        return Err("required REST contract section is empty".into());
    }
    Ok(())
}

/// Sets one config key on the daemon; `None` unsets it, putting the key
/// back on its default.
pub fn config_key(socket: &Path, key: &str, value: Option<&str>) -> bool {
    let line = match value {
        Some(v) => format!(
            r#"{{"jsonrpc":"2.0","id":"qa-cfg","method":"config.set","params":{{"key":"{key}","value":"{v}"}}}}"#
        ),
        None => format!(
            r#"{{"jsonrpc":"2.0","id":"qa-cfg","method":"config.unset","params":{{"key":"{key}"}}}}"#
        ),
    };
    replay::call(socket, &line).is_ok_and(|a| a.get("result").is_some())
}

/// Runs the MCP harness with the test model selected on our own daemon,
/// then puts `speech.model` back on its default. The live `import_audio`
/// step transcribes through the selected model, and the
/// `speech.models.delete` fixtures that replay afterwards read the
/// default selection back. A caller's daemon keeps its own selection.
pub fn harness_with_test_model(
    server: &Path,
    socket: &Path,
    fixture: Option<&Path>,
    own_daemon: bool,
) -> Vec<dettivo_mcp::harness::Row> {
    let selected =
        fixture.is_some() && own_daemon && config_key(socket, "speech.model", Some("tiny.en"));
    let rows = dettivo_mcp::harness::run_both_with(server, socket, fixture);
    if selected {
        config_key(socket, "speech.model", None);
    }
    rows
}

/// Runs the REST harness against the shim the daemon on `socket` hosts:
/// the port from `system.capabilities.rest`, the token the runner set.
/// A daemon without a listener (a caller's socket) yields no rows.
pub fn rest_rows(
    repo: &Path,
    socket: &Path,
    fixture: Option<&Path>,
) -> Vec<dettivo_rest::harness::Row> {
    let line = r#"{"jsonrpc":"2.0","id":"qa-rest","method":"system.capabilities","params":{}}"#;
    let caps = match replay::call(socket, line) {
        Ok(v) => v["result"]["rest"].clone(),
        Err(_) => return Vec::new(),
    };
    if caps["enabled"] != serde_json::Value::Bool(true) {
        return Vec::new();
    }
    let Some(port) = caps["port"].as_u64().and_then(|p| u16::try_from(p).ok()) else {
        return Vec::new();
    };
    let target = dettivo_rest::harness::Target {
        addr: std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        token: REST_TOKEN.into(),
        audio: fixture.map(Path::to_path_buf),
    };
    dettivo_rest::harness::run(&target, &repo.join("crates/dettivo-rest/fixtures"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_transports_cannot_pass_without_evidence() {
        assert!(validate_required(&replay::Report::default()).is_err());
    }
    #[test]
    fn each_required_section_and_failed_rest_result_affect_the_shared_gate() {
        let complete = replay::Report {
            rows: vec![replay::Row {
                fixture: "system/health.json".into(),
                method: "system.health".into(),
                verdict: replay::Verdict::Pass,
                detail: None,
                admitted_by: None,
            }],
            mcp: ["line-delimited", "content-length"]
                .iter()
                .map(|framing| dettivo_mcp::harness::Row {
                    step: "initialize".into(),
                    framing: (*framing).into(),
                    verdict: dettivo_mcp::harness::Verdict::Pass,
                    detail: None,
                })
                .collect(),
            rest: vec![dettivo_rest::harness::Row {
                fixture: "health".into(),
                verdict: dettivo_rest::harness::Verdict::Pass,
                detail: None,
            }],
        };
        assert!(validate_required(&complete).is_ok());
        assert!(complete.ok(true));
        for section in ["socket", "line-delimited", "content-length", "REST"] {
            let mut report = complete.clone();
            match section {
                "socket" => report.rows.clear(),
                "REST" => report.rest.clear(),
                framing => report.mcp.retain(|r| r.framing != framing),
            }
            assert!(validate_required(&report).unwrap_err().contains(section));
        }
        let mut broken = complete;
        broken.rest[0].verdict = dettivo_rest::harness::Verdict::Fail;
        broken.rest[0].detail = Some("response body differs".into());
        assert!(validate_required(&broken).is_ok());
        assert!(!broken.ok(true));
    }
}
