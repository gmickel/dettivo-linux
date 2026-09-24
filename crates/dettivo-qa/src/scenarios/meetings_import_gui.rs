//! An audio file imported into a meeting through the window (fn-34 R3):
//! the import dialog over the seeded list, a file the daemon rejects
//! names the daemon's reason in the dialog, the jfk clip goes through the
//! fields mapped onto `transcripts.import` (the target kind, the engine,
//! the language, diarize, analyse) with the chunking sentence and the
//! estimate on screen, the job is followed to its row and the detail
//! opens with the transcript. A second import is cancelled from its row
//! and recovered there, preserving the meeting's ID, retained audio and notes.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;

use super::app_support::{app_env, capture, close};
use super::daemon::{DaemonHandle, local_model};
use super::meetings_seeded::{click_named, wait_prefix, wait_rows};
use super::{Context, Scenario, binary};
use crate::driver::{App, Driver, Launch};

#[path = "meetings_import_retry.rs"]
mod retry;

/// The scenario.
pub struct MeetingsImportGui;

/// What `meetings-import-gui.json` records.
#[derive(Debug, Serialize)]
struct Evidence {
    rejected_reason: String,
    imported_title: String,
    transcript_rows: usize,
    transcript: String,
    upload_bytes: u64,
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

impl Scenario for MeetingsImportGui {
    fn id(&self) -> &'static str {
        "meetings_import_gui"
    }

    fn summary(&self) -> &'static str {
        "the import dialog rejects invalid audio and transcribes the jfk clip; row Cancel and Recover preserve the imported meeting's ID, audio and notes"
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
        let clip = large_upload_clip(&clip, ctx.evidence_dir)?;
        let seed = BTreeMap::from([
            ("DETTIVO_E2E_SEED".to_string(), "1".to_string()),
            ("RUST_LOG".to_string(), "info".to_string()),
        ]);
        let daemon = DaemonHandle::spawn_logged(
            &dettivod,
            ctx.profile,
            &config(ctx.repo_root),
            &seed,
            ctx.timeout,
            Some(&ctx.evidence_dir.join("daemon.log")),
        )?;
        ctx.evidence.push("daemon.log".into());
        ctx.timings.mark("daemon");
        let app = driver
            .launch(
                &Launch {
                    program: app_binary,
                    args: Vec::new(),
                    env: app_env(ctx, "meetings", None),
                },
                ctx.timeout,
            )
            .map_err(|e| format!("launch dettivo-app: {e}"))?;
        ctx.profile.track_pid("dettivo-app", app.pid);
        let result = Self::drive(driver, ctx, &app, &daemon, &clip);
        close(driver, &app)?;
        result
    }
}

