//! `insert.*`: `perform` (`docs/api/dettivo-ipc-v1.md` section 8.5) plus
//! the Linux additions `undo` and `target` (`docs/api/linux-deltas.md`).

use crate::capabilities::InsertMode;
use crate::runtime::{InsertionResult, TranscriptRef};
use serde::{Deserialize, Serialize};

/// `insert.perform` params. If both `text` and `source_ref` are provided,
/// `text` wins. If the guard fields mismatch the frontmost target, the
/// method returns `CONFLICT`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerformParams {
    /// Insertion mode.
    pub mode: InsertMode,
    /// Literal text to insert; optional when `source_ref` is provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Source item to insert from, when `text` is not given directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<TranscriptRef>,
    /// Optional guard: expected frontmost app bundle id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_target_bundle_id: Option<String>,
    /// Optional guard: expected frontmost app pid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_target_pid: Option<String>,
    /// Linux addition: let this insertion land in one of Dettivo's own
    /// windows. Honoured only while `insert.allow_self_target` is armed on
    /// a connection (the app's first-run Try it step); otherwise the
    /// chain's `target_is_self` refusal stands.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_self_target: bool,
}

/// `insert.allow_self_target` params (Linux addition): arm or disarm the
/// self-target allowance on this connection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllowSelfTargetParams {
    /// True arms it, false disarms it.
    pub enabled: bool,
}

/// `insert.allow_self_target` result: whether the allowance is armed now
/// and which app ids it covers (the app's own, never the pill's).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllowSelfTargetResult {
    /// True while armed.
    pub enabled: bool,
    /// The app ids an insertion may land in while armed.
    pub app_ids: Vec<String>,
}

/// `insert.perform` result.
pub type PerformResult = InsertionResult;

/// `insert.undo` result (Linux addition): whether the last insertion was
/// taken back, and why not otherwise (`nothing_to_undo`, `expired`,
/// `unsupported_backend`, `target_changed`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UndoResult {
    /// True when the inserted text was removed again.
    pub undone: bool,
    /// Reason code when `undone` is false.
    pub reason: Option<String>,
}

/// The focused window as the insertion chain sees it (Linux addition).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// The Wayland app id or X11 window class.
    pub app_id: String,
    /// The owning process id when the compositor reports one.
    pub pid: Option<u32>,
    /// A stable hash of the window title; the title itself never leaves
    /// the daemon.
    pub title_hash: String,
    /// True for one of Dettivo's own windows (`insert.self_app_ids`).
    pub is_dettivo: bool,
    /// True for an X client under XWayland: a Wayland virtual keyboard's
    /// keymap does not reach it, so the chain types through X11.
    pub xwayland: bool,
}

/// One backend of the chain with its availability in this session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendStatus {
    /// Backend name.
    pub name: String,
    /// True when the backend can run in this session right now.
    pub available: bool,
    /// Why not, when `available` is false.
    pub reason: Option<String>,
}

/// `insert.target` result (Linux addition): the focused window, how it was
/// probed, and the chain's availability so the OSD and `dettivo doctor`
/// can explain what an insertion would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetResult {
    /// The focused window, `None` when no probe can see it.
    pub target: Option<Target>,
    /// The probe that answered (`hyprland`, `x11`, `mock`), or `none`.
    pub probe: String,
    /// Every backend in chain order with its availability.
    pub backends: Vec<BackendStatus>,
    /// The backend the chain would use right now, `None` when nothing is
    /// available.
    pub chosen: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn target_result_round_trips() {
        let v = json!({
            "target": {"app_id": "foot", "pid": 12, "title_hash": "ab12", "is_dettivo": false, "xwayland": false},
            "probe": "hyprland",
            "backends": [{"name": "virtual_keyboard", "available": true, "reason": null}],
            "chosen": "virtual_keyboard"
        });
        let r: TargetResult = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(r.chosen.as_deref(), Some("virtual_keyboard"));
        assert_eq!(serde_json::to_value(&r).unwrap(), v);
    }

    /// No probe detects a password field, so the target carries no claim
    /// about one: a `secure_field` bit is not part of the contract.
    #[test]
    fn a_target_makes_no_secure_field_claim() {
        let with_bit = json!({"app_id": "foot", "pid": 12, "title_hash": "ab12", "is_dettivo": false, "xwayland": false, "secure_field": false});
        assert!(serde_json::from_value::<Target>(with_bit).is_err());
    }
}
