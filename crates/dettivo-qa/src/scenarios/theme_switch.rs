//! The theme switch (fn-20 R4): the pill and the app are on screen with
//! the Black Gold fixture as their Omarchy theme, the scenario replaces
//! the theme directory the way `omarchy theme set` does (a staged
//! directory moved over the old one), and every surface's pacing
//! collector reports when the new palette applied and when the first
//! frame after it swapped. The switch must show within 100 ms and the
//! dropped frames stay within the NFR-12 budget on a real display; under
//! CI the numbers are recorded and the report's presence is the gate.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;

use super::{Context, Scenario, binary};
use crate::driver::{App, Driver, Launch};
use crate::pacing::{self, Animating, DriveVerdict};

/// The theme the surfaces start in.
const FROM_THEME: &str = "black-gold";
/// The theme they switch to.
const TO_THEME: &str = "tokyo-night";
/// How long the surfaces stay up after the switch, so the pacing
/// evidence covers the re-themed frames as well as the switch itself.
const DWELL_AFTER_SWITCH: Duration = Duration::from_secs(2);

/// The scenario.
pub struct ThemeSwitch;

/// One surface under the switch.
struct Surface {
    name: &'static str,
    app: App,
    pacing_file: PathBuf,
}

/// The scenario's own report, `theme-switch.json`.
#[derive(Debug, Serialize)]
struct SwitchReport {
    from: String,
    to: String,
    switched_unix_ms: f64,
    gated: bool,
    surfaces: Vec<DriveVerdict>,
}

impl Scenario for ThemeSwitch {
    fn id(&self) -> &'static str {
        "theme_switch"
    }

    fn summary(&self) -> &'static str {
        "the pill and the app re-theme within 100 ms of the theme directory changing, frames paced"
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        binary(ctx.repo_root, "dettivo-osd")?;
        binary(ctx.repo_root, "dettivo-app")?;
        for theme in [FROM_THEME, TO_THEME] {
            let dir = fixture(ctx.repo_root, theme);
            for file in ["colors.toml", "shell.toml"] {
                if !dir.join(file).is_file() {
                    return Err(format!(
                        "theme fixture missing: {}",
                        dir.join(file).display()
                    ));
                }
            }
        }
        Ok(())
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let mut surfaces = Vec::new();
        let result = Self::drive(driver, ctx, &mut surfaces);
        if result.is_err() {
            for s in &surfaces {
                let _ = driver.close(&s.app);
            }
        }
        result
    }
}

fn fixture(repo: &Path, theme: &str) -> PathBuf {
    repo.join("qt/fixtures/themes").join(theme)
}

fn now_unix_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

/// Copies a theme's files into `dir`.
fn stage(theme_dir: &Path, into: &Path) -> Result<(), String> {
    std::fs::create_dir_all(into).map_err(|e| format!("{}: {e}", into.display()))?;
    for file in ["colors.toml", "shell.toml"] {
        std::fs::copy(theme_dir.join(file), into.join(file))
            .map_err(|e| format!("stage {file}: {e}"))?;
    }
    Ok(())
}

/// The moment the collector saw the new theme, plus its first frame after
/// it, relative to the switch; `None` until the change shows.
fn apply_latency(summary: &Value, switched_unix_ms: f64, background: &str) -> Option<f64> {
    summary["theme_changes"].as_array()?.iter().find_map(|c| {
        let applied = c["applied_unix_ms"].as_f64()?;
        if applied + 1.0 < switched_unix_ms || c["background"].as_str() != Some(background) {
            return None;
        }
        let frame = c["frame_after_ms"].as_f64()?;
        Some(applied + frame - switched_unix_ms)
    })
}

