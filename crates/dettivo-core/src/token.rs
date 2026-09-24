//! The shared token, resolved in the macOS order with the Keychain
//! replaced by the freedesktop Secret Service: `DETTIVO_IPC_TOKEN`, then
//! the Secret Service item (`service dettivo`, `key ipc-token`), then a
//! `0600` token file. The daemon's `peer_token` mode and the REST shim
//! read the same token from the same three places, so a client that
//! configured one of them reaches both.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use dettivo_proto::methods::config::Source;

/// The three sources in resolution order, named in refusals.
pub const SOURCES: &str = "DETTIVO_IPC_TOKEN, the Secret Service item (service dettivo, key ipc-token), or a 0600 token file";

/// Where a token came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// `DETTIVO_IPC_TOKEN`.
    Environment,
    /// The Secret Service item.
    SecretService,
    /// The `0600` token file.
    File,
}

impl Origin {
    /// The `Source` label the config methods use for the same origin.
    pub fn source(self) -> Source {
        match self {
            Self::Environment => Source::Environment,
            Self::SecretService => Source::Default,
            Self::File => Source::File,
        }
    }

    /// The origin's name in reports (`dettivo rest token`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::SecretService => "secret_service",
            Self::File => "file",
        }
    }
}

/// The shared token and where it came from, or `None` when no source has
/// one.
pub fn resolve(token_file: &Path) -> Option<(String, Origin)> {
    resolve_with(token_file, |k| std::env::var_os(k), secret_service_token)
}

/// `resolve` with the environment and the Secret Service lookup pinned,
/// so a test never shells out.
pub fn resolve_with(
    token_file: &Path,
    env: impl Fn(&str) -> Option<std::ffi::OsString>,
    secret: impl FnOnce() -> Option<String>,
) -> Option<(String, Origin)> {
    if let Some(value) = env(crate::paths::ENV_TOKEN) {
        let token = value.to_string_lossy().trim().to_string();
        if !token.is_empty() {
            return Some((token, Origin::Environment));
        }
    }
    if let Some(token) = secret() {
        return Some((token, Origin::SecretService));
    }
    if is_secure_file(token_file) {
        if let Ok(text) = fs::read_to_string(token_file) {
            let token = text.trim().to_string();
            if !token.is_empty() {
                return Some((token, Origin::File));
            }
        }
    }
    None
}

/// `secret-tool lookup service dettivo key ipc-token`, when libsecret's
/// CLI is installed and an item exists.
pub fn secret_service_token() -> Option<String> {
    let output = Command::new("secret-tool")
        .args(["lookup", "service", "dettivo", "key", "ipc-token"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!token.is_empty()).then_some(token)
}

/// A token file counts only when it is a regular file the user alone can
/// read (`0600` or tighter). A symlink at the path is refused outright,
/// so a link into a private file elsewhere cannot pass as the token.
pub fn is_secure_file(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o077 == 0,
        Err(_) => false,
    }
}

/// Compares in time that depends only on the longer length: the loop
/// always runs over every byte and the length difference is folded into
/// the accumulator instead of returning early.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let len = a.len().max(b.len());
    let mut diff = u8::from(a.len() != b.len());
    for i in 0..len {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn token_file_must_be_private() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ipc.token");
        fs::write(&path, "secret\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!is_secure_file(&path));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(is_secure_file(&path));
        let link = dir.path().join("link.token");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(!is_secure_file(&link), "a symlink is refused");
    }

    #[test]
    fn the_three_sources_resolve_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ipc.token");
        fs::write(&path, "from-file\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let env_set = |k: &str| (k == "DETTIVO_IPC_TOKEN").then(|| OsString::from(" from-env "));
        let none = |_: &str| None;
        assert_eq!(
            resolve_with(&path, env_set, || Some("from-secret".into())),
            Some(("from-env".into(), Origin::Environment))
        );
        assert_eq!(
            resolve_with(&path, none, || Some("from-secret".into())),
            Some(("from-secret".into(), Origin::SecretService))
        );
        assert_eq!(
            resolve_with(&path, none, || None),
            Some(("from-file".into(), Origin::File))
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(resolve_with(&path, none, || None), None);
        assert_eq!(Origin::SecretService.source(), Source::Default);
    }

    #[test]
    fn comparison_is_exact() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(!constant_time_eq(b"", b"a"));
        assert!(constant_time_eq(b"", b""));
    }
}
