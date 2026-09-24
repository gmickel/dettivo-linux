//! Settings round trip (fn-26 R1, R3, R4, R5): `dettivo-app` opens every
//! settings section over the profile's daemon, the drive edits every key
//! the section shows through its own control (a field is typed into, a
//! switch clicked, a segment or a combo box picked), reads the key back
//! through `config.get` with source `file`, restores the default through
//! `config.unset`, and sees the comment planted in the profile's file
//! survive every write. A key the environment sets shows its badge, an
//! invalid value shows the daemon's refusal under the field, a hand edit
//! of the file refreshes the open route, the column's action opens the
//! file, the Agents route writes a host through `dettivo mcp config`, and
//! Diagnostics shows the doctor report.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Value, json};

use super::app_support::{app_command, app_env, close, daemon_config, wait_status};
use super::daemon::DaemonHandle;
use super::settings_support::{excluded_keys, left_to_file, locked_by, route_keys};
use super::{Context, Scenario, binary};
use crate::driver::{App, Driver, Element, Launch};
use crate::negative_text;

/// The comment the profile's file carries; it must be there at the end.
const PLANTED: &str = "# planted: this comment survives every write the routes make";

/// The scenario: every section at once (`settings_roundtrip`), or one
/// section on its own (`settings_roundtrip.<section>`, the GUI pack's
/// eight steps, so a failing section is named).
pub struct SettingsRoundtrip {
    section: Option<&'static str>,
}

/// The eight sections with the step id each one runs under.
const SECTIONS: &[(&str, &str)] = &[
    ("general", "settings_roundtrip.general"),
    ("hotkeys", "settings_roundtrip.hotkeys"),
    ("models", "settings_roundtrip.models"),
    ("polish", "settings_roundtrip.polish"),
    ("insertion", "settings_roundtrip.insertion"),
    ("meetings", "settings_roundtrip.meetings"),
    ("agents", "settings_roundtrip.agents"),
    ("diagnostics", "settings_roundtrip.diagnostics"),
];

impl SettingsRoundtrip {
    /// Every section in one drive.
    pub fn whole() -> Self {
        Self { section: None }
    }

    /// One section by its step id; `None` for any other id.
    pub fn sectioned(id: &str) -> Option<Self> {
        SECTIONS
            .iter()
            .find(|(_, step)| *step == id)
            .map(|(section, _)| Self {
                section: Some(section),
            })
    }

    /// The step ids of the eight sections, in nav order.
    pub fn section_ids() -> Vec<&'static str> {
        SECTIONS.iter().map(|(_, id)| *id).collect()
    }
}

/// What one key went through.
#[derive(Debug, Serialize)]
pub(super) struct KeyOutcome {
    pub(super) key: String,
    pub(super) section: String,
    pub(super) control: String,
    pub(super) outcome: String,
    pub(super) value: Value,
}

/// What `roundtrip.json` records.
#[derive(Debug, Serialize)]
struct Evidence {
    keys: Vec<KeyOutcome>,
    excluded: Vec<(String, String)>,
    comment_survived: bool,
    refusal: String,
    hand_edit_seen: String,
    opened: String,
    host_file: String,
    doctor_lines: usize,
}

