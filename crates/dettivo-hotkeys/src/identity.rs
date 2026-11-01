//! The daemon's identity at the desktop portal (ADR 0083). A host
//! (unsandboxed) process has no app id unless it registers one, and the
//! GlobalShortcuts portal refuses a session without one (`NotAllowed: An
//! app id is required`). The app id is `dettivo`, the name of the desktop
//! entry the package ships; a source build gets a minimal entry under
//! `$XDG_DATA_HOME/applications/`. The id is registered through
//! `org.freedesktop.host.portal.Registry` on the connection that then
//! opens the session.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::Identity;

/// The app id, and the desktop entry's name without `.desktop`.
pub const APP_ID: &str = "dettivo";

/// What to do when the portal still refuses the identity.
pub const FIX: &str = "restart the portal once ~/.local/share/applications/dettivo.desktop or /usr/share/applications/dettivo.desktop exists (systemctl --user restart xdg-desktop-portal), and start the daemon directly rather than through a setgid wrapper such as newgrp, which hides its /proc entry from the portal";

/// The entry a source build gets, so the portal can resolve the app id.
/// GLib refuses an entry whose `Exec` program it cannot find, and the
/// portal refuses an app id without a valid entry, so `exec` is the
/// absolute path of a program that exists (the running daemon).
pub fn minimal_entry(exec: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Dettivo\nComment=Dictation hotkeys through the desktop portal\nExec={}\nNoDisplay=true\n",
        exec_arg(&exec.to_string_lossy())
    )
}

/// One `Exec` argument: quoted when it holds a reserved character, with
/// the quoting backslashes doubled for the string value (Desktop Entry
/// Specification, "The Exec key").
fn exec_arg(arg: &str) -> String {
    const RESERVED: &str = " \t\n\"'\\><~|&;$*?#()`";
    if !arg.chars().any(|c| RESERVED.contains(c)) {
        return arg.to_string();
    }
    let mut quoted = String::from("\"");
    for c in arg.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            quoted.push_str("\\\\");
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted
}

/// `$XDG_DATA_HOME` (or `~/.local/share`) and `$XDG_DATA_DIRS` (or
/// `/usr/local/share:/usr/share`).
pub fn data_dirs(env: impl Fn(&str) -> Option<std::ffi::OsString>) -> (PathBuf, Vec<PathBuf>) {
    let home = env("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/"))
                .join(".local/share")
        });
    let dirs = env("XDG_DATA_DIRS")
        .filter(|v| !v.is_empty())
        .map(|v| std::env::split_paths(&v).collect())
        .unwrap_or_else(|| vec!["/usr/local/share".into(), "/usr/share".into()]);
    (home, dirs)
}

/// Writes the minimal entry, running `exec`, under `data_home` unless
/// `dettivo.desktop` exists there or under one of `data_dirs`; returns
/// the path written.
pub fn ensure_desktop_entry(
    data_home: &Path,
    data_dirs: &[PathBuf],
    exec: &Path,
) -> Result<Option<PathBuf>, String> {
    let name = format!("applications/{APP_ID}.desktop");
    if std::iter::once(data_home)
        .chain(data_dirs.iter().map(PathBuf::as_path))
        .any(|dir| dir.join(&name).is_file())
    {
        return Ok(None);
    }
    let path = data_home.join(&name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    std::fs::write(&path, minimal_entry(exec))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(Some(path))
}

/// Registers [`APP_ID`] on `conn` before any portal request on it. A
/// portal without the registry answers that it has no such method or
/// interface: it derives the id from the systemd unit instead.
pub(crate) async fn register(conn: &zbus::Connection) -> Result<Identity, String> {
    let proxy = zbus::Proxy::new(
        conn,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.host.portal.Registry",
    )
    .await
    .map_err(|e| format!("portal registry: {e}"))?;
    let options: HashMap<&str, zbus::zvariant::Value<'_>> = HashMap::new();
    match proxy.call_method("Register", &(APP_ID, options)).await {
        Ok(_) => Ok(Identity::Registered),
        Err(e) if is_unknown(&e) => Ok(Identity::Unit),
        Err(e) => Err(format!(
            "the portal refused the app id {APP_ID}: {e}; {FIX}"
        )),
    }
}

fn is_unknown(e: &zbus::Error) -> bool {
    let name = match e {
        zbus::Error::MethodError(name, _, _) => name.as_str().to_string(),
        zbus::Error::FDO(fdo) => match **fdo {
            zbus::fdo::Error::UnknownMethod(_) => return true,
            zbus::fdo::Error::UnknownInterface(_) => return true,
            _ => return false,
        },
        _ => return false,
    };
    name == "org.freedesktop.DBus.Error.UnknownMethod"
        || name == "org.freedesktop.DBus.Error.UnknownInterface"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entry_is_written_once_and_only_when_none_exists() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let system = dir.path().join("system");
        let exec = Path::new("/opt/dettivo/bin/dettivod");
        let written = ensure_desktop_entry(&home, std::slice::from_ref(&system), exec).unwrap();
        let path = home.join("applications/dettivo.desktop");
        assert_eq!(written.as_deref(), Some(path.as_path()));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("\nExec=/opt/dettivo/bin/dettivod\n"),
            "{text}"
        );
        assert_eq!(
            ensure_desktop_entry(&home, std::slice::from_ref(&system), exec).unwrap(),
            None
        );

        // The package's entry under a data dir is enough.
        let fresh = dir.path().join("fresh");
        std::fs::create_dir_all(system.join("applications")).unwrap();
        std::fs::write(system.join("applications/dettivo.desktop"), "x").unwrap();
        assert_eq!(ensure_desktop_entry(&fresh, &[system], exec).unwrap(), None);
        assert!(!fresh.join("applications/dettivo.desktop").exists());
    }

    #[test]
    fn exec_paths_with_reserved_characters_are_quoted() {
        assert_eq!(exec_arg("/usr/bin/dettivod"), "/usr/bin/dettivod");
        assert_eq!(exec_arg("/home/a b/dettivod"), "\"/home/a b/dettivod\"");
        assert_eq!(exec_arg("/x/$y"), "\"/x/\\\\$y\"");
    }

    #[test]
    fn data_dirs_follow_the_xdg_defaults() {
        let env = |k: &str| match k {
            "HOME" => Some("/h".into()),
            _ => None,
        };
        assert_eq!(
            data_dirs(env),
            (
                PathBuf::from("/h/.local/share"),
                vec![PathBuf::from("/usr/local/share"), "/usr/share".into()]
            )
        );
        let env = |k: &str| match k {
            "XDG_DATA_HOME" => Some("/d".into()),
            "XDG_DATA_DIRS" => Some("/a:/b".into()),
            _ => None,
        };
        assert_eq!(
            data_dirs(env),
            (PathBuf::from("/d"), vec![PathBuf::from("/a"), "/b".into()])
        );
    }
}
