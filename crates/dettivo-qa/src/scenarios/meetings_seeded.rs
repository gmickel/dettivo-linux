//! Meetings against the seed (fn-34 R1): `dettivo-app` opens on Meetings
//! over a seeded daemon, the drive reads the week labels, the four rows
//! with their chips and swatches, opens the analysed roadmap review and
//! reads the talk-time bar, the tabs, the Raw and Polished toggle and the
//! analysis rail, exports the meeting as Markdown through the sheet into
//! the QA export directory and compares it with the store's golden,
//! renames a speaker through the popover and sees the transcript relabel,
//! then deletes the notes-only meeting after the urgent confirmation and
//! sees the list shrink. Every capture runs the negative text scan.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::json;

use super::app_support::{app_command, app_env, capture, close, daemon_config};
use super::daemon::{DaemonHandle, local_model};
use super::{Context, Scenario, binary};
use crate::driver::{App, Driver, DriverError, Element, Launch};

/// The seed's analysed meeting (`dettivo_storage::seed_meetings::RICH_MEETING_ID`).
const RICH_TITLE: &str = "Roadmap review";
/// The seed's notes-only meeting, the one the drive deletes.
const NOTES_ONLY_ID: &str = "5eed0000-0000-4000-8000-00000000a003";
const NOTES_ONLY_TITLE: &str = "Vendor intro";
/// The store's Markdown golden for the roadmap review.
const GOLDEN: &str = "crates/dettivo-storage/tests/goldens/meeting.md";

/// The scenario.
pub struct MeetingsSeeded;

impl Scenario for MeetingsSeeded {
    fn id(&self) -> &'static str {
        "meetings_seeded"
    }

    fn summary(&self) -> &'static str {
        "Meetings lists the seed by week with chips and swatches, shows the detail with its bar, tabs and rail, exports, renames a speaker and deletes"
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
        let daemon = DaemonHandle::spawn(
            &dettivod,
            ctx.profile,
            &daemon_config(ctx.repo_root),
            &seed,
            ctx.timeout,
        )?;
        ctx.timings.mark("daemon");
        let export_dir = ctx.evidence_dir.join("exports");
        std::fs::create_dir_all(&export_dir).map_err(|e| e.to_string())?;
        let mut env = app_env(ctx, "meetings", None);
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

/// The rows of the list on screen.
pub fn rows(tree: &[Element]) -> usize {
    tree.iter().filter(|e| e.role == "list item").count()
}