impl Scenario for SettingsRoundtrip {
    fn id(&self) -> &'static str {
        match self.section {
            None => "settings_roundtrip",
            Some(section) => SECTIONS
                .iter()
                .find(|(s, _)| *s == section)
                .map_or("settings_roundtrip", |(_, id)| id),
        }
    }

    fn summary(&self) -> &'static str {
        match self.section {
            None => {
                "every settings key round-trips through its control and config.get, the comment survives, unset restores the default"
            }
            Some(_) => {
                "one settings section's keys round-trip through their controls, with the section's own checks"
            }
        }
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        binary(ctx.repo_root, "dettivo-app")?;
        binary(ctx.repo_root, "dettivo")?;
        binary(ctx.repo_root, "dettivod").map(|_| ())
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let app_binary = binary(ctx.repo_root, "dettivo-app")?;
        let bin_dir = binary(ctx.repo_root, "dettivo")?
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let config = format!("{PLANTED}\n{}", daemon_config(ctx.repo_root));
        let daemon = DaemonHandle::spawn(
            &dettivod,
            ctx.profile,
            &config,
            &BTreeMap::new(),
            ctx.timeout,
        )?;
        ctx.timings.mark("daemon");

        // The column's action runs `xdg-open`; the profile's stub records
        // what it was handed.
        let stub_dir = ctx.profile.root.join("bin");
        std::fs::create_dir_all(&stub_dir).map_err(|e| e.to_string())?;
        let opened = ctx.profile.root.join("opened.txt");
        let stub = stub_dir.join("xdg-open");
        std::fs::write(
            &stub,
            format!("#!/bin/sh\nprintf '%s\\n' \"$1\" >> {}\n", opened.display()),
        )
        .map_err(|e| e.to_string())?;
        std::fs::set_permissions(&stub, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .map_err(|e| e.to_string())?;

        let mut env = app_env(ctx, "settings", Some(self.section.unwrap_or("general")));
        let path = std::env::var("PATH").unwrap_or_default();
        env.insert(
            "PATH".into(),
            format!("{}:{}:{path}", stub_dir.display(), bin_dir.display()),
        );
        let app = driver
            .launch(
                &Launch {
                    program: app_binary,
                    args: Vec::new(),
                    env,
                },
                ctx.timeout,
            )
            .map_err(|e| format!("launch dettivo-app: {e}"))?;
        ctx.profile.track_pid("dettivo-app", app.pid);
        // An X11 window can exist before the app's control socket is ready.
        wait_status(ctx, ctx.timeout, "the app control socket", |s| {
            s["ok"] == json!(true)
        })?;
        let mut run = Run {
            driver,
            ctx,
            app: &app,
            daemon: &daemon,
            bin_dir,
            opened,
        };
        let result = run.drive(self.section);
        close(driver, &app)?;
        result
    }
}

/// One drive over one app.
pub(super) struct Run<'a, 'b> {
    pub(super) driver: &'a mut dyn Driver,
    pub(super) ctx: &'a mut Context<'b>,
    pub(super) app: &'a App,
    pub(super) daemon: &'a DaemonHandle,
    pub(super) bin_dir: PathBuf,
    pub(super) opened: PathBuf,
}

impl Run<'_, '_> {
    fn title(section: &str) -> String {
        let mut c = section.chars();
        let head = c
            .next()
            .map(|h| h.to_uppercase().to_string())
            .unwrap_or_default();
        format!("Settings / {head}{}", c.as_str())
    }

    pub(super) fn config_file(&self) -> PathBuf {
        self.ctx.profile.root.join("cfg/dettivo/config.toml")
    }

    /// `config.get key`: the value and its source.
    pub(super) fn get(&self, key: &str) -> Result<(Value, String), String> {
        let answer = self.daemon.call("config.get", json!({"key": key}))?;
        let entry = answer["entries"]
            .as_array()
            .and_then(|e| e.first())
            .ok_or_else(|| format!("config.get {key} answered no entry"))?;
        Ok((
            entry["value"].clone(),
            entry["source"].as_str().unwrap_or_default().to_string(),
        ))
    }

