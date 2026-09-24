//! `system.*`: `ping`, `health`, `version`, `capabilities`
//! (`docs/api/dettivo-ipc-v1.md` section 8.1).

use crate::capabilities::Capabilities;
use serde::{Deserialize, Serialize};

/// `system.ping` result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PingResult {
    /// Always `true`.
    pub ok: bool,
}

/// Coarse recording activity for `system.health`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingState {
    /// Neither dictation nor a meeting is active.
    Idle,
    /// A dictation session is active.
    Dictation,
    /// A meeting session is active.
    Meeting,
}

/// `system.health` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthResult {
    /// Always `true` when the daemon can answer at all.
    pub ok: bool,
    /// Coarse recording activity.
    pub recording_state: RecordingState,
    /// Number of active background jobs.
    pub active_jobs: u32,
    /// Daemon process uptime in seconds.
    pub uptime_seconds: u64,
}

/// `system.version` result (section 6.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionResult {
    /// Contract/API version.
    pub api_version: String,
    /// Application/daemon build version.
    pub app_version: String,
    /// Build identifier, e.g. a short commit hash.
    pub build: String,
}

/// `system.capabilities` result (section 6.2, extended with Linux
/// additions — see [`Capabilities`]).
pub type CapabilitiesResult = Capabilities;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_result_round_trips() {
        let json = serde_json::to_string(&PingResult { ok: true }).unwrap();
        assert_eq!(json, "{\"ok\":true}");
    }
}

/// Transport outcome of a required diagnostic probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeState {
    /// The probe returned its result (which may itself report unhealthy).
    Success,
    /// The probe returned a structured error.
    Failure,
    /// No conclusive probe result was available.
    Unknown,
}

/// One daemon-owned diagnostic observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticProbe {
    /// Whether the query completed.
    pub state: ProbeState,
    /// Original method result, when successful.
    pub result: Option<serde_json::Value>,
    /// Original failure, without losing its error class.
    pub error: Option<crate::error::JsonRpcError>,
}

/// Daemon-owned probes keyed by their method names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticsResult {
    /// Each required query has an explicit outcome.
    pub probes: std::collections::BTreeMap<String, DiagnosticProbe>,
}
