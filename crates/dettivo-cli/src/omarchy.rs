//! `dettivo setup omarchy`: the whole Omarchy install in one command.
//! Enables `dettivod.socket`, writes the Hyprland Lua binding snippet the
//! `hyprland` arm writes, puts the plugin folder shipped at
//! `/usr/share/dettivo/omarchy` under `~/.config/omarchy/plugins/` (or
//! clones the mirror with `--git`), enables it in the shell, then reloads
//! Hyprland and the shell. `--check` reports each step's state as JSON,
//! `--stdout` prints what the command would write and run, and
//! `dettivo doctor` shows the same facts on one `omarchy` line.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use crate::client::{Client, SOCKET_UNIT};
use crate::exit::{Exit, Failure};
use crate::omarchy_check::{check, doctor_line, plugin_facts};
use crate::{Cli, output, setup};

/// The plugin id the manifest carries.
pub const PLUGIN_ID: &str = "gmickel.dettivo";
/// The session-bus name the panel claims while it hosts the pill.
pub const PANEL_BUS_NAME: &str = "dev.dettivo.OmarchyPanel";
/// Where the package installs the plugin folder.
pub const INSTALLED_PLUGIN_DIR: &str = "/usr/share/dettivo/omarchy";
/// Overrides the plugin source for a build tree or a QA profile.
pub const PLUGIN_DIR_VAR: &str = "DETTIVO_OMARCHY_PLUGIN_DIR";

/// Where the plugin folder is read from.
pub fn plugin_source(env: impl Fn(&str) -> Option<std::ffi::OsString>) -> PathBuf {
    match env(PLUGIN_DIR_VAR).filter(|v| !v.is_empty()) {
        Some(v) => PathBuf::from(v),
        None => PathBuf::from(INSTALLED_PLUGIN_DIR),
    }
}

/// Where the shell discovers user plugins: `<config home>/omarchy/plugins`.
pub fn plugins_dir(config_home: &Path) -> PathBuf {
    config_home.join("omarchy/plugins")
}

/// True when `program` resolves on `PATH`.
pub fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

/// What one command did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    /// The command line, for the report.
    pub command: String,
    /// The exit code, `None` when the program could not start.
    pub code: Option<i32>,
    /// Trimmed stdout.
    pub stdout: String,
    /// Trimmed stderr.
    pub stderr: String,
}

impl Ran {
    /// True on exit 0.
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }
}

/// Runs `program args` and captures the outcome; never fails the caller.
pub fn run_command(program: &str, args: &[&str]) -> Ran {
    let command = std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");
    match Command::new(program).args(args).output() {
        Ok(out) => Ran {
            command,
            code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        },
        Err(e) => Ran {
            command,
            code: None,
            stdout: String::new(),
            stderr: e.to_string(),
        },
    }
}

/// One step of the install as the report shows it.
fn step(state: &str, detail: impl Into<String>, ran: Option<&Ran>) -> Value {
    let mut v = json!({ "state": state, "detail": detail.into() });
    if let Some(r) = ran {
        v["command"] = json!(r.command);
        v["exit_code"] = json!(r.code);
        if !r.stderr.is_empty() {
            v["stderr"] = json!(r.stderr);
        }
    }
    v
}

fn failed(ran: &Ran, what: &str) -> Value {
    step(
        "failed",
        format!(
            "{what}: `{}` exited {}",
            ran.command,
            ran.code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "without starting".into())
        ),
        Some(ran),
    )
}

