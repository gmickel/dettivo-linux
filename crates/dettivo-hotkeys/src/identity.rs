//! The daemon's identity at the desktop portal (ADR 0083). A host
//! (unsandboxed) process has no app id unless it registers one, and the
//! GlobalShortcuts portal refuses a session without one (`NotAllowed: An
//! app id is required`). The app id is `dettivo`, the name of the desktop
//! entry the package ships; a source build gets a minimal entry under
//! `$XDG_DATA_HOME/applications/`, retired once the package's entry
//! exists so it never shadows it. The id is registered through
//! `org.freedesktop.host.portal.Registry` on the connection that then
//! opens the session.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::Identity;

/// The app id, and the desktop entry's name without `.desktop`.
pub const APP_ID: &str = "dettivo";

/// What to do when the portal still refuses the identity.
pub const FIX: &str = "restart the portal once ~/.local/share/applications/dettivo.desktop or /usr/share/applications/dettivo.desktop exists (systemctl --user restart xdg-desktop-portal), and start the daemon directly rather than through a setgid wrapper such as newgrp, which hides its /proc entry from the portal";

/// The line that marks an entry as the daemon's own, so it is the only
/// user entry the daemon ever removes.
const GENERATED: &str = "X-Dettivo-Generated=true";

/// The entry a source build gets, so the portal can resolve the app id.
/// GLib refuses an entry whose `Exec` program it cannot find, and the
/// portal refuses an app id without a valid entry, so `exec` is the
/// absolute path of a program that exists (the running daemon).
pub fn minimal_entry(exec: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Dettivo\nComment=Dictation hotkeys through the desktop portal\nExec={}\nNoDisplay=true\n{GENERATED}\n",
        exec_arg(&exec.to_string_lossy())
    )
}

/// One `Exec` argument (Desktop Entry Specification, "The Exec key"):
/// quoted when it holds a reserved character, with `%` doubled so it is
/// no field code, then escaped as a string value, which doubles every
/// backslash and writes a newline, tab or carriage return as `\n`, `\t`
/// or `\r`.
fn exec_arg(arg: &str) -> String {
    const RESERVED: &str = " \t\n\"'\\><~|&;$*?#()`";
    let mut quoted = String::new();
    if arg.chars().any(|c| RESERVED.contains(c)) {
        quoted.push('"');
        for c in arg.chars() {
            if matches!(c, '"' | '`' | '$' | '\\') {
                quoted.push('\\');
            }
            quoted.push(c);
        }
        quoted.push('"');
    } else {
        quoted.push_str(arg);
    }
    let mut value = String::new();
    for c in quoted.replace('%', "%%").chars() {
        match c {
            '\\' => value.push_str("\\\\"),
            '\n' => value.push_str("\\n"),
            '\t' => value.push_str("\\t"),
            '\r' => value.push_str("\\r"),
            c => value.push(c),
        }
    }
    value
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

/// What [`ensure_desktop_entry`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryChange {
    /// Wrote the minimal entry at this path.
    Written(PathBuf),
    /// Removed the generated entry at this path, which the package's
    /// entry replaces.
    Retired(PathBuf),
    /// Left everything as it was.
    Unchanged,
}

/// Makes `dettivo.desktop` resolve to one valid entry. When one of
/// `data_dirs` holds the package's entry, a generated entry under
/// `data_home` would shadow it, so that one is removed; an entry the user
/// wrote, or a symlink, is never touched. Otherwise the minimal entry,
/// running `exec`, is created under `data_home` unless something already
/// sits at that path.
pub fn ensure_desktop_entry(
    data_home: &Path,
    data_dirs: &[PathBuf],
    exec: &Path,
) -> Result<EntryChange, String> {
    let name = format!("applications/{APP_ID}.desktop");
    let path = data_home.join(&name);
    let packaged = data_dirs
        .iter()
        .filter(|dir| dir.as_path() != data_home)
        .any(|dir| dir.join(&name).is_file());
    if packaged {
        if !is_generated(&path) {
            return Ok(EntryChange::Unchanged);
        }
        std::fs::remove_file(&path)
            .map_err(|e| format!("cannot remove {}: {e}", path.display()))?;
        return Ok(EntryChange::Retired(path));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    // Never truncates an entry created meanwhile, nor follows a symlink.
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path);
    let mut file = match file {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Ok(EntryChange::Unchanged);
        }
        Err(e) => return Err(format!("cannot create {}: {e}", path.display())),
    };
    std::io::Write::write_all(&mut file, minimal_entry(exec).as_bytes())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(EntryChange::Written(path))
}