impl MeetingsImportGui {
    fn wait_dialog(
        driver: &mut dyn Driver,
        app: &App,
        visible: bool,
        timeout: Duration,
    ) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            let tree = driver.snapshot(app).map_err(|e| format!("snapshot: {e}"))?;
            let shown = tree
                .iter()
                .any(|e| e.name == "Import dialog" && e.bounds.is_some());
            if shown == visible {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(format!("Import dialog did not reach visible={visible}"));
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    /// Waits until the list shows the five rows or the detail is open.
    fn wait_row_or_detail(
        driver: &mut dyn Driver,
        app: &App,
        timeout: Duration,
    ) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            let tree = driver.snapshot(app).map_err(|e| format!("snapshot: {e}"))?;
            let rows = tree.iter().filter(|e| e.role == "list item").count();
            if rows == 5 || tree.iter().any(|e| e.name == "Meeting transcript") {
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "the list shows {rows} rows after {timeout:?}, expected 5, and no detail opened"
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn drive(
        driver: &mut dyn Driver,
        ctx: &mut Context<'_>,
        app: &App,
        daemon: &DaemonHandle,
        clip: &Path,
    ) -> Result<(), String> {
        driver
            .wait_for_label(app, "Meetings", ctx.timeout)
            .map_err(|e| format!("label Meetings: {e}"))?;
        wait_rows(driver, app, 4, ctx.timeout)?;

        // A file the daemon rejects: a WAV header over noise.
        let bad = ctx.evidence_dir.join("not-audio.wav");
        std::fs::write(&bad, b"RIFF\x24\x00\x00\x00WAVEjunkjunkjunkjunkjunkjunk")
            .map_err(|e| e.to_string())?;
        click_named(driver, app, "Import audio", ctx.timeout)?;
        Self::wait_dialog(driver, app, true, ctx.timeout)?;
        let dialog = capture(driver, app, ctx, "import-dialog")?;
        for name in [
            "Import audio",
            "Import engine",
            "Import language",
            "diarize after transcription",
            "summary, decisions, action items",
            "Import estimate",
        ] {
            if !dialog.iter().any(|e| e.name == name) {
                return Err(format!("the dialog shows no element named {name:?}"));
            }
        }
        if dialog.iter().any(|e| e.name == "Target kind") {
            return Err("the meeting import still offers an unsupported target choice".into());
        }
        click_named(driver, app, "Import file", ctx.timeout)?;
        driver
            .type_text(app, &bad.to_string_lossy())
            .map_err(|e| format!("type the bad path: {e}"))?;
        click_named(driver, app, "Import", ctx.timeout)?;
        let refused = wait_prefix(driver, app, "Import refused: ", ctx.timeout)?;
        let rejected_reason = refused
            .name
            .trim_start_matches("Import refused: ")
            .to_string();
        if rejected_reason.is_empty() {
            return Err("the refusal names no reason".into());
        }
        ctx.timings.mark("rejected");

        // The clip: the dialog reopened with an empty field, the path
        // typed, the job followed.
        click_named(driver, app, "Cancel", ctx.timeout)?;
        Self::wait_dialog(driver, app, false, ctx.timeout)?;
        click_named(driver, app, "Import audio", ctx.timeout)?;
        Self::wait_dialog(driver, app, true, ctx.timeout)?;
        click_named(driver, app, "Import file", ctx.timeout)?;
        driver
            .type_text(app, &clip.to_string_lossy())
            .map_err(|e| format!("type the clip path: {e}"))?;
        // The probe reads the header as the path lands; the estimate line
        // carries the clip's length once it did.
        let deadline = Instant::now() + ctx.timeout;
        loop {
            let tree = driver.snapshot(app).map_err(|e| format!("snapshot: {e}"))?;
            let ready = tree.iter().any(|e| {
                e.name == "File check"
                    && e.value
                        .as_deref()
                        .is_some_and(|v| v.contains(" s") || v.contains("min"))
            });
            if ready {
                break;
            }
            if Instant::now() > deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        click_named(driver, app, "Import", ctx.timeout)?;
        ctx.timings.mark("import-sent");

        // The row appears, the job ends, the detail opens on it. A fast
        // engine finishes the clip before the list is read, so the detail
        // itself counts as the row having appeared.
        Self::wait_row_or_detail(driver, app, ctx.timeout)?;
        driver
            .wait_for_label(app, "Meeting transcript", Duration::from_secs(180))
            .map_err(|e| format!("the detail after the import: {e}"))?;
        wait_prefix(driver, app, "You: ", Duration::from_secs(30))?;
        let detail = capture(driver, app, ctx, "imported")?;
        let transcript_rows = detail.iter().filter(|e| e.role == "list item").count();
        let listed = daemon.call("meetings.list", json!({"limit": 10, "cursor": null}))?;
        let imported = listed["items"]
            .as_array()
            .and_then(|items| {
                items
                    .iter()
                    .find(|i| i["title"].as_str().is_some_and(|t| t.contains("jfk")))
            })
            .cloned()
            .ok_or_else(|| format!("no imported meeting in the store: {listed}"))?;
        let id = imported["ref"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let got = daemon.call("meetings.get", json!({"meeting_id": id}))?;
        let transcript = got["transcript"].as_str().unwrap_or("").to_string();
        if !transcript.to_lowercase().contains("country") {
            return Err(format!(
                "the imported transcript misses the clip's words: {transcript:?}"
            ));
        }
        ctx.timings.mark("imported");

        let evidence = Evidence {
            rejected_reason,
            imported_title: imported["title"].as_str().unwrap_or("").to_string(),
            transcript_rows,
            transcript,
            upload_bytes: std::fs::metadata(clip).map_err(|e| e.to_string())?.len(),
        };
        std::fs::write(
            ctx.evidence_dir.join("meetings-import-gui.json"),
            serde_json::to_string_pretty(&evidence).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("meetings-import-gui.json".into());
        retry::drive(driver, ctx, app, daemon, clip)?;
        Ok(())
    }
}

// Exercise multiple encoded IPC chunks while keeping the spoken fixture short.
fn large_upload_clip(source: &Path, dir: &Path) -> Result<PathBuf, String> {
    let mut input = std::fs::File::open(source).map_err(|e| e.to_string())?;
    let mut header = [0u8; 12];
    input.read_exact(&mut header).map_err(|e| e.to_string())?;
    if &header[..4] != b"RIFF" || &header[8..] != b"WAVE" {
        return Err("the upload fixture is not a RIFF WAV".into());
    }
    const PADDING: u32 = 4 * 1024 * 1024;
    let size = u32::from_le_bytes(header[4..8].try_into().unwrap())
        .checked_add(PADDING + 8)
        .ok_or("upload fixture length overflow")?;
    header[4..8].copy_from_slice(&size.to_le_bytes());
    let path = dir.join("jfk-large-upload.wav");
    let mut out = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    out.write_all(&header).map_err(|e| e.to_string())?;
    out.write_all(b"JUNK").map_err(|e| e.to_string())?;
    out.write_all(&PADDING.to_le_bytes())
        .map_err(|e| e.to_string())?;
    std::io::copy(&mut std::io::repeat(0).take(u64::from(PADDING)), &mut out)
        .map_err(|e| e.to_string())?;
    std::io::copy(&mut input, &mut out).map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    Ok(path)
}
