//! A flow closed mid-way reopens where it was (fn-35 R2, ADR 0024): the
//! app opens first run on Models in a fresh profile, the window closes
//! there, the state file records `first_run.step = "models"` with no
//! completion, and a plain relaunch with no route asked opens Models
//! again rather than Keys or Home.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde_json::json;

use super::app_support::{app_env, capture, close, reset_journal};
use super::daemon::DaemonHandle;
use super::first_run_support::write_config;
use super::{Context, Scenario, binary};
use crate::driver::{Driver, Launch};

/// The scenario.
pub struct FirstRunResume;

impl FirstRunResume {
    /// The `[first_run]` table once `step` is what it should be.
    fn wait_step(ctx: &Context<'_>, want: &str) -> Result<toml::Table, String> {
        let path = ctx.profile.root.join("state/dettivo/state.toml");
        let deadline = Instant::now() + ctx.timeout;
        loop {
            if let Ok(doc) = std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| t.parse::<toml::Table>().map_err(|e| e.to_string()))
            {
                let table = doc
                    .get("first_run")
                    .and_then(|v| v.as_table())
                    .cloned()
                    .unwrap_or_default();
                if table.get("step").and_then(|v| v.as_str()) == Some(want) {
                    return Ok(table);
                }
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "{} never recorded first_run.step = {want:?}",
                    path.display()
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Scenario for FirstRunResume {
    fn id(&self) -> &'static str {
        "first_run_resume"
    }

    fn summary(&self) -> &'static str {
        "a first run closed on Models records the step and reopens on Models"
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        binary(ctx.repo_root, "dettivo-app")?;
        binary(ctx.repo_root, "dettivo").map(|_| ())
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let app_binary = binary(ctx.repo_root, "dettivo-app")?;
        write_config(ctx, None)?;
        let mut extra = BTreeMap::new();
        extra.insert("XDG_CURRENT_DESKTOP".to_string(), "Hyprland".to_string());
        let config = std::fs::read_to_string(ctx.profile.root.join("cfg/dettivo/config.toml"))
            .map_err(|e| e.to_string())?;
        let _daemon = DaemonHandle::spawn(&dettivod, ctx.profile, &config, &extra, ctx.timeout)?;
        ctx.timings.mark("daemon");

        // Open on Models and close there.
        reset_journal(ctx);
        let mut env = app_env(ctx, "onboarding", None);
        env.remove("DETTIVO_E2E_OPEN");
        env.insert("DETTIVO_E2E_STEP".into(), "models".into());
        let app = driver
            .launch(
                &Launch {
                    program: app_binary.clone(),
                    args: Vec::new(),
                    env,
                },
                ctx.timeout,
            )
            .map_err(|e| format!("launch dettivo-app on Models: {e}"))?;
        ctx.profile.track_pid("dettivo-app", app.pid);
        driver
            .wait_for_label(&app, "Models", ctx.timeout)
            .map_err(|e| format!("Models never opened: {e}"))?;
        capture(driver, &app, ctx, "models-before-close")?;
        close(driver, &app)?;
        let state = Self::wait_step(ctx, "models")?;
        if state
            .get("completed_at")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty())
        {
            return Err("a flow closed on Models was recorded as complete".into());
        }
        ctx.timings.mark("close");

        // A plain relaunch: no route, no step asked.
        reset_journal(ctx);
        let mut env = app_env(ctx, "home", None);
        env.remove("DETTIVO_E2E_OPEN");
        let app = driver
            .launch(
                &Launch {
                    program: app_binary,
                    args: Vec::new(),
                    env,
                },
                ctx.timeout,
            )
            .map_err(|e| format!("relaunch dettivo-app: {e}"))?;
        ctx.profile.track_pid("dettivo-app", app.pid);
        driver
            .wait_for_label(&app, "Models", ctx.timeout)
            .map_err(|e| format!("the relaunch did not reopen on Models: {e}"))?;
        std::thread::sleep(Duration::from_millis(300));
        let tree = capture(driver, &app, ctx, "models-resumed")?;
        for wrong in ["Home", "Keys"] {
            if tree.iter().any(|e| e.role == "heading" && e.name == wrong) {
                return Err(format!("the relaunch opened {wrong} instead of Models"));
            }
        }
        close(driver, &app)?;
        let after = Self::wait_step(ctx, "models")?;
        ctx.timings.mark("resume");
        std::fs::write(
            ctx.evidence_dir.join("first-run-resume.json"),
            serde_json::to_string_pretty(&json!({
                "closed_on": "models",
                "state_after_close": toml::to_string(&state).unwrap_or_default(),
                "state_after_resume": toml::to_string(&after).unwrap_or_default(),
                "resumed_on": "models",
            }))
            .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("first-run-resume.json".into());
        Ok(())
    }
}