/// Copies the plugin folder into the plugins directory, replacing what is
/// there unless it is a git checkout (then `omarchy plugin update` owns
/// it). Symlinks are never copied: the shell refuses them.
pub fn install_copy(source: &Path, plugins_dir: &Path) -> Result<PathBuf, String> {
    let manifest = source.join("manifest.json");
    if !manifest.is_file() {
        return Err(format!(
            "no plugin at {} (manifest.json missing); set {PLUGIN_DIR_VAR} or install dettivo",
            source.display()
        ));
    }
    let target = plugins_dir.join(PLUGIN_ID);
    if target.join(".git").exists() {
        return Ok(target);
    }
    if target.exists() {
        std::fs::remove_dir_all(&target)
            .map_err(|e| format!("cannot replace {}: {e}", target.display()))?;
    }
    copy_tree(source, &target)?;
    Ok(target)
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("cannot create {}: {e}", to.display()))?;
    for entry in
        std::fs::read_dir(from).map_err(|e| format!("cannot read {}: {e}", from.display()))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let name = entry.file_name();
        if kind.is_symlink() || name == ".git" {
            continue;
        }
        let dest = to.join(&name);
        if kind.is_dir() {
            copy_tree(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)
                .map_err(|e| format!("cannot copy {}: {e}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// Runs `dettivo setup omarchy`.
pub fn run(
    cli: &Cli,
    client: &Client,
    stdout: bool,
    check_only: bool,
    no_plugin: bool,
    git: Option<&str>,
) -> Result<(), Failure> {
    let home = setup::config_home(|k| std::env::var_os(k));
    if check_only {
        let report = check(&home);
        if cli.json {
            output::result(cli, "setup.omarchy.check", &report);
        } else if !cli.quiet {
            print!("{}", doctor_line(&report));
            print!("{}", setup::check_line(&report["snippet"]));
        }
        let failed = ["socket", "snippet", "plugin"]
            .iter()
            .filter(|k| report[k]["state"] == "failed")
            .map(|k| k.to_string())
            .collect::<Vec<_>>();
        return if failed.is_empty() {
            Ok(())
        } else {
            Err(Failure::reported(
                Exit::Failure,
                format!("steps not complete: {}", failed.join(", ")),
            ))
        };
    }
    let source = plugin_source(|k| std::env::var_os(k));
    if stdout {
        setup::run(cli, client, "hyprland-lua", true, false)?;
        if !cli.quiet {
            eprintln!("would run: systemctl --user enable --now {SOCKET_UNIT}");
            if !no_plugin {
                match git {
                    Some(url) => eprintln!("would run: omarchy plugin add {url} --enable --yes"),
                    None => eprintln!(
                        "would copy {} to {} and run: omarchy plugin enable {PLUGIN_ID} right",
                        source.display(),
                        plugins_dir(&home).join(PLUGIN_ID).display()
                    ),
                }
            }
            eprintln!("would run: hyprctl reload; omarchy-shell shell rescanPlugins");
        }
        return Ok(());
    }
    let mut steps = serde_json::Map::new();
    let socket = run_command("systemctl", &["--user", "enable", "--now", SOCKET_UNIT]);
    steps.insert(
        "socket".into(),
        if socket.ok() {
            step(
                "ok",
                format!("{SOCKET_UNIT} enabled and active"),
                Some(&socket),
            )
        } else {
            failed(&socket, "enabling the socket")
        },
    );
    let install_snippet = || -> Result<Value, Failure> {
        let compositor = setup::resolve("hyprland", &home)?;
        let (path, _notes) = setup::write(client, compositor, &home)?;
        let snippet = setup::check(compositor, &home);
        Ok(step(
            if snippet["main_config"]["sourced"] == Value::Bool(true) {
                "ok"
            } else {
                "written"
            },
            if compositor == dettivo_hotkeys::snippet::Compositor::HyprlandLua {
                format!("activated shortcuts from {}", path.display())
            } else {
                format!(
                    "wrote {}; load it with: {}",
                    path.display(),
                    compositor.include_line()
                )
            },
            None,
        ))
    };
    steps.insert(
        "snippet".into(),
        install_snippet().unwrap_or_else(|error| step("failed", error.message, None)),
    );
    steps.insert(
        "plugin".into(),
        install_plugin(&home, &source, no_plugin, git),
    );
    let hypr = if on_path("hyprctl") {
        let ran = run_command("hyprctl", &["reload"]);
        if ran.ok() {
            step("ok", "Hyprland reloaded", Some(&ran))
        } else {
            failed(&ran, "reloading Hyprland")
        }
    } else {
        step("skipped", "hyprctl is not on PATH", None)
    };
    steps.insert("hyprland".into(), hypr);
    let shell = if on_path("omarchy-shell") {
        let ran = run_command("omarchy-shell", &["shell", "rescanPlugins"]);
        if ran.ok() {
            step("ok", "shell rescanned its plugins", Some(&ran))
        } else {
            failed(&ran, "reloading the shell")
        }
    } else {
        step("skipped", "omarchy-shell is not on PATH", None)
    };
    steps.insert("shell".into(), shell);
    let report = Value::Object(steps);
    if cli.json {
        output::result(cli, "setup.omarchy", &report);
    } else if !cli.quiet {
        for (name, s) in report.as_object().into_iter().flatten() {
            println!(
                "{name:<9} {}: {}",
                s["state"].as_str().unwrap_or("?"),
                s["detail"].as_str().unwrap_or("")
            );
        }
    }
    let failures = report
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, s)| s["state"] == "failed")
        .map(|(k, _)| k.clone())
        .collect::<Vec<_>>();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(Failure::reported(
            Exit::Failure,
            format!("steps failed: {}", failures.join(", ")),
        ))
    }
}

