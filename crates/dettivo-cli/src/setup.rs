//! `dettivo setup <compositor>`: writes the binding snippet for Hyprland,
//! Sway or Niri under `$XDG_CONFIG_HOME` with the chords from `[hotkeys]`
//! (read through the daemon), and activates the Lua Hyprland include.
//! `--stdout` prints the snippet instead of writing it; `--check` reports whether the include line is present.

use std::path::{Path, PathBuf};

use dettivo_hotkeys::Keys;
use dettivo_hotkeys::snippet::{self, Compositor};
use serde_json::{Value, json};

use crate::client::Client;
use crate::exit::{Exit, Failure};
use crate::{Cli, output};

/// `$XDG_CONFIG_HOME`, or `$HOME/.config`.
pub fn config_home(env: impl Fn(&str) -> Option<std::ffi::OsString>) -> PathBuf {
    snippet::config_home(env)
}

/// The compositor for a name: `hyprland` becomes the Lua flavour when
/// `hyprland.lua` exists under `config_home/hypr` (Hyprland 0.56 and
/// Omarchy), the classic one otherwise.
pub fn resolve(name: &str, config_home: &Path) -> Result<Compositor, Failure> {
    Compositor::resolve(name, config_home)
        .map_err(|e| Failure::new(Exit::InvalidArgs, e.to_string()))
}

/// The compositor of the running session, from `system.capabilities`'
/// `platform.compositor` (`Hyprland`, `sway`, `niri`).
pub fn detect(compositor: Option<&str>, config_home: &Path) -> Option<Compositor> {
    Compositor::detect(compositor, config_home)
}

/// The `[hotkeys]` chords through the daemon; an unparseable chord is
/// reported with its key and the notation.
pub fn keys(client: &Client) -> Result<Keys, Failure> {
    let get = |key: &str| -> Result<String, Failure> {
        let result = client.call("config.get", json!({ "key": key }))?;
        Ok(result["entries"][0]["value"]
            .as_str()
            .unwrap_or_default()
            .to_string())
    };
    Keys::parse(
        &get("hotkeys.hold")?,
        &get("hotkeys.toggle")?,
        &get("hotkeys.cancel")?,
        &get("hotkeys.reinsert")?,
    )
    .map_err(|e| Failure::new(Exit::InvalidArgs, e.to_string()))
}

/// What `--check` (and `doctor`) report.
pub fn check(compositor: Compositor, config_home: &Path) -> Value {
    let c = snippet::check(compositor, config_home);
    json!({
        "compositor": compositor.name(),
        "snippet": { "path": c.snippet_path.to_string_lossy(), "written": c.written },
        "main_config": { "path": c.main_config.to_string_lossy(), "exists": c.main_exists, "sourced": c.sourced },
        "include_line": compositor.include_line(),
    })
}

/// One doctor line for a check result.
pub fn check_line(check: &Value) -> String {
    let state = if check["main_config"]["sourced"] == Value::Bool(true) {
        "sourced".to_string()
    } else if check["snippet"]["written"] == Value::Bool(true) {
        format!(
            "written, not sourced (add to {}: {})",
            check["main_config"]["path"].as_str().unwrap_or("?"),
            check["include_line"].as_str().unwrap_or("?")
        )
    } else {
        format!(
            "not written (run `dettivo setup {}`)",
            check["compositor"].as_str().unwrap_or("?")
        )
    };
    format!(
        "snippet   {}: {state} ({})\n",
        check["compositor"].as_str().unwrap_or("?"),
        check["snippet"]["path"].as_str().unwrap_or("?")
    )
}

/// Renders the snippet for `compositor` with the daemon's chords and
/// writes it under `home`, leaving a file that already holds the same
/// text untouched; the notes are what the generator wants said.
pub fn write(
    client: &Client,
    compositor: Compositor,
    home: &Path,
) -> Result<(PathBuf, Vec<String>), Failure> {
    let keys = keys(client)?;
    let (mut rendered, written) = dettivo_hotkeys::install::install(compositor, home, &keys)
        .map_err(|e| Failure::new(Exit::Failure, e))?;
    if compositor == Compositor::HyprlandLua {
        rendered
            .notes
            .retain(|note| !note.starts_with("Add this line"));
    }
    Ok((written.snippet_path, rendered.notes))
}

