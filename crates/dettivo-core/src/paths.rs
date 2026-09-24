//! Where Dettivo's files live, following the XDG base directory spec with
//! the environment overrides the contract documents (`DETTIVO_CONFIG`,
//! `DETTIVO_DATA_DIR`, `DETTIVO_IPC_SOCKET`).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use dettivo_proto::methods::config::Source;

/// Environment variable that replaces the configuration file path.
pub const ENV_CONFIG: &str = "DETTIVO_CONFIG";
/// Environment variable that replaces the data directory.
pub const ENV_DATA_DIR: &str = "DETTIVO_DATA_DIR";
/// Environment variable that replaces the socket path.
pub const ENV_SOCKET: &str = "DETTIVO_IPC_SOCKET";
/// Environment variable that turns QA mode on (`1`, `true`, `yes`, `on`).
pub const ENV_QA: &str = "DETTIVO_QA";
/// Environment variable that supplies the shared token in `peer_token` mode.
pub const ENV_TOKEN: &str = "DETTIVO_IPC_TOKEN";

/// Every location the daemon resolves before reading the configuration
/// file, plus the source of each override-able one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// `$XDG_CONFIG_HOME/dettivo/config.toml` unless `DETTIVO_CONFIG`.
    pub config_file: PathBuf,
    /// `$XDG_STATE_HOME/dettivo/state.toml`.
    pub state_file: PathBuf,
    /// `$XDG_DATA_HOME/dettivo` unless `DETTIVO_DATA_DIR`; the file may
    /// also set it, which the daemon resolves on top of this default.
    pub data_dir: PathBuf,
    /// `$XDG_RUNTIME_DIR/dettivo`, created `0700`.
    pub runtime_dir: PathBuf,
    /// `$XDG_CACHE_HOME/dettivo`: transfer staging and export files.
    pub cache_dir: PathBuf,
    /// `<runtime_dir>/dettivo.sock` unless `DETTIVO_IPC_SOCKET`.
    pub socket: PathBuf,
    /// `$XDG_CONFIG_HOME/dettivo/ipc.token`, consulted in `peer_token` mode.
    pub token_file: PathBuf,
    /// Where `config_file` came from (`Default` or `Environment`).
    pub config_file_source: Source,
    /// Where `data_dir` came from.
    pub data_dir_source: Source,
    /// Where `socket` came from.
    pub socket_source: Source,
}

impl Paths {
    /// Resolves from the process environment.
    pub fn resolve() -> Self {
        Self::from_env(|key| std::env::var_os(key))
    }

    /// Resolves from an environment lookup, so tests can pin every input.
    pub fn from_env(get: impl Fn(&str) -> Option<OsString>) -> Self {
        let home = get("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/"));
        let xdg = |var: &str, fallback: &str| -> PathBuf {
            match get(var).filter(|v| !v.is_empty()) {
                Some(v) => PathBuf::from(v),
                None => home.join(fallback),
            }
        };
        let config_home = xdg("XDG_CONFIG_HOME", ".config");
        let state_home = xdg("XDG_STATE_HOME", ".local/state");
        let data_home = xdg("XDG_DATA_HOME", ".local/share");
        let cache_home = xdg("XDG_CACHE_HOME", ".cache");
        let runtime_dir = match get("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
            Some(v) => PathBuf::from(v).join("dettivo"),
            None => {
                let user = get("USER")
                    .and_then(|u| u.into_string().ok())
                    .unwrap_or_else(|| "user".into());
                PathBuf::from(format!("/tmp/dettivo-{user}"))
            }
        };
        let (config_file, config_file_source) = match get(ENV_CONFIG).filter(|v| !v.is_empty()) {
            Some(v) => (PathBuf::from(v), Source::Environment),
            None => (
                config_home.join("dettivo").join("config.toml"),
                Source::Default,
            ),
        };
        let (data_dir, data_dir_source) = match get(ENV_DATA_DIR).filter(|v| !v.is_empty()) {
            Some(v) => (PathBuf::from(v), Source::Environment),
            None => (data_home.join("dettivo"), Source::Default),
        };
        let (socket, socket_source) = match get(ENV_SOCKET).filter(|v| !v.is_empty()) {
            Some(v) => (PathBuf::from(v), Source::Environment),
            None => (runtime_dir.join("dettivo.sock"), Source::Default),
        };
        Self {
            config_file,
            state_file: state_home.join("dettivo").join("state.toml"),
            data_dir,
            runtime_dir,
            cache_dir: cache_home.join("dettivo"),
            socket,
            token_file: config_home.join("dettivo").join("ipc.token"),
            config_file_source,
            data_dir_source,
            socket_source,
        }
    }

    /// The runtime directory a socket path implies: its parent.
    pub fn socket_dir(socket: &Path) -> PathBuf {
        socket
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Reads a boolean environment flag: `1`, `true`, `yes`, `on` are true.
pub fn env_flag(value: &OsString) -> bool {
    matches!(
        value.to_string_lossy().trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, OsString> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), OsString::from(v)))
            .collect();
        move |key| map.get(key).cloned()
    }

    #[test]
    fn xdg_defaults_hang_off_home() {
        let p = Paths::from_env(env(&[
            ("HOME", "/home/u"),
            ("XDG_RUNTIME_DIR", "/run/user/7"),
        ]));
        assert_eq!(
            p.config_file,
            PathBuf::from("/home/u/.config/dettivo/config.toml")
        );
        assert_eq!(
            p.state_file,
            PathBuf::from("/home/u/.local/state/dettivo/state.toml")
        );
        assert_eq!(p.data_dir, PathBuf::from("/home/u/.local/share/dettivo"));
        assert_eq!(p.cache_dir, PathBuf::from("/home/u/.cache/dettivo"));
        assert_eq!(p.socket, PathBuf::from("/run/user/7/dettivo/dettivo.sock"));
        assert_eq!(
            p.token_file,
            PathBuf::from("/home/u/.config/dettivo/ipc.token")
        );
        assert_eq!(p.socket_source, Source::Default);
    }

    #[test]
    fn environment_overrides_win_and_are_labelled() {
        let p = Paths::from_env(env(&[
            ("HOME", "/home/u"),
            ("XDG_CONFIG_HOME", "/cfg"),
            (ENV_CONFIG, "/etc/d.toml"),
            (ENV_DATA_DIR, "/data"),
            (ENV_SOCKET, "/tmp/s.sock"),
        ]));
        assert_eq!(p.config_file, PathBuf::from("/etc/d.toml"));
        assert_eq!(p.config_file_source, Source::Environment);
        assert_eq!(p.data_dir, PathBuf::from("/data"));
        assert_eq!(p.socket, PathBuf::from("/tmp/s.sock"));
        assert_eq!(p.socket_source, Source::Environment);
        assert_eq!(p.token_file, PathBuf::from("/cfg/dettivo/ipc.token"));
    }

    #[test]
    fn missing_runtime_dir_falls_back_under_tmp() {
        let p = Paths::from_env(env(&[("HOME", "/home/u"), ("USER", "u")]));
        assert_eq!(p.runtime_dir, PathBuf::from("/tmp/dettivo-u"));
        assert!(env_flag(&OsString::from("Yes")));
        assert!(!env_flag(&OsString::from("0")));
    }
}
