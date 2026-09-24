//! Every route of the app (fn-17 R1, R4): a seeded daemon in the profile,
//! `dettivo-app` launched once per route through `DETTIVO_E2E_OPEN` (and
//! `DETTIVO_E2E_ROUTE` for the settings sections), the route's title read
//! through the accessibility tree, the negative text scan over the whole
//! tree, a screenshot and the tree as evidence. The Home launch records
//! the first frame and the resident set after five seconds into
//! `startup.json`; with `DETTIVO_TIMING_GATE=1` the budgets are asserted.

use std::time::{Duration, Instant};

use serde_json::json;

use super::app_support::{
    self, ANONYMOUS_BUDGET_KB, FIRST_FRAME_BUDGET_MS, ROUTES, RSS_BUDGET_KB, RouteCase, app_env,
    capture, close, daemon_config, reset_journal, timing_gate, wait_journal,
};
use super::daemon::DaemonHandle;
use super::{Context, Scenario, binary};
use crate::driver::{Driver, Launch};

/// The scenario.
pub struct AppRoutes;

impl Scenario for AppRoutes {
    fn id(&self) -> &'static str {
        "app_routes"
    }

    fn summary(&self) -> &'static str {
        "dettivo-app opens every route by name with its title, no developer text, and records the startup budget"
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        binary(ctx.repo_root, "dettivo-app").map(|_| ())
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let app_binary = binary(ctx.repo_root, "dettivo-app")?;
        let mut seed = std::collections::BTreeMap::new();
        seed.insert("DETTIVO_E2E_SEED".to_string(), "1".to_string());
        let _daemon = DaemonHandle::spawn(
            &dettivod,
            ctx.profile,
            &daemon_config(ctx.repo_root),
            &seed,
            ctx.timeout,
        )?;
        ctx.timings.mark("daemon");

        for case in ROUTES {
            let result = Self::open_route(driver, ctx, &app_binary, case);
            ctx.timings.mark(case.name);
            result?;
        }
        Ok(())
    }
}

impl AppRoutes {
    fn open_route(
        driver: &mut dyn Driver,
        ctx: &mut Context<'_>,
        app_binary: &std::path::Path,
        case: &RouteCase,
    ) -> Result<(), String> {
        reset_journal(ctx);
        let app = driver
            .launch(
                &Launch {
                    program: app_binary.to_path_buf(),
                    args: Vec::new(),
                    env: app_env(ctx, case.open, case.route),
                },
                ctx.timeout,
            )
            .map_err(|e| format!("launch dettivo-app for {}: {e}", case.name))?;
        ctx.profile.track_pid("dettivo-app", app.pid);
        let launched = Instant::now();
        let result = Self::check_route(driver, ctx, &app, case, launched);
        close(driver, &app)?;
        result
    }

    fn check_route(
        driver: &mut dyn Driver,
        ctx: &mut Context<'_>,
        app: &crate::driver::App,
        case: &RouteCase,
        launched: Instant,
    ) -> Result<(), String> {
        driver
            .wait_for_label(app, case.title, ctx.timeout)
            .map_err(|e| format!("title {:?} of {}: {e}", case.title, case.name))?;
        let tree = capture(driver, app, ctx, case.name)?;
        if !tree.iter().any(|e| e.name == case.title) {
            return Err(format!(
                "{} shows no element named {:?}",
                case.name, case.title
            ));
        }
        if case.name != "home" {
            return Ok(());
        }
        if !tree.iter().any(|e| e.name == "mcp: not checked") {
            return Err(
                "Home must keep MCP hosts unobserved until the requested Agents check".into(),
            );
        }
        // Home also proves the startup budget: the first frame from the
        // app's own clock and the resident set after five seconds.
        let first = wait_journal(ctx, "first_frame", ctx.timeout)?;
        let first_frame_ms = first["ms"].as_u64().unwrap_or(u64::MAX);
        if let Some(rest) = Duration::from_secs(5).checked_sub(launched.elapsed()) {
            std::thread::sleep(rest);
        }
        let memory =
            app_support::memory_kb(app.pid).ok_or("the app's resident set is unreadable")?;
        let rss_kb = memory.rss_kb;
        let gate = timing_gate();
        let first_frame_within = first_frame_ms <= FIRST_FRAME_BUDGET_MS;
        let rss_within = rss_kb <= RSS_BUDGET_KB;
        let within = first_frame_within && memory.within_budget();
        let mut report = json!({
            "first_frame_ms": first_frame_ms,
            "application_ms": first["application_ms"],
            "qml_ms": first["qml_ms"],
            "first_frame_budget_ms": FIRST_FRAME_BUDGET_MS,
            "first_frame_within_budget": first_frame_within,
            "rss_kb_after_5s": rss_kb,
            "pss_kb_after_5s": memory.pss_kb,
            "anonymous_kb_after_5s": memory.anonymous_kb,
            "rss_budget_kb": RSS_BUDGET_KB,
            "rss_within_budget": rss_within,
            "theme": first["theme"],
            "within_budget": within,
            "asserted": gate,
        });
        report
            .as_object_mut()
            .unwrap()
            .extend(memory.evidence().as_object().unwrap().clone());
        report["pid"] = json!(app.pid);
        report["status"] = app_support::app_command(ctx, &json!({"cmd": "status"}))?;
        report["rust_profile"] = json!(super::build_profile());
        std::fs::write(
            ctx.evidence_dir.join("startup.json"),
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("startup.json".into());
        if gate && !within {
            return Err(format!(
                "startup over budget: first frame {first_frame_ms} ms (under {FIRST_FRAME_BUDGET_MS}), resident {rss_kb} KiB (limit {RSS_BUDGET_KB}; proportional {} KiB, anonymous {} KiB, limit {ANONYMOUS_BUDGET_KB})",
                memory.pss_kb, memory.anonymous_kb
            ));
        }
        Ok(())
    }
}
