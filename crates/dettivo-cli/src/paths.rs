//! The two locations the client resolves on its own: the socket (so it can
//! connect) and the default configuration file (so `config edit` and
//! `config path` work before a daemon has ever run). The daemon owns every
//! other path and reports them through `config.path`.

use std::ffi::OsString;
use std::path::PathBuf;

/// The socket to connect to: `--socket`, `DETTIVO_IPC_SOCKET`, then the
/// default under `$XDG_RUNTIME_DIR`.
pub fn socket(flag: Option<&PathBuf>, env: impl Fn(&str) -> Option<OsString>) -> PathBuf {
    if let Some(p) = flag {
        return p.clone();
    }
    if let Some(v) = env("DETTIVO_IPC_SOCKET").filter(|v| !v.is_empty()) {
        return PathBuf::from(v);
    }
    runtime_dir(&env).join("dettivo.sock")
}

/// `$XDG_RUNTIME_DIR/dettivo`, or `/tmp/dettivo-$USER` without a session.
pub fn runtime_dir(env: &impl Fn(&str) -> Option<OsString>) -> PathBuf {
    match env("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        Some(v) => PathBuf::from(v).join("dettivo"),
        None => {
            let user = env("USER")
                .and_then(|u| u.into_string().ok())
                .unwrap_or_else(|| "user".into());
            PathBuf::from(format!("/tmp/dettivo-{user}"))
        }
    }
}

/// The default configuration file: `DETTIVO_CONFIG`, then
/// `$XDG_CONFIG_HOME/dettivo/config.toml`.
pub fn config_file(env: impl Fn(&str) -> Option<OsString>) -> PathBuf {
    if let Some(v) = env("DETTIVO_CONFIG").filter(|v| !v.is_empty()) {
        return PathBuf::from(v);
    }
    let home = env("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));
    let config_home = match env("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        Some(v) => PathBuf::from(v),
        None => home.join(".config"),
    };
    config_home.join("dettivo").join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_resolution_order() {
        let env = |k: &str| match k {
            "DETTIVO_IPC_SOCKET" => Some(OsString::from("/tmp/e.sock")),
            "XDG_RUNTIME_DIR" => Some(OsString::from("/run/user/7")),
            _ => None,
        };
        assert_eq!(
            socket(Some(&PathBuf::from("/f.sock")), env),
            PathBuf::from("/f.sock")
        );
        assert_eq!(socket(None, env), PathBuf::from("/tmp/e.sock"));
        let env = |k: &str| (k == "XDG_RUNTIME_DIR").then(|| OsString::from("/run/user/7"));
        assert_eq!(
            socket(None, env),
            PathBuf::from("/run/user/7/dettivo/dettivo.sock")
        );
    }

    #[test]
    fn config_file_follows_xdg() {
        let env = |k: &str| (k == "HOME").then(|| OsString::from("/home/u"));
        assert_eq!(
            config_file(env),
            PathBuf::from("/home/u/.config/dettivo/config.toml")
        );
    }
}