/// A regular file, not a symlink, that carries the generated marker.
fn is_generated(path: &Path) -> bool {
    path.symlink_metadata().is_ok_and(|m| m.is_file())
        && std::fs::read_to_string(path)
            .is_ok_and(|text| text.lines().any(|l| l.trim() == GENERATED))
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
        assert_eq!(written, EntryChange::Written(path.clone()));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("\nExec=/opt/dettivo/bin/dettivod\n"),
            "{text}"
        );
        assert_eq!(
            ensure_desktop_entry(&home, std::slice::from_ref(&system), exec).unwrap(),
            EntryChange::Unchanged
        );

        // The package's entry under a data dir is enough.
        let fresh = dir.path().join("fresh");
        std::fs::create_dir_all(system.join("applications")).unwrap();
        std::fs::write(system.join("applications/dettivo.desktop"), "x").unwrap();
        assert_eq!(
            ensure_desktop_entry(&fresh, &[system], exec).unwrap(),
            EntryChange::Unchanged
        );
        assert!(!fresh.join("applications/dettivo.desktop").exists());
    }

    #[test]
    fn exec_paths_with_reserved_characters_are_quoted() {
        assert_eq!(exec_arg("/usr/bin/dettivod"), "/usr/bin/dettivod");
        assert_eq!(exec_arg("/home/a b/dettivod"), "\"/home/a b/dettivod\"");
        assert_eq!(exec_arg("/x/$y"), "\"/x/\\\\$y\"");
        assert_eq!(exec_arg("/x/a\\b"), "\"/x/a\\\\\\\\b\"");
        assert_eq!(exec_arg("/x/a\nb"), "\"/x/a\\nb\"");
        assert_eq!(exec_arg("/x/100%"), "/x/100%%");
        assert_eq!(exec_arg("/a b/%"), "\"/a b/%%\"");
        for path in ["/a b/$c", "/x/a\\b", "/x/a\nb", "/x/a\tb\"`'%"] {
            let line = minimal_entry(Path::new(path));
            let exec = line.lines().find_map(|l| l.strip_prefix("Exec=")).unwrap();
            assert_eq!(unquote(&unescape(exec)), path, "{line}");
        }
    }

    /// The string layer of a desktop entry value, read back.
    fn unescape(value: &str) -> String {
        let mut out = String::new();
        let mut chars = value.chars();
        while let Some(c) = chars.next() {
            out.push(match (c, c == '\\') {
                (_, true) => match chars.next() {
                    Some('s') => ' ',
                    Some('n') => '\n',
                    Some('t') => '\t',
                    Some('r') => '\r',
                    Some(other) => other,
                    None => '\\',
                },
                (c, false) => c,
            });
        }
        out
    }

    /// One `Exec` argument, read back: unquoted, field codes undoubled.
    fn unquote(arg: &str) -> String {
        let arg = arg.replace("%%", "%");
        let Some(inner) = arg.strip_prefix('"').and_then(|a| a.strip_suffix('"')) else {
            return arg;
        };
        let mut out = String::new();
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            out.push(if c == '\\' {
                chars.next().unwrap_or(c)
            } else {
                c
            });
        }
        out
    }

    #[test]
    fn a_dangling_symlink_at_the_user_path_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        std::fs::create_dir_all(home.join("applications")).unwrap();
        let target = dir.path().join("elsewhere.desktop");
        let link = home.join("applications/dettivo.desktop");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let exec = Path::new("/opt/dettivo/bin/dettivod");
        assert_eq!(
            ensure_desktop_entry(&home, &[], exec).unwrap(),
            EntryChange::Unchanged
        );
        assert!(link.symlink_metadata().unwrap().is_symlink());
        assert!(!target.exists());
    }

    #[test]
    fn a_packaged_entry_retires_only_the_generated_user_entry() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let system = dir.path().join("system");
        let exec = Path::new("/opt/dettivo/bin/dettivod");
        let user = home.join("applications/dettivo.desktop");

        // Alone, the generated entry stays.
        ensure_desktop_entry(&home, &[], exec).unwrap();
        ensure_desktop_entry(&home, std::slice::from_ref(&system), exec).unwrap();
        assert!(user.is_file());

        // Once the package installs its entry, the generated one goes.
        std::fs::create_dir_all(system.join("applications")).unwrap();
        std::fs::write(system.join("applications/dettivo.desktop"), "x").unwrap();
        assert_eq!(
            ensure_desktop_entry(&home, std::slice::from_ref(&system), exec).unwrap(),
            EntryChange::Retired(user.clone())
        );
        assert!(!user.exists());

        // An entry the user wrote stays.
        std::fs::write(&user, "[Desktop Entry]\nName=Mine\n").unwrap();
        assert_eq!(
            ensure_desktop_entry(&home, std::slice::from_ref(&system), exec).unwrap(),
            EntryChange::Unchanged
        );
        assert_eq!(
            std::fs::read_to_string(&user).unwrap(),
            "[Desktop Entry]\nName=Mine\n"
        );
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
