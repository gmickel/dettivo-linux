//! Activates the Omarchy Lua shortcut snippet after writing it.
use std::io::Write;
use std::path::Path;

use crate::Keys;
use crate::snippet::{self, Check, Compositor, Snippet};

/// Install shortcuts, reloading the Lua compositor before reporting success.
pub fn install(
    compositor: Compositor,
    home: &Path,
    keys: &Keys,
) -> Result<(Snippet, Check), String> {
    install_with(compositor, home, keys, |args| {
        let output = std::process::Command::new("hyprctl")
            .args(args)
            .output()
            .map_err(|e| format!("cannot run hyprctl: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "hyprctl {} failed: {}{}",
                args.join(" "),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    })
}

fn install_with(
    compositor: Compositor,
    home: &Path,
    keys: &Keys,
    mut run: impl FnMut(&[&str]) -> Result<String, String>,
) -> Result<(Snippet, Check), String> {
    if compositor != Compositor::HyprlandLua {
        return snippet::write(compositor, home, keys);
    }
    let mut attempt = || -> Result<(Snippet, Check), String> {
        let result = snippet::write(compositor, home, keys)?;
        run(&["reload"])?;
        let errors = run(&["configerrors"])?;
        if !errors.trim().is_empty() {
            return Err(format!("Hyprland configuration errors: {}", errors.trim()));
        }
        Ok((result.0, snippet::check(compositor, home)))
    };
    attempt().map_err(|e| format!("Shortcuts are not activated: {e}. Fix the problem and retry shortcut setup (dettivo setup hyprland)."))
}

/// Add the Lua include after defaults without changing unrelated settings.
pub(crate) fn ensure_include(home: &Path) -> Result<(), String> {
    let compositor = Compositor::HyprlandLua;
    let main = home.join(compositor.main_config());
    let text = std::fs::read_to_string(&main)
        .map_err(|e| format!("cannot read {}: {e}", main.display()))?;
    if compositor.is_sourced(&text) {
        return Ok(());
    }
    let path = home.join(compositor.snippet_path());
    let path = path.to_str().ok_or("shortcut path is not valid UTF-8")?;
    let quoted = path
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r");
    let separator = if text.ends_with('\n') { "" } else { "\n" };
    let include = format!("{separator}dofile(\"{quoted}\")\n");
    std::fs::OpenOptions::new()
        .append(true)
        .open(&main)
        .and_then(|mut file| {
            file.write_all(include.as_bytes())?;
            file.sync_all()
        })
        .map_err(|e| format!("cannot write {}: {e}", main.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lua_setup_sources_after_defaults_and_is_repeatable() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("hypr")).unwrap();
        let main = dir.path().join("hypr/hyprland.lua");
        let original = "require('defaults')\n-- dofile('dettivo.lua')\nlocal unrelated = true -- dettivo.lua\n";
        std::fs::write(&main, original).unwrap();
        for _ in 0..2 {
            let mut calls = Vec::new();
            let (_, check) = install_with(
                Compositor::HyprlandLua,
                dir.path(),
                &Keys::defaults(),
                |args| {
                    calls.push(args.join(" "));
                    Ok(String::new())
                },
            )
            .unwrap();
            assert!(check.written && check.sourced);
            assert_eq!(calls, ["reload", "configerrors"]);
        }
        let text = std::fs::read_to_string(main).unwrap();
        assert!(text.starts_with(original));
        assert_eq!(
            text.lines()
                .filter(|line| line.starts_with("dofile("))
                .count(),
            1
        );
        assert!(text.contains(dir.path().to_str().unwrap()));
    }

    #[test]
    fn failures_are_actionable_and_never_success() {
        for failure in ["missing-main", "unwritable-main", "reload", "configerrors"] {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir(dir.path().join("hypr")).unwrap();
            if failure == "unwritable-main" {
                std::fs::create_dir(dir.path().join("hypr/hyprland.lua")).unwrap();
            } else if failure != "missing-main" {
                std::fs::write(dir.path().join("hypr/hyprland.lua"), "").unwrap();
            }
            let error = install_with(
                Compositor::HyprlandLua,
                dir.path(),
                &Keys::defaults(),
                |args| {
                    if failure == "reload" {
                        Err("connection refused".into())
                    } else if args == ["configerrors"] {
                        Ok("bad Lua syntax".into())
                    } else {
                        Ok(String::new())
                    }
                },
            )
            .unwrap_err();
            assert!(error.contains("retry shortcut setup"), "{error}");
            assert!(
                error.contains(match failure {
                    "reload" => "connection refused",
                    "configerrors" => "bad Lua syntax",
                    _ => "cannot read",
                }),
                "{error}"
            );
        }
    }
}
