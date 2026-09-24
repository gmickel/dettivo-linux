//! A live meeting through the window (fn-34 R2): on a fresh profile with
//! the disclosure pending, Start meeting shows the disclosure dialog, Not
//! now leaves the list without a meeting, Acknowledge and start records
//! the acknowledgement and opens the live screen with both meters and the
//! elapsed display; the mock tracks (the jfk clip as the other side, its
//! last phrase as the microphone) reach the transcript as segments that
//! arrive provisional then final in their source colours; Stop asks once
//! and the finalisation lands on the detail with the transcript. The
//! tracks come from `DETTIVO_MOCK_MIC` and `DETTIVO_MOCK_SYSTEM_AUDIO`
//! (the CI path); the rig's null sinks feed the same capture on a desktop.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;

use super::app_support::{app_env, capture, close, wait_status};
use super::daemon::{DaemonHandle, local_model};
use super::meeting_live::write_microphone_fixture;
use super::meetings_seeded::click_named;
use super::{Context, Scenario, binary};
use crate::driver::{App, Driver, Element, Launch};

/// The scenario.
pub struct MeetingsLiveGui;

/// What `meetings-live-gui.json` records.
#[derive(Debug, Serialize)]
struct Evidence {
    not_now_left_the_list: bool,
    provisional_seen_before_final: bool,
    sources_seen: Vec<String>,
    final_rows_on_stop: usize,
    detail_rows: usize,
    meeting_id: String,
    status: String,
}

fn config(repo_root: &Path) -> String {
    let bin_dir = binary(repo_root, "dettivod")
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    format!(
        "[engines]\ndirectory = \"{}\"\n[speech]\nmodel = \"tiny.en\"\n[dictation]\nlanguage = \"en\"\n[meetings.diarization]\nauto = false\n[meetings.analysis]\nauto = false\n",
        bin_dir.display()
    )
}

impl Scenario for MeetingsLiveGui {
    fn id(&self) -> &'static str {
        "meetings_live_gui"
    }

    fn summary(&self) -> &'static str {
        "the disclosure dialog on a fresh profile, Acknowledge and start, both meters, provisional then final segments, Stop into the detail"
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        local_model(ctx.profile).map(|_| ()).ok_or_else(|| {
            "tiny.en or jfk.wav missing under the model directory (scripts/models/fetch-test-model.sh)".to_string()
        })?;
        binary(ctx.repo_root, "dettivo-app").map(|_| ())
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let app_binary = binary(ctx.repo_root, "dettivo-app")?;
        let (_, clip) = local_model(ctx.profile).ok_or("model missing")?;
        let mic_fixture = ctx.evidence_dir.join("microphone-fixture.wav");
        write_microphone_fixture(&clip, &mic_fixture)?;
        let env: BTreeMap<String, String> = BTreeMap::from([
            (
                "DETTIVO_MOCK_MIC".to_string(),
                mic_fixture.to_string_lossy().into_owned(),
            ),
            (
                "DETTIVO_MOCK_SYSTEM_AUDIO".to_string(),
                clip.to_string_lossy().into_owned(),
            ),
            ("DETTIVO_E2E_DISCLOSURE".to_string(), "pending".to_string()),
        ]);
        let daemon = DaemonHandle::spawn(
            &dettivod,
            ctx.profile,
            &config(ctx.repo_root),
            &env,
            ctx.timeout,
        )?;
        ctx.timings.mark("daemon");
        let app = driver
            .launch(
                &Launch {
                    program: app_binary,
                    args: Vec::new(),
                    env: app_env(ctx, "home", None),
                },
                ctx.timeout,
            )
            .map_err(|e| format!("launch dettivo-app: {e}"))?;
        ctx.profile.track_pid("dettivo-app", app.pid);
        let result = Self::drive(driver, ctx, &app, &daemon);
        close(driver, &app)?;
        result
    }
}

/// The transcript rows on screen as `(provisional, name)`: a provisional
/// row carries the `provisional` mark beside its time and the finals come
/// first, so the marks count off the tail of the list.
fn transcript_rows(tree: &[Element]) -> Vec<(bool, String)> {
    let marks = tree.iter().filter(|e| e.name == "provisional").count();
    let rows: Vec<String> = tree
        .iter()
        .filter(|e| e.role == "list item")
        .map(|e| e.name.clone())
        .collect();
    let finals = rows.len().saturating_sub(marks);
    rows.into_iter()
        .enumerate()
        .map(|(i, name)| (i >= finals, name))
        .collect()
}

