//! The two designed states of History (fn-35 R4, `states-and-hint-sheet.png`):
//! an empty profile shows the state sentence `Nothing dictated yet.`
//! with its reason and the key hint and no row, and a search with no
//! hits over the seed names the query (`No match for "…".`) with the
//! reason and `0 hits`. Neither state shows a raw table, a raw path or a
//! generic placeholder; every capture runs the negative text scan.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde_json::json;

use super::app_support::{app_env, capture, close, daemon_config, reset_journal};
use super::daemon::DaemonHandle;
use super::{Context, Scenario, binary};
use crate::driver::{App, Driver, DriverError, Element, Launch};

/// A query no seed row carries.
const QUERY: &str = "zzqx";
/// Words a designed state never shows in place of the sentence.
const PLACEHOLDERS: &[&str] = &["No data", "Empty", "null", "undefined", "N/A", "Loading..."];

/// The scenario.
pub struct HistoryStates;

impl Scenario for HistoryStates {
    fn id(&self) -> &'static str {
        "history_states"
    }

    fn summary(&self) -> &'static str {
        "History shows the designed empty state in an empty profile and names the query when a search has no hits"
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        binary(ctx.repo_root, "dettivo-app")?;
        binary(ctx.repo_root, "dettivod").map(|_| ())
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let app_binary = binary(ctx.repo_root, "dettivo-app")?;
        let config = daemon_config(ctx.repo_root);

        // An empty profile: no seed, no rows, the designed state.
        let mut daemon = DaemonHandle::spawn(
            &dettivod,
            ctx.profile,
            &config,
            &BTreeMap::new(),
            ctx.timeout,
        )?;
        ctx.timings.mark("daemon-empty");
        let app = Self::launch(driver, ctx, &app_binary)?;
        let result = Self::empty_state(driver, ctx, &app);
        close(driver, &app)?;
        result?;
        daemon.stop();
        ctx.timings.mark("empty");

        // The seed, and a query nothing carries.
        let seed = BTreeMap::from([("DETTIVO_E2E_SEED".to_string(), "1".to_string())]);
        let daemon = DaemonHandle::spawn(&dettivod, ctx.profile, &config, &seed, ctx.timeout)?;
        ctx.timings.mark("daemon-seeded");
        let seeded = super::history_seed::seeded_rows(&daemon)?;
        let app = Self::launch(driver, ctx, &app_binary)?;
        let result = Self::no_hits(driver, ctx, &app, &seeded);
        close(driver, &app)?;
        result?;
        ctx.timings.mark("no-hits");
        Ok(())
    }
}

impl HistoryStates {
    fn launch(
        driver: &mut dyn Driver,
        ctx: &mut Context<'_>,
        app_binary: &std::path::Path,
    ) -> Result<App, String> {
        reset_journal(ctx);
        let app = driver
            .launch(
                &Launch {
                    program: app_binary.to_path_buf(),
                    args: Vec::new(),
                    env: app_env(ctx, "history", None),
                },
                ctx.timeout,
            )
            .map_err(|e| format!("launch dettivo-app: {e}"))?;
        ctx.profile.track_pid("dettivo-app", app.pid);
        driver
            .wait_for_label(&app, "History", ctx.timeout)
            .map_err(|e| format!("History never opened: {e}"))?;
        Ok(app)
    }

    fn rows(tree: &[Element]) -> usize {
        tree.iter().filter(|e| e.role == "list item").count()
    }

