//! `hotkeys.status`, `hotkeys.snippet` and `hotkeys.setup` (Linux
//! additions): the backend in force with the portal and evdev availability
//! and the moment a key last reached the daemon; the compositor snippet
//! rendered by the same generator `dettivo setup --stdout` uses, so the
//! first-run Keys step shows the exact text it will write; and the write
//! of that snippet with the check `dettivo setup --check` runs (ADR 0024).

use dettivo_hotkeys::Keys;
use dettivo_hotkeys::snippet::{self, Compositor};
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::hotkeys::{
    MainConfig, SetupParams, SetupResult, SnippetParams, SnippetResult,
};
use serde_json::Value;

use super::{json, params};

use crate::daemon::Daemon;

/// `hotkeys.status`.
pub fn status(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    json(daemon.hotkeys().status(&daemon.config()))
}

/// The compositor a request names, or the session's; a desktop the
/// snippets do not cover is `INVALID_PARAMS` naming the supported ones,
/// which tells the Keys step to show the portal path instead.
fn compositor(
    name: Option<&str>,
    config_home: &std::path::Path,
) -> Result<Compositor, JsonRpcError> {
    let invalid = |m: String| JsonRpcError::new(AppCode::InvalidParams, m, ErrorDetails::empty());
    match name {
        Some(name) => Compositor::resolve(name, config_home).map_err(|e| invalid(e.to_string())),
        None => {
            let compositor = crate::platform::compositor(&|k| std::env::var(k).ok());
            Compositor::detect(compositor.as_deref(), config_home).ok_or_else(|| {
                invalid(format!(
                    "no snippet for this desktop ({}); supported: {}",
                    compositor.as_deref().unwrap_or("unknown compositor"),
                    snippet::SUPPORTED.join(", ")
                ))
            })
        }
    }
}

/// The `[hotkeys]` chords in force; a chord that does not parse is
/// `INVALID_PARAMS` naming its key.
fn keys(daemon: &Daemon) -> Result<Keys, JsonRpcError> {
    let h = daemon.config().config.hotkeys;
    Keys::parse(&h.hold, &h.toggle, &h.cancel, &h.reinsert).map_err(|e| {
        JsonRpcError::new(AppCode::InvalidParams, e.to_string(), ErrorDetails::empty())
    })
}

/// `hotkeys.snippet`.
pub fn snippet(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: SnippetParams = params(params_value)?;
    let home = snippet::config_home(|k| std::env::var_os(k));
    let compositor = compositor(p.compositor.as_deref(), &home)?;
    let rendered = snippet::render(compositor, &keys(daemon)?);
    json(SnippetResult {
        compositor: compositor.name().to_string(),
        text: rendered.text,
        include_line: compositor.include_line().to_string(),
        path: home
            .join(compositor.snippet_path())
            .to_string_lossy()
            .into_owned(),
        notes: rendered.notes,
    })
}

/// `hotkeys.setup`: writes the snippet when asked (a file that already
/// holds the same text is left alone), then reports the state the way
/// `dettivo setup --check` does. Lua Hyprland setup adds the include and reloads.
pub fn setup(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: SetupParams = params(params_value)?;
    let home = snippet::config_home(|k| std::env::var_os(k));
    let compositor = compositor(p.compositor.as_deref(), &home)?;
    let check = if p.write {
        let (_, check) = dettivo_hotkeys::install::install(compositor, &home, &keys(daemon)?)
            .map_err(|e| JsonRpcError::new(AppCode::InternalError, e, ErrorDetails::empty()))?;
        tracing::info!(compositor = compositor.name(), path = %check.snippet_path.display(), "hotkeys: snippet written");
        check
    } else {
        snippet::check(compositor, &home)
    };
    json(SetupResult {
        compositor: compositor.name().to_string(),
        written: check.written,
        path: check.snippet_path.to_string_lossy().into_owned(),
        include_line: compositor.include_line().to_string(),
        main_config: MainConfig {
            path: check.main_config.to_string_lossy().into_owned(),
            exists: check.main_exists,
        },
        sourced: check.sourced,
    })
}
