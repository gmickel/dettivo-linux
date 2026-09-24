//! `system.ping`, `system.health`, `system.version`, `system.capabilities`.

use dettivo_proto::error::JsonRpcError;
use dettivo_proto::methods::system::{HealthResult, PingResult, RecordingState, VersionResult};
use serde_json::Value;

use super::json;

use crate::daemon::Daemon;
use crate::platform;

/// The build identifier baked in at compile time (`DETTIVO_BUILD`), or
/// `dev` for a local build.
pub const BUILD: &str = match option_env!("DETTIVO_BUILD") {
    Some(b) => b,
    None => "dev",
};

/// `system.ping`.
pub fn ping() -> Result<Value, JsonRpcError> {
    json(PingResult { ok: true })
}

/// `system.health`: `ok` is false while the configuration file does not
/// parse (the daemon runs on defaults and `config.validate` names why).
pub fn health(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let active = daemon.dictation().snapshot().is_some();
    let meeting = daemon.meetings().snapshot().is_some();
    let jobs =
        u32::try_from(daemon.jobs().active() + daemon.diarization().active()).unwrap_or(u32::MAX);
    json(HealthResult {
        ok: daemon.config_ok(),
        recording_state: if active {
            RecordingState::Dictation
        } else if meeting {
            RecordingState::Meeting
        } else {
            RecordingState::Idle
        },
        active_jobs: u32::from(active || meeting).saturating_add(jobs),
        uptime_seconds: daemon.uptime_seconds(),
    })
}

/// `system.version`.
pub fn version() -> Result<Value, JsonRpcError> {
    json(VersionResult {
        api_version: dettivo_proto::API_VERSION.to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        build: BUILD.to_string(),
    })
}

/// `system.capabilities`, reporting the auth mode in force and the
/// insertion backend the chain would use right now.
pub fn capabilities(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let settings = daemon.insert_settings();
    let backend = daemon.insert.chosen_backend(&settings);
    let loaded = daemon.config();
    let hotkeys = daemon.hotkeys().capabilities(&loaded);
    let local_llm = daemon.engines().local_llm().available();
    let rest = daemon.rest().caps(&loaded);
    let tier = daemon.engines().tier();
    json(platform::capabilities(
        daemon.auth_mode(),
        backend,
        hotkeys,
        local_llm,
        rest,
        &tier,
    ))
}

/// `system.diagnostics`: one owner for the daemon probes used by Doctor.
pub fn diagnostics(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    use super::{audio, config, hotkeys, insert, llm, llm_models, speech, transcripts};
    use dettivo_proto::methods::system::{DiagnosticProbe, DiagnosticsResult, ProbeState};
    let empty = || serde_json::json!({});
    let probes = [
        ("system.version", version()),
        ("system.health", health(daemon)),
        ("system.capabilities", capabilities(daemon)),
        ("insert.target", insert::target(daemon)),
        ("hotkeys.status", hotkeys::status(daemon)),
        ("audio.devices", audio::devices(daemon)),
        ("config.validate", config::validate(daemon)),
        ("speech.engines", speech::engines(daemon)),
        (
            "speech.models.status",
            speech::models_status(daemon, empty()),
        ),
        ("llm.providers.list", llm::providers_list(daemon)),
        ("llm.engine.status", llm_models::engine_status(daemon)),
        (
            "llm.models.status",
            llm_models::models_status(daemon, empty()),
        ),
        ("transcripts.stats", transcripts::stats(daemon)),
    ]
    .into_iter()
    .map(|(method, result)| {
        let probe = match result {
            Ok(value) => DiagnosticProbe {
                state: ProbeState::Success,
                result: Some(value),
                error: None,
            },
            Err(error) => DiagnosticProbe {
                state: ProbeState::Failure,
                result: None,
                error: Some(error),
            },
        };
        (method.to_string(), probe)
    })
    .collect();
    json(DiagnosticsResult { probes })
}
