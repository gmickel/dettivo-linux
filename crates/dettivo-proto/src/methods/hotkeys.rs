//! `hotkeys.status`, `hotkeys.snippet` and `hotkeys.setup` (Linux
//! additions, `docs/api/linux-deltas.md`): which daemon-side hotkey backend
//! runs, whether the portal and evdev paths are available and why not,
//! which actions are bound and when a key last reached the daemon; the
//! compositor snippet `dettivo setup` writes, rendered by the daemon so
//! the first-run Keys step shows the same text; and the write of that
//! snippet with the check `--check` runs.

use serde::{Deserialize, Serialize};

/// One backend's availability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendAvailability {
    /// True when the backend can run in this session.
    pub available: bool,
    /// Why not, when `available` is false.
    pub reason: Option<String>,
}

/// `hotkeys.status` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusResult {
    /// The backend delivering presses to the daemon: `portal`, `evdev` or
    /// `none` (compositor bindings call the CLI and need none).
    pub backend: String,
    /// The `[hotkeys] backend` setting in force.
    pub requested: String,
    /// The portal's GlobalShortcuts interface.
    pub portal: BackendAvailability,
    /// The evdev path; its reason names the `input` group when devices
    /// cannot be read.
    pub evdev: BackendAvailability,
    /// The action ids the active backend bound.
    pub bound: Vec<String>,
    /// Why the requested backend is not running, when it is not.
    pub error: Option<String>,
    /// When a press or release last reached the daemon through its own
    /// backend (ISO 8601 UTC); absent until one does.
    #[serde(default)]
    pub last_press_at: Option<String>,
}

/// `hotkeys.snippet` params: the compositor (`hyprland`, `hyprland-lua`,
/// `hyprland-conf`, `sway`, `niri`), or absent for the running session's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnippetParams {
    /// The compositor name; the session's compositor when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compositor: Option<String>,
}

/// `hotkeys.snippet` result: the snippet text `dettivo setup <compositor>
/// --stdout` prints, with the chords from `[hotkeys]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnippetResult {
    /// The compositor the snippet is for (`hyprland`, `sway`, `niri`).
    pub compositor: String,
    /// The snippet text.
    pub text: String,
    /// The line the main configuration needs to load the snippet.
    pub include_line: String,
    /// Where `hotkeys.setup` writes the snippet.
    pub path: String,
    /// What to tell the user beyond the include line.
    pub notes: Vec<String>,
}

/// `hotkeys.setup` params: the compositor, and whether to write.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupParams {
    /// The compositor name; the session's compositor when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compositor: Option<String>,
    /// Write the snippet (idempotent); false reports the state only, the
    /// way `dettivo setup --check` does.
    #[serde(default)]
    pub write: bool,
}

/// `hotkeys.setup` result: the snippet's state after the call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupResult {
    /// The compositor.
    pub compositor: String,
    /// True when the snippet file exists now.
    pub written: bool,
    /// The snippet file.
    pub path: String,
    /// The line the main configuration needs.
    pub include_line: String,
    /// The user's main configuration file.
    pub main_config: MainConfig,
    /// True when the main configuration loads the snippet.
    pub sourced: bool,
}

/// The user's main configuration as `hotkeys.setup` sees it; never
/// written by the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MainConfig {
    /// The file the include line belongs in.
    pub path: String,
    /// True when the file exists.
    pub exists: bool,
}
