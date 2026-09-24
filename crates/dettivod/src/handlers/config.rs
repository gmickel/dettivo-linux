//! `config.*`: the file API (ADR 0009). Writes go through
//! `dettivo_core::config::edit`, which keeps comments and ordering and
//! refuses any text the schema would not read back, so the daemon never
//! writes a file it could not parse (R5).

use dettivo_core::config::{self, DEFAULT_TOML, edit, keys};
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::config::{
    Entry, GetParams, GetResult, KeysResult, PathResult, PrintDefaultResult, SetParams,
    UnsetParams, ValidateResult, ValidationError,
};
use serde_json::{Map, Value};

use super::{json, params};

use crate::daemon::Daemon;

fn internal(message: String) -> JsonRpcError {
    JsonRpcError::new(AppCode::InternalError, message, ErrorDetails::empty())
}

/// A validation finding as the wire error: `INVALID_PARAMS`, the message
/// as validation produced it, the key in `details.key` when known. The
/// line is not carried: `config.validate` reports it for the file as a
/// whole, while an edit's failure is about the value the client sent.
fn invalid(err: ValidationError) -> JsonRpcError {
    let mut details = Map::new();
    if let Some(key) = &err.key {
        details.insert("key".into(), Value::String(key.clone()));
    }
    JsonRpcError::new(AppCode::InvalidParams, err.message, ErrorDetails(details))
}

fn entry_for(daemon: &Daemon, key: &str) -> Result<Entry, JsonRpcError> {
    let loaded = daemon.config();
    let mut entries = loaded.entries(Some(key), &daemon.paths).map_err(invalid)?;
    entries.pop().ok_or_else(|| {
        invalid(ValidationError {
            key: Some(key.to_string()),
            line: None,
            message: format!("{key}: unknown key"),
        })
    })
}

/// `config.get`.
pub fn get(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: GetParams = params(params_value)?;
    let loaded = daemon.config();
    let entries = loaded
        .entries(p.key.as_deref(), &daemon.paths)
        .map_err(invalid)?;
    json(GetResult { entries })
}

/// The file text to edit: the file as it is, or empty when absent.
pub(crate) fn current_text(daemon: &Daemon) -> Result<String, JsonRpcError> {
    match std::fs::read_to_string(&daemon.paths.config_file) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(internal(format!(
            "cannot read {}: {e}",
            daemon.paths.config_file.display()
        ))),
    }
}

/// One edit of the file under the daemon's edit lock: the text as it is,
/// `edit` applied to it, the result written and reloaded. Every writer
/// goes through here, so a `config.set` from the app and a `config.unset`
/// from the command line cannot interleave their read and their write.
pub(crate) fn edit_file(
    daemon: &Daemon,
    edit: impl FnOnce(String) -> Result<String, JsonRpcError>,
) -> Result<(), JsonRpcError> {
    let _guard = daemon.config_edit_lock();
    let text = current_text(daemon)?;
    let updated = edit(text)?;
    write_and_reload(daemon, &updated)
}

/// `edit_file` for a writer that derives its change from the values the
/// file holds (a rule added to the list, an endpoint added to the trusted
/// ones): the values are read from the file text under the same lock, so
/// two additions at once never derive from the same stale snapshot and
/// an update cannot restore what a delete just removed.
pub(crate) fn edit_values(
    daemon: &Daemon,
    edit: impl FnOnce(String, config::Config) -> Result<String, JsonRpcError>,
) -> Result<(), JsonRpcError> {
    edit_file(daemon, |text| {
        let current = config::validate::validate(&text).map_err(invalid)?;
        edit(text, current)
    })
}

fn write_and_reload(daemon: &Daemon, text: &str) -> Result<(), JsonRpcError> {
    config::write_text(&daemon.paths.config_file, text).map_err(|e| {
        internal(format!(
            "cannot write {}: {e}",
            daemon.paths.config_file.display()
        ))
    })?;
    let loaded = daemon.reload_locked();
    if let Some(err) = loaded.error {
        return Err(invalid(err));
    }
    tracing::info!(path = %daemon.paths.config_file.display(), "configuration written");
    Ok(())
}

/// `config.set`.
pub fn set(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: SetParams = params(params_value)?;
    if p.key == "audio.input_device" {
        if let (Some(name), Some(service)) =
            (p.value.as_str().filter(|s| !s.is_empty()), daemon.audio())
        {
            if service.node_named(name).is_none() {
                return Err(invalid(ValidationError {
                    key: Some(p.key.clone()),
                    line: None,
                    message: format!(
                        "audio.input_device: {name:?} is not a PipeWire node (audio.devices lists them)"
                    ),
                }));
            }
        }
    }
    edit_file(daemon, |text| {
        edit::set(&text, &p.key, &p.value).map_err(invalid)
    })?;
    json(entry_for(daemon, &p.key)?)
}

/// `config.unset`.
pub fn unset(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: UnsetParams = params(params_value)?;
    edit_file(daemon, |text| edit::unset(&text, &p.key).map_err(invalid))?;
    json(entry_for(daemon, &p.key)?)
}

/// `config.validate`: the file as it is on disk right now. A file that
/// differs from the loaded snapshot is reloaded first, so an editor's
/// save followed by `validate` converges without waiting for the watcher.
pub fn validate(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let text = current_text(daemon)?;
    if daemon.config().text.as_deref().unwrap_or("") != text {
        let loaded = daemon.reload();
        tracing::info!(
            path = %daemon.paths.config_file.display(),
            ok = loaded.error.is_none(),
            "configuration reloaded on validate"
        );
    }
    let errors = match config::validate::validate(&text) {
        Ok(_) => Vec::new(),
        Err(e) => vec![e],
    };
    json(ValidateResult {
        ok: errors.is_empty(),
        path: daemon.paths.config_file.to_string_lossy().into_owned(),
        errors,
    })
}

/// `config.path`.
pub fn path(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let loaded = daemon.config();
    let base = &daemon.paths;
    let socket = loaded.socket(base);
    json(PathResult {
        config: base.config_file.to_string_lossy().into_owned(),
        state: base.state_file.to_string_lossy().into_owned(),
        data: loaded.data_dir(base).to_string_lossy().into_owned(),
        runtime: dettivo_core::paths::Paths::socket_dir(&socket)
            .to_string_lossy()
            .into_owned(),
        socket: socket.to_string_lossy().into_owned(),
        token_file: loaded.token_file(base).to_string_lossy().into_owned(),
    })
}

/// `config.print_default`.
pub fn print_default() -> Result<Value, JsonRpcError> {
    json(PrintDefaultResult {
        text: DEFAULT_TOML.to_string(),
    })
}

/// `config.keys` (ADR 0033): the registry the settings routes and the
/// coverage lint read.
pub fn keys() -> Result<Value, JsonRpcError> {
    json(KeysResult {
        keys: keys::describe(),
    })
}