fn install_plugin(home: &Path, source: &Path, no_plugin: bool, git: Option<&str>) -> Value {
    if no_plugin {
        return step("skipped", "--no-plugin", None);
    }
    if !on_path("omarchy") {
        return step(
            "skipped",
            "the omarchy command is not on PATH; dettivo-osd.service keeps the pill",
            None,
        );
    }
    let installed = plugins_dir(home)
        .join(PLUGIN_ID)
        .join("manifest.json")
        .is_file();
    if let Some(url) = git {
        if !installed {
            let ran = run_command("omarchy", &["plugin", "add", url, "--enable", "--yes"]);
            return if ran.ok() {
                step(
                    "ok",
                    format!("added {PLUGIN_ID} from {url} and enabled it"),
                    Some(&ran),
                )
            } else {
                failed(&ran, "adding the plugin")
            };
        }
    } else {
        let target = match install_copy(source, &plugins_dir(home)) {
            Ok(t) => t,
            Err(e) => return step("failed", e, None),
        };
        let rescan = run_command("omarchy-shell", &["-q", "shell", "rescanPlugins"]);
        if !rescan.ok() {
            return failed(&rescan, "rescanning the shell's plugins");
        }
        if plugin_facts(home)["enabled"] == Value::Bool(true) {
            return step(
                "ok",
                format!("{PLUGIN_ID} at {} is enabled", target.display()),
                None,
            );
        }
    }
    let ran = run_command("omarchy", &["plugin", "enable", PLUGIN_ID, "right"]);
    if ran.ok() {
        step(
            "ok",
            format!("{PLUGIN_ID} enabled in the bar's right section"),
            Some(&ran),
        )
    } else {
        failed(&ran, "enabling the plugin")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_plugin() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../omarchy")
    }

    #[test]
    fn the_manifest_carries_the_plugin_contract() {
        let text = std::fs::read_to_string(repo_plugin().join("manifest.json")).unwrap();
        let m: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(m["schemaVersion"], 1);
        assert_eq!(m["id"], PLUGIN_ID);
        assert_eq!(
            m["version"],
            env!("CARGO_PKG_VERSION"),
            "the manifest version follows Cargo.toml"
        );
        let kinds = m["kinds"].as_array().unwrap();
        assert!(kinds.contains(&json!("bar-widget")) && kinds.contains(&json!("panel")));
        assert_eq!(m["keepLoaded"], true);
        for (kind, entry) in [("barWidget", "BarWidget.qml"), ("panel", "Panel.qml")] {
            assert_eq!(m["entryPoints"][kind], entry);
            assert!(repo_plugin().join(entry).is_file(), "{entry} exists");
        }
        assert_eq!(m["barWidget"]["category"], "Developer Tools");
        assert_eq!(m["barWidget"]["defaultSection"], "right");
        let keys: Vec<&str> = m["barWidget"]["schema"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["key"].as_str().unwrap())
            .collect();
        assert_eq!(keys, ["glyph", "levelMeter", "osd"]);
        assert!(m["omarchy"]["minDettivo"].is_string());
    }

    #[test]
    fn install_copies_the_folder_and_leaves_a_git_checkout_alone() {
        let dir = tempfile::tempdir().unwrap();
        let plugins = dir.path().join("plugins");
        let target = install_copy(&repo_plugin(), &plugins).unwrap();
        assert!(target.join("manifest.json").is_file());
        assert!(target.join("BarWidget.qml").is_file());
        assert_eq!(target, plugins.join(PLUGIN_ID));
        std::fs::write(target.join("local-edit"), "x").unwrap();
        install_copy(&repo_plugin(), &plugins).unwrap();
        assert!(
            !target.join("local-edit").exists(),
            "a plain copy is replaced whole"
        );
        std::fs::create_dir_all(target.join(".git")).unwrap();
        std::fs::write(target.join("local-edit"), "x").unwrap();
        install_copy(&repo_plugin(), &plugins).unwrap();
        assert!(
            target.join("local-edit").exists(),
            "a git checkout is the shell's to update"
        );
        let err = install_copy(&dir.path().join("nowhere"), &plugins).unwrap_err();
        assert!(err.contains(PLUGIN_DIR_VAR));
    }

    #[test]
    fn plugin_source_follows_the_override() {
        assert_eq!(plugin_source(|_| None), PathBuf::from(INSTALLED_PLUGIN_DIR));
        let env = |k: &str| (k == PLUGIN_DIR_VAR).then(|| std::ffi::OsString::from("/x"));
        assert_eq!(plugin_source(env), PathBuf::from("/x"));
    }
}
