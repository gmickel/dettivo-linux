//! History against the seed (fn-22 R1, R2, R3): `dettivo-app` opens on
//! History over a seeded daemon that keeps the seed's take, the drive
//! reads the day labels and as many rows as the seed lists (the count is
//! read from the daemon's `transcripts.list` over the seeded profile, the
//! same request the list makes, never a literal), searches for a seeded word
//! and reads the hit count and the painted row, opens a row and reads the
//! detail's Enhanced, Raw, audio strip and facts (the insertion backend
//! and the stop-to-insert time among them), exports everything as JSON
//! through the sheet into the QA export directory and compares it with
//! the store's golden, re-runs the item with the take through the dialog
//! and waits for the linked item to settle, then deletes an item after the
//! confirmation and sees the list shrink. Every capture runs the negative
//! text scan.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::app_support::{app_env, capture, close, daemon_config};
use super::daemon::{DaemonHandle, local_model};
use super::history_seed;
use super::{Context, Scenario, binary};
use crate::driver::{App, Driver, Launch};

/// The seed row with the retained take (`dettivo_storage::seed::AUDIO_ITEM_ID`).
const AUDIO_ITEM: &str = "5eed0000-0000-4000-8000-000000000011";
/// Its title, the row the drive opens.
const AUDIO_TITLE: &str = "Summary of the design call: the bar widget";
/// A word some seed rows carry; how many is read from the seed.
const QUERY: &str = "api";
/// The row the drive deletes at the end.
/// A row on screen without scrolling once the re-run item joined the
/// list; a row below the fold has no bounds a click can reach.
const DELETE_TITLE: &str = "Thanks for the review, I pushed the fixes for";

/// The scenario.
pub struct HistorySeeded;

impl Scenario for HistorySeeded {
    fn id(&self) -> &'static str {
        "history_seeded"
    }

    fn summary(&self) -> &'static str {
        "History lists the seed by day, searches with painted hits, shows a detail with its facts, re-runs, exports and deletes"
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        local_model(ctx.profile).map(|_| ()).ok_or_else(|| {
            "tiny.en missing under the model directory (scripts/models/fetch-test-model.sh)"
                .to_string()
        })?;
        binary(ctx.repo_root, "dettivo-app").map(|_| ())
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let app_binary = binary(ctx.repo_root, "dettivo-app")?;
        let seed = BTreeMap::from([("DETTIVO_E2E_SEED".to_string(), "1".to_string())]);
        // The seed keeps its take only with the age limit off (the rows
        // are dated in February).
        let config = format!(
            "{}[history]\naudio_retention_days = 0\n",
            daemon_config(ctx.repo_root)
        );
        let daemon = DaemonHandle::spawn(&dettivod, ctx.profile, &config, &seed, ctx.timeout)?;
        ctx.timings.mark("daemon");
        let export_dir = ctx.evidence_dir.join("exports");
        std::fs::create_dir_all(&export_dir).map_err(|e| e.to_string())?;
        let mut env = app_env(ctx, "history", None);
        env.insert(
            "DETTIVO_E2E_EXPORT_DIR".into(),
            export_dir.to_string_lossy().into_owned(),
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
        let result = Self::drive(driver, ctx, &app, &daemon, &export_dir);
        close(driver, &app)?;
        result
    }
}