    /// Waits until `config.get key` satisfies `ok`.
    pub(super) fn wait_get(
        &self,
        key: &str,
        timeout: Duration,
        ok: impl Fn(&Value, &str) -> bool,
    ) -> Result<(Value, String), String> {
        let deadline = Instant::now() + timeout;
        loop {
            let last = self.get(key)?;
            if ok(&last.0, &last.1) {
                return Ok(last);
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "{key} is {} from {} after {timeout:?}",
                    last.0, last.1
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Opens a section, scrolled to `key` when given, with the window
    /// raised and active: on a live desktop the focus can move between
    /// two keys, and a keystroke into another window is not a round trip.
    pub(super) fn open(&mut self, section: &str, key: &str) -> Result<(), String> {
        let route = format!("settings.{section}");
        app_command(
            self.ctx,
            &json!({"cmd": "open", "route": route, "arg": key}),
        )?;
        app_command(self.ctx, &json!({"cmd": "raise"}))?;
        let _ = wait_status(self.ctx, Duration::from_secs(2), "an active window", |s| {
            s["active"] == json!(true)
        });
        self.driver
            .wait_for_label(self.app, &Self::title(section), self.ctx.timeout)
            .map_err(|e| format!("{route}: {e}"))?;
        Ok(())
    }

    pub(super) fn click_named(&mut self, name: &str) -> Result<Element, String> {
        let element = self
            .driver
            .wait_for_label(self.app, name, self.ctx.timeout)
            .map_err(|e| format!("{name:?}: {e}"))?;
        self.driver
            .click(self.app, &element)
            .map_err(|e| format!("click {name:?}: {e}"))?;
        Ok(element)
    }

    pub(super) fn snapshot(&mut self) -> Result<Vec<Element>, String> {
        self.driver
            .snapshot(self.app)
            .map_err(|e| format!("snapshot: {e}"))
    }

    /// The tree and a screenshot; the negative text scan runs over the
    /// tree, with the profile's own paths (which the product shows
    /// relative to home) set aside.
    pub(super) fn capture(&mut self, name: &str) -> Result<Vec<Element>, String> {
        let tree = self.snapshot()?;
        let file = format!("tree-{name}.json");
        std::fs::write(
            self.ctx.evidence_dir.join(&file),
            serde_json::to_string_pretty(&tree).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        self.ctx.evidence.push(file);
        let shot = format!("screenshot-{name}.png");
        self.driver
            .screenshot(self.app, &self.ctx.evidence_dir.join(&shot))
            .map_err(|e| format!("screenshot {name}: {e}"))?;
        self.ctx.evidence.push(shot);
        let root = self.ctx.profile.root.to_string_lossy().into_owned();
        let findings: Vec<String> = negative_text::findings(&tree)
            .iter()
            .filter(|f| !f.name.contains(&root))
            .map(ToString::to_string)
            .collect();
        if !findings.is_empty() {
            return Err(format!(
                "developer text on the {name} route: {}",
                findings.join("; ")
            ));
        }
        Ok(tree)
    }

    /// The key loop over every route, or over `only`, then the checks
    /// that belong to the routes driven.
    fn drive(&mut self, only: Option<&str>) -> Result<(), String> {
        let registry = self.daemon.call("config.keys", json!({}))?;
        let kinds: BTreeMap<String, String> = registry["keys"]
            .as_array()
            .map(|keys| {
                keys.iter()
                    .filter_map(|k| {
                        Some((
                            k["key"].as_str()?.to_string(),
                            k["kind"].as_str()?.to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let routes = route_keys(self.ctx.repo_root)?;
        if only.is_some_and(|s| !routes.iter().any(|r| r.section == s)) {
            return Err(format!(
                "{} is not a settings section; the routes are {}",
                only.unwrap_or_default(),
                routes
                    .iter()
                    .map(|r| r.section.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let mut keys = Vec::new();
        for route in routes
            .iter()
            .filter(|r| only.is_none_or(|s| s == r.section))
        {
            self.open(&route.section, "")?;
            self.capture(&route.section)?;
            for key in &route.keys {
                let kind = kinds
                    .get(key)
                    .cloned()
                    .ok_or_else(|| format!("{key} is not in config.keys"))?;
                if let Some(reason) = left_to_file(key) {
                    keys.push(KeyOutcome {
                        key: key.clone(),
                        section: route.section.clone(),
                        control: "file".into(),
                        outcome: reason.into(),
                        value: Value::Null,
                    });
                    continue;
                }
                if let Some(variable) = locked_by(key) {
                    self.open(&route.section, key)?;
                    self.driver
                        .wait_for_label(self.app, &format!("env · {variable}"), self.ctx.timeout)
                        .map_err(|e| format!("{key}: the environment badge: {e}"))?;
                    let (value, source) = self.get(key)?;
                    if source != "environment" {
                        return Err(format!("{key} comes from {source}, expected environment"));
                    }
                    keys.push(KeyOutcome {
                        key: key.clone(),
                        section: route.section.clone(),
                        control: "locked".into(),
                        outcome: format!("locked by {variable}"),
                        value,
                    });
                    continue;
                }
                keys.push(self.edit(&route.section, key, &kind)?);
            }
            self.ctx.timings.mark(&route.section);
        }
        let text = std::fs::read_to_string(self.config_file()).map_err(|e| e.to_string())?;
        if !text.contains(PLANTED) {
            return Err("the planted comment is gone from config.toml".into());
        }

        let checks = super::settings_checks::after_keys(self, only)?;
        let evidence = Evidence {
            keys,
            excluded: excluded_keys(self.ctx.repo_root)?,
            comment_survived: true,
            refusal: checks.refusal,
            hand_edit_seen: checks.hand_edit_seen,
            opened: checks.opened,
            host_file: checks.host_file,
            doctor_lines: checks.doctor_lines,
        };
        std::fs::write(
            self.ctx.evidence_dir.join("roundtrip.json"),
            serde_json::to_string_pretty(&evidence).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        self.ctx.evidence.push("roundtrip.json".into());
        Ok(())
    }
}