/// Waits until the process is gone or a zombie (the driver reaps it), so
/// the collector's last summary, written as the process quits, is on disk.
fn wait_exit(pid: u32, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let status = match std::fs::read_to_string(format!("/proc/{pid}/status")) {
            Ok(s) => s,
            Err(_) => return,
        };
        if status
            .lines()
            .any(|l| l.starts_with("State:") && l.contains('Z'))
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The background colour a theme's colors.toml names, lower case.
fn background_of(theme_dir: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(theme_dir.join("colors.toml")).map_err(|e| e.to_string())?;
    text.lines()
        .find_map(|l| {
            let (key, value) = l.split_once('=')?;
            (key.trim() == "background").then(|| value.trim().trim_matches('"').to_lowercase())
        })
        .ok_or_else(|| format!("{}: no background", theme_dir.display()))
}

impl ThemeSwitch {
    fn drive(
        driver: &mut dyn Driver,
        ctx: &mut Context<'_>,
        surfaces: &mut Vec<Surface>,
    ) -> Result<(), String> {
        // The profile's own Omarchy state: theme/ is what the surfaces
        // watch, next-theme/ is staged beside it as omarchy-theme-set does.
        let omarchy = ctx.profile.root.join("omarchy");
        let theme_dir = omarchy.join("theme");
        stage(&fixture(ctx.repo_root, FROM_THEME), &theme_dir)?;
        let to_background = background_of(&fixture(ctx.repo_root, TO_THEME))?;

        for (name, extra) in [
            ("dettivo-osd", vec![("DETTIVO_E2E_OSD_STATE", "listening")]),
            ("dettivo-app", Vec::new()),
        ] {
            let program = binary(ctx.repo_root, name)?;
            let pacing_file = ctx.evidence_dir.join(format!("{name}.pacing.raw.json"));
            let mut env: BTreeMap<String, String> = ctx.profile.env();
            env.insert(
                "DETTIVO_OMARCHY_THEME_DIR".into(),
                theme_dir.to_string_lossy().into_owned(),
            );
            env.insert(
                "DETTIVO_QA_PACING".into(),
                pacing_file.to_string_lossy().into_owned(),
            );
            for (k, v) in extra {
                env.insert(k.into(), v.into());
            }
            let app = driver
                .launch(
                    &Launch {
                        program,
                        args: Vec::new(),
                        env,
                    },
                    ctx.timeout,
                )
                .map_err(|e| format!("launch {name}: {e}"))?;
            ctx.profile.track_pid(name, app.pid);
            surfaces.push(Surface {
                name,
                app,
                pacing_file,
            });
        }
        ctx.timings.mark("launch");
        // Both surfaces have painted at least once before the switch.
        for s in surfaces.iter() {
            let shot = format!("{}-before.png", s.name);
            driver
                .screenshot(&s.app, &ctx.evidence_dir.join(&shot))
                .map_err(|e| format!("screenshot {}: {e}", s.name))?;
            ctx.evidence.push(shot);
        }

        // The switch, as omarchy-theme-set does it: stage, remove, move.
        let next = omarchy.join("next-theme");
        stage(&fixture(ctx.repo_root, TO_THEME), &next)?;
        std::fs::remove_dir_all(&theme_dir).map_err(|e| format!("remove theme: {e}"))?;
        let switched_unix_ms = now_unix_ms();
        std::fs::rename(&next, &theme_dir).map_err(|e| format!("move next-theme: {e}"))?;
        ctx.timings.mark("switch");
        std::thread::sleep(DWELL_AFTER_SWITCH);

        let gated = pacing::gate_active();
        let mut verdicts = Vec::new();
        for s in surfaces.iter() {
            let deadline = Instant::now() + ctx.timeout;
            let latency = loop {
                let seen = std::fs::read_to_string(&s.pacing_file)
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    .and_then(|v| apply_latency(&v, switched_unix_ms, &to_background));
                if let Some(ms) = seen {
                    break Some(ms);
                }
                if Instant::now() > deadline {
                    break None;
                }
                std::thread::sleep(Duration::from_millis(50));
            };
            let shot = format!("{}-after.png", s.name);
            driver
                .screenshot(&s.app, &ctx.evidence_dir.join(&shot))
                .map_err(|e| format!("screenshot {}: {e}", s.name))?;
            ctx.evidence.push(shot);
            let Some(latency) = latency else {
                return Err(format!(
                    "{} never reported the {TO_THEME} palette (background {to_background}) within {:?}; see {}",
                    s.name,
                    ctx.timeout,
                    s.pacing_file.display()
                ));
            };
            driver
                .close(&s.app)
                .map_err(|e| format!("close {}: {e}", s.name))?;
            wait_exit(s.app.pid, Duration::from_secs(5));
            // The pill's bars animate the whole time; the app repaints on
            // the switch alone, so its swap gaps are idle time.
            let animating = if s.name == "dettivo-osd" {
                Animating::Always
            } else {
                Animating::Never
            };
            let (verdict, file) = pacing::collect(
                &s.pacing_file,
                "theme_switch",
                ctx.evidence_dir,
                Some(latency),
                gated,
                animating,
            )?;
            ctx.evidence.push(file);
            verdicts.push(verdict);
        }
        ctx.timings.mark("applied");

        let report = SwitchReport {
            from: FROM_THEME.into(),
            to: TO_THEME.into(),
            switched_unix_ms,
            gated,
            surfaces: verdicts,
        };
        std::fs::write(
            ctx.evidence_dir.join("theme-switch.json"),
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("theme-switch.json".into());
        let breaches: Vec<String> = report
            .surfaces
            .iter()
            .filter(|v| !v.ok)
            .flat_map(|v| v.reasons.iter().map(move |r| format!("{}: {r}", v.surface)))
            .collect();
        if breaches.is_empty() {
            Ok(())
        } else {
            Err(breaches.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_apply_latency_is_the_first_matching_change_plus_its_frame() {
        let summary = json!({"theme_changes": [
            {"applied_unix_ms": 1000.0, "background": "#0d0d0d", "frame_after_ms": 2.0},
            {"applied_unix_ms": 1030.0, "background": "#1a1b26"},
            {"applied_unix_ms": 1031.0, "background": "#1a1b26", "frame_after_ms": 4.5},
        ]});
        // The first tokyo-night change has no frame yet; the next carries one.
        assert_eq!(apply_latency(&summary, 1010.0, "#1a1b26"), Some(25.5));
        assert_eq!(apply_latency(&summary, 1010.0, "#000000"), None);
        // A change from before the switch never counts.
        assert_eq!(apply_latency(&summary, 1100.0, "#1a1b26"), None);
        assert_eq!(apply_latency(&json!({}), 0.0, "#1a1b26"), None);
    }

    #[test]
    fn the_background_comes_from_the_fixture() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("colors.toml"),
            "accent = \"#7AA2F7\"\nbackground = \"#1A1B26\"\n",
        )
        .unwrap();
        assert_eq!(background_of(dir.path()).unwrap(), "#1a1b26");
    }
}