impl HistorySeeded {
    /// Waits until no list item carries `title`.
    fn wait_gone(
        driver: &mut dyn Driver,
        app: &App,
        ctx: &mut Context<'_>,
        title: &str,
    ) -> Result<(), String> {
        let deadline = Instant::now() + ctx.timeout;
        loop {
            let tree = driver.snapshot(app).map_err(|e| format!("snapshot: {e}"))?;
            if !tree
                .iter()
                .any(|e| e.role == "list item" && e.name == title)
            {
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "the row {title:?} is still listed after the delete"
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Waits until exactly `expected` rows are on screen (the search hits).
    fn wait_hits(
        driver: &mut dyn Driver,
        app: &App,
        ctx: &mut Context<'_>,
        expected: usize,
    ) -> Result<Vec<crate::driver::Element>, String> {
        let deadline = Instant::now() + ctx.timeout;
        loop {
            let tree = driver.snapshot(app).map_err(|e| format!("snapshot: {e}"))?;
            let shown = history_seed::rows(&tree).len();
            if shown == expected {
                return Ok(tree);
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "the list shows {shown} rows after {:?}, expected {expected}",
                    ctx.timeout
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn click_named(
        driver: &mut dyn Driver,
        app: &App,
        name: &str,
        timeout: Duration,
    ) -> Result<(), String> {
        let element = driver
            .wait_for_label(app, name, timeout)
            .map_err(|e| format!("{name:?}: {e}"))?;
        driver
            .click(app, &element)
            .map_err(|e| format!("click {name:?}: {e}"))
    }

    fn drive(
        driver: &mut dyn Driver,
        ctx: &mut Context<'_>,
        app: &App,
        daemon: &DaemonHandle,
        export_dir: &Path,
    ) -> Result<(), String> {
        driver
            .wait_for_label(app, "History", ctx.timeout)
            .map_err(|e| format!("label History: {e}"))?;
        let seeded = history_seed::seeded_rows(daemon)?;
        let tree = history_seed::wait(driver, app, ctx, &seeded, None)?;
        for label in ["Wed 11 Feb", "Thu 12 Feb", "Fri 13 Feb"] {
            if !tree.iter().any(|e| e.name == label) {
                return Err(format!("the list shows no day label {label:?}"));
            }
        }
        capture(driver, app, ctx, "list")?;
        ctx.timings.mark("list");

        // Search: the hit count and the painted rows.
        history_seed::type_query(driver, app, ctx, QUERY)?;
        let found = history_seed::seeded_hits(daemon, QUERY)?;
        let hits = Self::wait_hits(driver, app, ctx, found)?;
        let label = if found == 1 {
            "1 hit".to_string()
        } else {
            format!("{found} hits")
        };
        if !hits.iter().any(|e| e.name == label) {
            return Err(format!(
                "the hit count does not say {label:?} (the seed's transcripts.search answered {found} rows)"
            ));
        }
        if !hits
            .iter()
            .any(|e| e.role == "list item" && e.name.to_lowercase().contains(QUERY))
        {
            return Err("no row carries the searched word".into());
        }
        capture(driver, app, ctx, "search")?;
        Self::click_named(driver, app, "Clear search", ctx.timeout)?;
        history_seed::wait(driver, app, ctx, &seeded, None)?;
        ctx.timings.mark("search");

        // The detail of the row with the take.
        Self::click_named(driver, app, AUDIO_TITLE, ctx.timeout)?;
        driver
            .wait_for_label(app, "stop to insert: 0.8 s", ctx.timeout)
            .map_err(|e| format!("the stop-to-insert fact: {e}"))?;
        let detail = capture(driver, app, ctx, "detail")?;
        for name in [
            "Enhanced",
            "Raw",
            "Audio",
            "Play",
            "insertion: mock",
            "app: TextEditor",
            "engine: whisper · large-v3-turbo",
        ] {
            if !detail.iter().any(|e| e.name == name) {
                return Err(format!("the detail shows no element named {name:?}"));
            }
        }
        if !detail
            .iter()
            .any(|e| e.name.starts_with("Summary of the design call"))
        {
            return Err("the detail shows no Enhanced text".into());
        }
        ctx.timings.mark("detail");

        // Export everything as JSON through the sheet into the QA directory.
        Self::click_named(driver, app, "Export", ctx.timeout)?;
        Self::click_named(driver, app, "Everything", ctx.timeout)?;
        Self::click_named(driver, app, "Write file", ctx.timeout)?;
        let written = export_dir.join("dictations.json");
        let deadline = Instant::now() + ctx.timeout;
        while !written.is_file() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
        let exported = std::fs::read_to_string(&written)
            .map_err(|e| format!("the export never landed at {}: {e}", written.display()))?;
        Self::check_export(ctx.repo_root, &exported)?;
        capture(driver, app, ctx, "export")?;
        Self::click_named(driver, app, "Close", ctx.timeout)?;
        ctx.evidence.push("exports/dictations.json".into());
        ctx.timings.mark("export");

        // Re-run with the daemon's engine: the linked item joins the top
        // of the list, works and settles.
        let top = driver
            .snapshot(app)
            .ok()
            .and_then(|tree| history_seed::top_row(&tree))
            .ok_or("no row on top of the list before the re-run")?;
        Self::click_named(driver, app, "Re-run", ctx.timeout)?;
        Self::click_named(driver, app, "Start re-run", ctx.timeout)?;
        history_seed::wait(driver, app, ctx, &seeded, Some(&top))?;
        let linked = Self::wait_linked(daemon, Duration::from_secs(60))?;
        capture(driver, app, ctx, "rerun")?;
        ctx.timings.mark("rerun");
        let _ = linked;

        // Delete after the confirmation: the list shrinks without a reload.
        Self::click_named(driver, app, DELETE_TITLE, ctx.timeout)?;
        Self::click_named(driver, app, "Delete", ctx.timeout)?;
        driver
            .wait_for_label(app, "Delete this dictation?", ctx.timeout)
            .map_err(|e| format!("the confirmation: {e}"))?;
        Self::click_named(driver, app, "Delete for good", ctx.timeout)?;
        // The list shrinks in place: the row is gone from the tree without
        // a reload (the view keeps only the delegates on screen, so the
        // count of rows is not the measure here).
        Self::wait_gone(driver, app, ctx, DELETE_TITLE)?;
        let listed = daemon.call(
            "transcripts.list",
            json!({"kinds": ["dictation"], "limit": 50, "cursor": null}),
        )?;
        let titles: Vec<&str> = listed["items"]
            .as_array()
            .map(|items| items.iter().filter_map(|i| i["title"].as_str()).collect())
            .unwrap_or_default();
        if titles.iter().any(|t| t.starts_with(DELETE_TITLE)) {
            return Err("the deleted item is still in the store".into());
        }
        capture(driver, app, ctx, "deleted")?;
        ctx.timings.mark("delete");
        Ok(())
    }

    /// The exported JSON equals the store's golden once the profile's
    /// audio path is levelled: the seed keeps one take here.
    fn check_export(repo_root: &Path, exported: &str) -> Result<(), String> {
        let golden_path: PathBuf = repo_root.join("crates/dettivo-storage/tests/goldens/seed.json");
        let golden: Value = serde_json::from_str(
            &std::fs::read_to_string(&golden_path)
                .map_err(|e| format!("{}: {e}", golden_path.display()))?,
        )
        .map_err(|e| e.to_string())?;
        let mut live: Value =
            serde_json::from_str(exported).map_err(|e| format!("the export is not JSON: {e}"))?;
        if let Some(items) = live["items"].as_array_mut() {
            for item in items.iter_mut() {
                item["audio_path"] = Value::Null;
            }
        }
        if live != golden {
            return Err(format!(
                "the exported JSON differs from {}",
                golden_path.display()
            ));
        }
        Ok(())
    }

    /// The re-run item linked to the take's row, once it completed.
    fn wait_linked(daemon: &DaemonHandle, timeout: Duration) -> Result<Value, String> {
        let deadline = Instant::now() + timeout;
        loop {
            let listed = daemon.call(
                "transcripts.list",
                json!({"kinds": ["dictation"], "limit": 50, "cursor": null}),
            )?;
            let linked = listed["items"]
                .as_array()
                .and_then(|items| items.iter().find(|i| i["source"] == "rerun").cloned());
            if let Some(item) = linked {
                let id = item["ref"]["id"].as_str().unwrap_or_default().to_string();
                let got = daemon.call(
                    "transcripts.get",
                    json!({"ref": {"kind": "dictation", "id": id}}),
                )?;
                if got["facts"]["rerun_of"]["id"] != json!(AUDIO_ITEM) {
                    return Err(format!(
                        "the re-run item is not linked to {AUDIO_ITEM}: {got}"
                    ));
                }
                if rerun_settled(&item)? {
                    return Ok(got);
                }
            }
            if Instant::now() > deadline {
                return Err(format!("no re-run item settled within {timeout:?}"));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
}

/// Whether a listed re-run item reached `completed`: `transcribing` is
/// still worth waiting for, `failed` and any status the store does not
/// know end the wait with the item's own diagnostic.
fn rerun_settled(item: &Value) -> Result<bool, String> {
    match item["status"].as_str() {
        Some("completed") => Ok(true),
        Some("transcribing") => Ok(false),
        Some("failed") => Err(format!(
            "the re-run failed: {} ({})",
            item["error_message"].as_str().unwrap_or("no error message"),
            item["error_code"].as_str().unwrap_or("no error code")
        )),
        other => Err(format!("the re-run item has status {other:?}: {item}")),
    }
}

#[cfg(test)]
mod tests {
    use super::rerun_settled;
    use serde_json::json;

    #[test]
    fn only_a_completed_rerun_settles() {
        assert_eq!(rerun_settled(&json!({"status": "completed"})), Ok(true));
        assert_eq!(rerun_settled(&json!({"status": "transcribing"})), Ok(false));
        let failed = rerun_settled(&json!({
            "status": "failed",
            "error_code": "engine_failed",
            "error_message": "the engine exited"
        }))
        .unwrap_err();
        assert!(failed.contains("the engine exited"), "{failed}");
        assert!(rerun_settled(&json!({"status": "done"})).is_err());
        assert!(rerun_settled(&json!({})).is_err());
    }
}
