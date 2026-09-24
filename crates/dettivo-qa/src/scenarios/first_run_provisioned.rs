//! A provisioned profile never sees first run (fn-21 R1, ADR 0024): the
//! machine's tiny.en is linked into the profile, the Hyprland snippet is
//! written and sourced from `hyprland.conf` the way a dotfile manager
//! would leave it, and `dettivo-app` launched with no route opens Home,
//! with no `First run` anywhere in the tree; the completion is recorded
//! in the state file so the check never runs again.

use std::collections::BTreeMap;

use super::app_support::{app_env, capture, close, reset_journal};
use super::daemon::DaemonHandle;
use super::first_run_support::{
    golden_snippet, provision_hyprland, wait_first_run_state, write_config,
};
use super::{Context, Scenario, binary};
use crate::driver::{Driver, Launch};

/// The scenario.
pub struct FirstRunProvisioned;

impl Scenario for FirstRunProvisioned {
    fn id(&self) -> &'static str {
        "first_run_provisioned"
    }

    fn summary(&self) -> &'static str {
        "a profile with a model and a sourced snippet opens Home and never sees first run"
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        binary(ctx.repo_root, "dettivo-app")?;
        binary(ctx.repo_root, "dettivo")?;
        if !ctx
            .profile
            .root
            .join("data/dettivo/models/whisper/tiny.en/ggml-tiny.en.bin")
            .is_file()
        {
            return Err(
                "the local tiny.en model is missing (scripts/models/fetch-test-model.sh)".into(),
            );
        }
        Ok(())
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let app_binary = binary(ctx.repo_root, "dettivo-app")?;
        write_config(ctx, None)?;
        // The daemon sees a Hyprland session whatever the drive runs under,
        // so the snippet path is the one the profile provisions.
        let mut extra = BTreeMap::new();
        extra.insert("XDG_CURRENT_DESKTOP".to_string(), "Hyprland".to_string());
        let config = std::fs::read_to_string(ctx.profile.root.join("cfg/dettivo/config.toml"))
            .map_err(|e| e.to_string())?;
        let _daemon = DaemonHandle::spawn(&dettivod, ctx.profile, &config, &extra, ctx.timeout)?;
        ctx.timings.mark("daemon");
        let snippet = golden_snippet(ctx, &extra)?;
        provision_hyprland(ctx.profile, &snippet)?;
        std::fs::write(ctx.evidence_dir.join("snippet.conf"), &snippet)
            .map_err(|e| e.to_string())?;
        ctx.evidence.push("snippet.conf".into());

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
            .map_err(|e| format!("launch dettivo-app: {e}"))?;
        ctx.profile.track_pid("dettivo-app", app.pid);
        driver
            .wait_for_label(&app, "Home", ctx.timeout)
            .map_err(|e| format!("Home never opened: {e}"))?;
        // The decision lands within the first answers; the tree after a
        // settle shows Home and nothing of first run.
        std::thread::sleep(std::time::Duration::from_millis(500));
        let tree = capture(driver, &app, ctx, "home")?;
        if tree
            .iter()
            .any(|e| e.name == "First run" || e.name == "Keys")
        {
            return Err("a provisioned profile showed first run".into());
        }
        if !tree.iter().any(|e| e.name == "Home") {
            return Err("Home is not on screen".into());
        }
        ctx.timings.mark("home");
        close(driver, &app)?;
        let state = wait_first_run_state(ctx.profile, ctx.timeout, true)?;
        std::fs::write(
            ctx.evidence_dir.join("first-run-state.json"),
            serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("first-run-state.json".into());
        Ok(())
    }
}
