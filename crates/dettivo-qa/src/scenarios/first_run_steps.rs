//! Each first-run step opens by name (fn-35 R2, ADR 0024): `dettivo-app`
//! launched with `DETTIVO_E2E_STEP=keys`, `models` and `try` in turn over
//! the profile's daemon shows that step's title and no other, the flow
//! has exactly the three screens and no welcome title, and every screen
//! leaves its tree and screenshot for the scans.

use std::collections::BTreeMap;

use serde_json::json;

use super::app_support::{app_env, capture, close, reset_journal};
use super::daemon::DaemonHandle;
use super::first_run_support::write_config;
use super::{Context, Scenario, binary};
use crate::driver::{Driver, Launch};

/// The steps with the title each one carries (`docs/qa/a11y-names.md`).
pub const STEPS: &[(&str, &str)] = &[("keys", "Keys"), ("models", "Models"), ("try", "Try it")];

/// Titles a first-run screen never shows: a welcome screen was ruled out
/// (ADR 0024), and a fourth step has no name to be found under.
const NEVER: &[&str] = &["Welcome", "Welcome to Dettivo", "Get started", "Finish"];

/// The scenario.
pub struct FirstRunSteps;

impl Scenario for FirstRunSteps {
    fn id(&self) -> &'static str {
        "first_run_steps"
    }

    fn summary(&self) -> &'static str {
        "each first-run step opens by name through DETTIVO_E2E_STEP with its title, and the flow has exactly three screens"
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
        let mut seen = Vec::new();
        for (step, title) in STEPS {
            reset_journal(ctx);
            let mut env = app_env(ctx, "onboarding", None);
            env.remove("DETTIVO_E2E_OPEN");
            env.insert("DETTIVO_E2E_STEP".into(), (*step).into());
            let app = driver
                .launch(
                    &Launch {
                        program: app_binary.clone(),
                        args: Vec::new(),
                        env,
                    },
                    ctx.timeout,
                )
                .map_err(|e| format!("launch dettivo-app on {step}: {e}"))?;
            ctx.profile.track_pid("dettivo-app", app.pid);
            let result = (|| {
                driver
                    .wait_for_label(&app, title, ctx.timeout)
                    .map_err(|e| format!("{step} never showed {title:?}: {e}"))?;
                let tree = capture(driver, &app, ctx, step)?;
                for (other, other_title) in STEPS.iter().filter(|(s, _)| s != step) {
                    // Every step's heading is its title; another step's
                    // title on screen means the wrong screen opened.
                    if tree
                        .iter()
                        .any(|e| e.role == "heading" && e.name == *other_title)
                    {
                        return Err(format!(
                            "{step} shows the heading of {other} ({other_title:?})"
                        ));
                    }
                }
                if let Some(bad) = tree.iter().find(|e| NEVER.contains(&e.name.as_str())) {
                    return Err(format!(
                        "{step} shows a screen the flow does not have: {:?}",
                        bad.name
                    ));
                }
                seen.push(json!({"step": step, "title": title, "elements": tree.len()}));
                Ok(())
            })();
            close(driver, &app)?;
            result?;
            ctx.timings.mark(step);
        }
        std::fs::write(
            ctx.evidence_dir.join("first-run-steps.json"),
            serde_json::to_string_pretty(&json!({"steps": seen, "count": STEPS.len()}))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("first-run-steps.json".into());
        Ok(())
    }
}