    fn wait_rows(
        driver: &mut dyn Driver,
        app: &App,
        expected: usize,
        timeout: Duration,
    ) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            let tree = match driver.snapshot(app) {
                Ok(tree) => tree,
                Err(DriverError::NotFound(_)) => Vec::new(),
                Err(e) => return Err(format!("snapshot: {e}")),
            };
            let last = Self::rows(&tree);
            if last == expected {
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "the list shows {last} rows after {timeout:?}, expected {expected}"
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Waits until an element's name starts with `prefix`.
    fn wait_prefix(
        driver: &mut dyn Driver,
        app: &App,
        ctx: &mut Context<'_>,
        prefix: &str,
    ) -> Result<(), String> {
        let deadline = Instant::now() + ctx.timeout;
        loop {
            let tree = driver.snapshot(app).map_err(|e| format!("snapshot: {e}"))?;
            if tree.iter().any(|e| e.name.starts_with(prefix)) {
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "no element named {prefix:?}... after {:?}",
                    ctx.timeout
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// The state sentence (by prefix), its reason and no generic placeholder.
    fn check_state(
        tree: &[Element],
        name: &str,
        sentence: &str,
        reason: &str,
    ) -> Result<(), String> {
        if !tree.iter().any(|e| e.name.starts_with(sentence)) {
            return Err(format!("the {name} state shows no sentence {sentence:?}"));
        }
        if !tree.iter().any(|e| e.name.starts_with(reason)) {
            return Err(format!(
                "the {name} state shows no reason starting {reason:?}"
            ));
        }
        if let Some(bad) = tree.iter().find(|e| PLACEHOLDERS.contains(&e.name.trim())) {
            return Err(format!(
                "the {name} state shows a generic placeholder {:?}",
                bad.name
            ));
        }
        // The detail's facts grid is a named table and part of the design;
        // a table without a name is a data grid shown where the sentence
        // belongs.
        if tree
            .iter()
            .any(|e| (e.role == "table" || e.role == "tree table") && e.name.trim().is_empty())
        {
            return Err(format!("the {name} state shows a raw table"));
        }
        Ok(())
    }

    fn empty_state(
        driver: &mut dyn Driver,
        ctx: &mut Context<'_>,
        app: &App,
    ) -> Result<(), String> {
        driver
            .wait_for_label(app, "Nothing dictated yet.", ctx.timeout)
            .map_err(|e| format!("the empty state: {e}"))?;
        let tree = capture(driver, app, ctx, "empty")?;
        Self::check_state(&tree, "empty", "Nothing dictated yet.", "Hold ")?;
        if Self::rows(&tree) != 0 {
            return Err(format!("an empty profile lists {} rows", Self::rows(&tree)));
        }
        Ok(())
    }

    fn no_hits(
        driver: &mut dyn Driver,
        ctx: &mut Context<'_>,
        app: &App,
        seeded: &super::history_seed::Seed,
    ) -> Result<(), String> {
        super::history_seed::wait(driver, app, ctx, seeded, None)?;
        super::history_seed::type_query(driver, app, ctx, QUERY)?;
        // The sentence quotes the query, and both drivers carry the whole
        // sentence (the cua tree parser decodes its quotes).
        let sentence = format!("No match for \"{QUERY}\".");
        if let Err(e) = Self::wait_prefix(driver, app, ctx, "No match for ") {
            capture(driver, app, ctx, "no-hits-missing")?;
            return Err(format!("the no-hits state: {e}"));
        }
        Self::wait_rows(driver, app, 0, ctx.timeout)?;
        let tree = capture(driver, app, ctx, "no-hits")?;
        Self::check_state(&tree, "no-hits", "No match for ", "Search covers ")?;
        let named = tree.iter().any(|e| e.name == sentence);
        if !named {
            let shown: Vec<&str> = tree
                .iter()
                .filter(|e| e.name.starts_with("No match for "))
                .map(|e| e.name.as_str())
                .collect();
            return Err(format!(
                "the no-hits state does not name the query: {sentence:?} expected, {shown:?} shown"
            ));
        }
        if !tree.iter().any(|e| e.name == "0 hits") {
            return Err("the hit count does not say 0 hits".into());
        }
        std::fs::write(
            ctx.evidence_dir.join("history-states.json"),
            serde_json::to_string_pretty(&json!({
                "empty": {"sentence": "Nothing dictated yet.", "rows": 0},
                "no_hits": {"query": QUERY, "sentence": sentence, "query_named": named, "rows": 0, "hits": "0 hits"},
            }))
            .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("history-states.json".into());
        Ok(())
    }
}