impl MeetingsLiveGui {
    fn drive(
        driver: &mut dyn Driver,
        ctx: &mut Context<'_>,
        app: &App,
        daemon: &DaemonHandle,
    ) -> Result<(), String> {
        wait_status(ctx, ctx.timeout, "connected Home", |state| {
            state["route"] == "home" && state["daemon_connected"] == true
        })?;
        capture(driver, app, ctx, "home-start")?;

        // Home's start action must survive navigation into Meetings and
        // open disclosure. Not now closes it without starting a recording.
        click_named(driver, app, "Start meeting", ctx.timeout)?;
        driver
            .wait_for_label(app, "Recording disclosure", ctx.timeout)
            .map_err(|e| format!("the disclosure dialog: {e}"))?;
        capture(driver, app, ctx, "disclosure")?;
        click_named(driver, app, "Not now", ctx.timeout)?;
        std::thread::sleep(Duration::from_millis(500));
        let after = driver.snapshot(app).map_err(|e| format!("snapshot: {e}"))?;
        let not_now_left_the_list = after.iter().any(|e| e.name == "Meetings")
            && !after.iter().any(|e| e.name == "Recording disclosure")
            && !after.iter().any(|e| e.name == "Live meeting");
        if !not_now_left_the_list {
            return Err("Not now did not leave the list without a meeting".into());
        }
        let status = daemon.call("system.health", json!({}))?;
        if status["recording_state"] == "meeting" {
            return Err("a meeting started after Not now".into());
        }
        ctx.timings.mark("not-now");

        // Acknowledge and start: the live screen with both meters.
        click_named(driver, app, "Start meeting", ctx.timeout)?;
        click_named(driver, app, "Acknowledge and start", ctx.timeout)?;
        driver
            .wait_for_label(app, "Live meeting", ctx.timeout)
            .map_err(|e| format!("the live screen: {e}"))?;
        for name in ["Mic meter", "System meter", "Elapsed", "Stop", "Pause"] {
            driver
                .wait_for_label(app, name, ctx.timeout)
                .map_err(|e| format!("{name}: {e}"))?;
        }
        let listed = daemon.call("meetings.list", json!({"limit": 5, "cursor": null}))?;
        let meeting_id = listed["items"]
            .as_array()
            .and_then(|items| items.first())
            .and_then(|i| i["ref"]["id"].as_str())
            .map(str::to_string)
            .ok_or_else(|| format!("no meeting in the store after the start: {listed}"))?;
        ctx.timings.mark("started");

        // Segments arrive provisional then final, per source.
        let deadline = Instant::now() + Duration::from_secs(40);
        let mut provisional_first = false;
        let mut final_seen = false;
        let mut sources: Vec<String> = Vec::new();
        loop {
            let tree = driver.snapshot(app).map_err(|e| format!("snapshot: {e}"))?;
            let rows = transcript_rows(&tree);
            if !final_seen && rows.iter().any(|(p, _)| *p) {
                provisional_first = true;
            }
            if rows.iter().any(|(p, _)| !*p) {
                final_seen = true;
            }
            for (_, name) in &rows {
                for source in ["You", "Remote"] {
                    if name.starts_with(&format!("{source}: "))
                        && !sources.iter().any(|s| s == source)
                    {
                        sources.push(source.to_string());
                    }
                }
            }
            if final_seen && sources.len() == 2 {
                break;
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "the transcript never showed a final segment of both sources (final {final_seen}, sources {sources:?})"
                ));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        let live = capture(driver, app, ctx, "live")?;
        let final_rows_on_stop = transcript_rows(&live).iter().filter(|(p, _)| !*p).count();
        ctx.timings.mark("segments");

        // Stop asks once, then the detail with the transcript.
        click_named(driver, app, "Stop", ctx.timeout)?;
        driver
            .wait_for_label(app, "Stop this meeting?", ctx.timeout)
            .map_err(|e| format!("the stop confirmation: {e}"))?;
        click_named(driver, app, "Stop meeting", ctx.timeout)?;
        driver
            .wait_for_label(app, "Meeting transcript", Duration::from_secs(180))
            .map_err(|e| format!("the detail after the stop: {e}"))?;
        let deadline = Instant::now() + Duration::from_secs(30);
        let detail = loop {
            let tree = driver.snapshot(app).map_err(|e| format!("snapshot: {e}"))?;
            if tree.iter().filter(|e| e.role == "list item").count() > 0 {
                break tree;
            }
            if Instant::now() > deadline {
                return Err("the detail shows no transcript rows".into());
            }
            std::thread::sleep(Duration::from_millis(250));
        };
        let detail_rows = detail.iter().filter(|e| e.role == "list item").count();
        capture(driver, app, ctx, "detail")?;
        let status = daemon.call("meetings.status", json!({"meeting_id": meeting_id}))?;
        ctx.timings.mark("stopped");

        let evidence = Evidence {
            not_now_left_the_list,
            provisional_seen_before_final: provisional_first,
            sources_seen: sources,
            final_rows_on_stop,
            detail_rows,
            meeting_id: meeting_id.clone(),
            status: status["status"].as_str().unwrap_or("").to_string(),
        };
        std::fs::write(
            ctx.evidence_dir.join("meetings-live-gui.json"),
            serde_json::to_string_pretty(&evidence).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("meetings-live-gui.json".into());
        if evidence.status != "completed" {
            return Err(format!("the meeting did not complete: {status}"));
        }
        if !provisional_first {
            return Err("no provisional segment showed before the first final one".into());
        }
        Ok(())
    }
}