/// Waits until the list shows `expected` rows.
pub fn wait_rows(
    driver: &mut dyn Driver,
    app: &App,
    expected: usize,
    timeout: Duration,
) -> Result<Vec<Element>, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let tree = match driver.snapshot(app) {
            Ok(tree) => tree,
            Err(DriverError::NotFound(_)) => Vec::new(),
            Err(e) => return Err(format!("snapshot: {e}")),
        };
        let last = rows(&tree);
        if last == expected {
            return Ok(tree);
        }
        if Instant::now() > deadline {
            return Err(format!(
                "the list shows {last} rows after {timeout:?}, expected {expected}"
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Clicks the element named `name` once it is on screen.
pub fn click_named(
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

/// Waits until an element whose name starts with `prefix` is on screen.
pub fn wait_prefix(
    driver: &mut dyn Driver,
    app: &App,
    prefix: &str,
    timeout: Duration,
) -> Result<Element, String> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(tree) = driver.snapshot(app) {
            if let Some(found) = tree.into_iter().find(|e| e.name.starts_with(prefix)) {
                return Ok(found);
            }
        }
        if Instant::now() > deadline {
            return Err(format!("no element named {prefix:?}… within {timeout:?}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

impl MeetingsSeeded {
    fn drive(
        driver: &mut dyn Driver,
        ctx: &mut Context<'_>,
        app: &App,
        daemon: &DaemonHandle,
        export_dir: &Path,
    ) -> Result<(), String> {
        driver
            .wait_for_label(app, "Meetings", ctx.timeout)
            .map_err(|e| format!("label Meetings: {e}"))?;
        let tree = wait_rows(driver, app, 4, ctx.timeout)?;
        for label in ["Week of 9 Feb", "Week of 2 Feb"] {
            if !tree.iter().any(|e| e.name == label) {
                return Err(format!("the list shows no week label {label:?}"));
            }
        }
        for chip in ["Analysed", "Partial", "Notes only", "Transcript"] {
            if !tree.iter().any(|e| e.name == chip) {
                return Err(format!("the list shows no chip {chip:?}"));
            }
        }
        if !tree.iter().any(|e| e.name == "Gordon, Mara") {
            return Err("the roadmap review row shows no speaker swatches".into());
        }
        capture(driver, app, ctx, "list")?;
        ctx.timings.mark("list");

        // The analysed meeting: the bar, the tabs, the toggle, the rail.
        click_named(driver, app, RICH_TITLE, ctx.timeout)?;
        driver
            .wait_for_label(app, "Rename Gordon", ctx.timeout)
            .map_err(|e| format!("the talk-time bar: {e}"))?;
        let detail = capture(driver, app, ctx, "detail")?;
        for name in [
            "Rename Mara",
            "Meeting tabs",
            "Transcript",
            "Notes",
            "Analysis",
            "Polished",
            "Raw",
            "Analysis rail",
            "Summary",
            "Decisions",
            "Action items",
            "exports: md · txt · srt · vtt · json",
            "Delete meeting",
        ] {
            if !detail.iter().any(|e| e.name == name) {
                return Err(format!("the detail shows no element named {name:?}"));
            }
        }
        if !detail
            .iter()
            .any(|e| e.role == "list item" && e.name.starts_with("Mara: "))
        {
            return Err("the transcript shows no row spoken by Mara".into());
        }
        ctx.timings.mark("detail");

        // Export as Markdown into the QA directory; the golden is the store's.
        click_named(driver, app, "Export", ctx.timeout)?;
        click_named(driver, app, "Write file", ctx.timeout)?;
        let written = export_dir.join("meeting.md");
        let deadline = Instant::now() + ctx.timeout;
        while !written.is_file() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
        let exported = std::fs::read_to_string(&written)
            .map_err(|e| format!("the export never landed at {}: {e}", written.display()))?;
        let golden_path = ctx.repo_root.join(GOLDEN);
        let golden = std::fs::read_to_string(&golden_path)
            .map_err(|e| format!("{}: {e}", golden_path.display()))?;
        if exported != golden {
            return Err(format!(
                "the exported Markdown differs from {}",
                golden_path.display()
            ));
        }
        capture(driver, app, ctx, "export")?;
        click_named(driver, app, "Close", ctx.timeout)?;
        let deadline = Instant::now() + ctx.timeout;
        loop {
            match driver.snapshot(app) {
                Ok(tree) if !tree.iter().any(|e| e.name == "Meeting export sheet") => break,
                Ok(_) | Err(DriverError::NotFound(_)) => {}
                Err(e) => return Err(format!("the export sheet dismissal: {e}")),
            }
            if Instant::now() >= deadline {
                return Err("the export sheet did not dismiss before the rename".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        ctx.evidence.push("exports/meeting.md".into());
        ctx.timings.mark("export");

        // Rename Mara through the popover: the bar and the transcript relabel.
        click_named(driver, app, "Rename Mara", ctx.timeout)?;
        driver
            .wait_for_label(app, "Rename speaker", ctx.timeout)
            .map_err(|e| format!("the rename popover: {e}"))?;
        driver
            .type_text(app, "Marta\n")
            .map_err(|e| format!("type the name: {e}"))?;
        driver
            .wait_for_label(app, "Rename Marta", ctx.timeout)
            .map_err(|e| format!("the renamed speaker: {e}"))?;
        wait_prefix(driver, app, "Marta: ", ctx.timeout)?;
        let got = daemon.call(
            "meetings.get",
            json!({"meeting_id": "5eed0000-0000-4000-8000-00000000a001"}),
        )?;
        let renamed = got["speakers"]
            .as_array()
            .map(|s| s.iter().any(|x| x["name"] == "Marta"))
            .unwrap_or(false);
        if !renamed {
            return Err(format!("the store does not carry the new name: {got}"));
        }
        capture(driver, app, ctx, "rename")?;
        ctx.timings.mark("rename");

        // Delete the notes-only meeting after the confirmation.
        app_command(
            ctx,
            &json!({"cmd": "open", "route": "meetings.detail", "arg": NOTES_ONLY_ID}),
        )?;
        driver
            .wait_for_label(app, NOTES_ONLY_TITLE, ctx.timeout)
            .map_err(|e| format!("the notes-only meeting: {e}"))?;
        click_named(driver, app, "Delete meeting", ctx.timeout)?;
        driver
            .wait_for_label(app, "Delete this meeting?", ctx.timeout)
            .map_err(|e| format!("the confirmation: {e}"))?;
        click_named(driver, app, "Delete meeting for good", ctx.timeout)?;
        wait_rows(driver, app, 3, ctx.timeout)?;
        let listed = daemon.call("meetings.list", json!({"limit": 50, "cursor": null}))?;
        let titles: Vec<&str> = listed["items"]
            .as_array()
            .map(|items| items.iter().filter_map(|i| i["title"].as_str()).collect())
            .unwrap_or_default();
        if titles.contains(&NOTES_ONLY_TITLE) {
            return Err("the deleted meeting is still in the store".into());
        }
        capture(driver, app, ctx, "deleted")?;
        ctx.timings.mark("delete");
        Ok(())
    }
}