/// Runs `dettivo setup`.
pub fn run(
    cli: &Cli,
    client: &Client,
    name: &str,
    stdout: bool,
    check_only: bool,
) -> Result<(), Failure> {
    let home = config_home(|k| std::env::var_os(k));
    let compositor = resolve(name, &home)?;
    if check_only {
        let report = check(compositor, &home);
        if cli.json {
            output::result(cli, "setup.check", &report);
        } else if !cli.quiet {
            print!("{}", check_line(&report));
        }
        return if report["main_config"]["sourced"] == Value::Bool(true) {
            Ok(())
        } else {
            Err(Failure::reported(
                Exit::Failure,
                format!(
                    "the snippet is not loaded; add to {}: {}",
                    report["main_config"]["path"].as_str().unwrap_or("?"),
                    compositor.include_line()
                ),
            ))
        };
    }
    if stdout {
        let keys = keys(client)?;
        let rendered = snippet::render(compositor, &keys);
        print!("{}", rendered.text);
        if !cli.quiet {
            for note in &rendered.notes {
                eprintln!("note: {note}");
            }
        }
        return Ok(());
    }
    let (path, notes) = write(client, compositor, &home)?;
    let report = check(compositor, &home);
    if cli.json {
        output::result(
            cli,
            "setup",
            &json!({ "written": path.to_string_lossy(), "check": report, "notes": notes }),
        );
    } else if !cli.quiet {
        println!("wrote {}", path.display());
        if report["main_config"]["sourced"] == Value::Bool(true) {
            println!(
                "{} already loads it ({})",
                report["main_config"]["path"].as_str().unwrap_or("?"),
                compositor.include_line()
            );
        } else {
            println!(
                "add to {}: {}",
                report["main_config"]["path"].as_str().unwrap_or("?"),
                compositor.include_line()
            );
        }
        for note in &notes {
            println!("note: {note}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyprland_picks_the_lua_flavour_when_hyprland_lua_exists() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            resolve("hyprland", dir.path()).unwrap(),
            Compositor::HyprlandConf
        );
        std::fs::create_dir_all(dir.path().join("hypr")).unwrap();
        std::fs::write(dir.path().join("hypr/hyprland.lua"), "").unwrap();
        assert_eq!(
            resolve("hyprland", dir.path()).unwrap(),
            Compositor::HyprlandLua
        );
        assert_eq!(
            resolve("hyprland-conf", dir.path()).unwrap(),
            Compositor::HyprlandConf
        );
        assert_eq!(
            resolve("gnome", dir.path()).unwrap_err().exit,
            Exit::InvalidArgs
        );
        assert_eq!(
            detect(Some("Hyprland"), dir.path()),
            Some(Compositor::HyprlandLua)
        );
        assert_eq!(detect(Some("GNOME"), dir.path()), None);
        assert_eq!(detect(None, dir.path()), None);
    }

    #[test]
    fn check_reports_written_and_sourced_and_the_doctor_line_names_the_fix() {
        let dir = tempfile::tempdir().unwrap();
        let report = check(Compositor::Sway, dir.path());
        assert_eq!(report["snippet"]["written"], false);
        assert_eq!(report["main_config"]["sourced"], false);
        assert!(check_line(&report).contains("not written (run `dettivo setup sway`)"));
        std::fs::create_dir_all(dir.path().join("sway")).unwrap();
        std::fs::write(dir.path().join("sway/dettivo"), "bindsym").unwrap();
        std::fs::write(dir.path().join("sway/config"), "# nothing\n").unwrap();
        let report = check(Compositor::Sway, dir.path());
        assert_eq!(report["snippet"]["written"], true);
        assert!(check_line(&report).contains("written, not sourced (add to"));
        std::fs::write(
            dir.path().join("sway/config"),
            "include ~/.config/sway/dettivo\n",
        )
        .unwrap();
        let report = check(Compositor::Sway, dir.path());
        assert_eq!(report["main_config"]["sourced"], true);
        assert!(check_line(&report).contains("sway: sourced"));
    }

    #[test]
    fn config_home_follows_xdg() {
        let env = |k: &str| (k == "HOME").then(|| std::ffi::OsString::from("/home/u"));
        assert_eq!(config_home(env), PathBuf::from("/home/u/.config"));
        let env = |k: &str| (k == "XDG_CONFIG_HOME").then(|| std::ffi::OsString::from("/x"));
        assert_eq!(config_home(env), PathBuf::from("/x"));
    }
}
