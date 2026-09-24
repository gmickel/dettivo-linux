//! The pill through a whole dictation (fn-12 R2): a daemon in the profile
//! dictates the speech fixture through the mock microphone, dettivo-osd
//! shows Listening, Transcribing and Inserted in that order (the labels
//! are read through the driver's accessibility tree), the pill hides
//! after `hide_after_ms`, and a second dictation without the mock
//! backend, pinned to the clipboard, shows Copied or Error with the
//! reason. The pill's frame pacing over the whole drive is collected
//! through `DETTIVO_QA_PACING` (fn-20 R4) and judged against the NFR-12
//! budget on a real display, recorded under CI.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::daemon::{DaemonHandle, local_model};
use super::{Context, Scenario, binary};
use crate::driver::{App, Driver, DriverError, Launch};
use crate::pacing::{self, Animating};

/// The pill hides this long after Inserted: long enough for the slower
/// driver to read the tree and take its screenshot, short for the drive.
const HIDE_AFTER_MS: u64 = 3000;
/// How much of the fixture is recorded before stop; long enough that the
/// engine's Transcribing moment is on screen for the driver to see.
const RECORD_FOR: Duration = Duration::from_secs(6);

/// The scenario.
pub struct OsdDictation;

impl OsdDictation {
    /// The profile's config; `pin_clipboard` pins `[insert] backend` to
    /// the clipboard so the second take's outcome does not depend on
    /// which desktop backends the machine has.
    fn config(repo_root: &std::path::Path, pin_clipboard: bool) -> String {
        let bin_dir = binary(repo_root, "dettivod")
            .ok()
            .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_default();
        let insert = if pin_clipboard {
            "[insert]\nbackend = \"clipboard\"\n"
        } else {
            ""
        };
        format!(
            "[engines]\ndirectory = \"{}\"\n[speech]\nmodel = \"tiny.en\"\n[dictation]\nlanguage = \"en\"\n{insert}[osd]\nhost = \"window\"\nhide_after_ms = {HIDE_AFTER_MS}\nerror_hide_after_ms = 6000\n",
            bin_dir.display()
        )
    }

    fn snapshot(
        driver: &mut dyn Driver,
        app: &App,
        ctx: &mut Context<'_>,
        name: &str,
    ) -> Result<(), String> {
        let tree = driver
            .snapshot(app)
            .map_err(|e| format!("snapshot {name}: {e}"))?;
        let file = format!("tree-{name}.json");
        std::fs::write(
            ctx.evidence_dir.join(&file),
            serde_json::to_string_pretty(&tree).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push(file);
        let shot = format!("screenshot-{name}.png");
        driver
            .screenshot(app, &ctx.evidence_dir.join(&shot))
            .map_err(|e| format!("screenshot {name}: {e}"))?;
        ctx.evidence.push(shot);
        Ok(())
    }

    /// One line to the pill's control socket beside the daemon socket.
    fn osd_status(ctx: &Context<'_>) -> Result<Value, String> {
        Self::osd_status_at(&ctx.profile.socket().with_file_name("osd.sock"))
    }

    fn osd_status_at(socket: &std::path::Path) -> Result<Value, String> {
        let mut stream = UnixStream::connect(socket).map_err(|e| format!("osd.sock: {e}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|e| e.to_string())?;
        stream
            .write_all(b"{\"cmd\":\"status\"}\n")
            .map_err(|e| e.to_string())?;
        let mut line = String::new();
        BufReader::new(stream)
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;
        serde_json::from_str(line.trim_end()).map_err(|e| e.to_string())
    }

    fn wait_hidden(ctx: &Context<'_>, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            let status = Self::osd_status(ctx)?;
            if status["visible"] == Value::Bool(false) {
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "the pill is still visible after {timeout:?}: {status}"
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn now_unix_ms() -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0)
    }

    /// Gives the pill time to leave on SIGTERM, taking its control socket
    /// with it, before the driver's own cleanup kills it.
    fn wait_exit(pid: u32, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while std::path::Path::new(&format!("/proc/{pid}")).exists() && Instant::now() < deadline {
            let zombie = std::fs::read_to_string(format!("/proc/{pid}/status"))
                .map(|s| {
                    s.lines()
                        .any(|l| l.starts_with("State:") && l.contains('Z'))
                })
                .unwrap_or(true);
            if zombie {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Whichever of `labels` appears first, one snapshot per look so a
    /// state that holds for its beat is seen on the slower driver too.
    fn wait_for_any(
        driver: &mut dyn Driver,
        app: &App,
        labels: &[&str],
        timeout: Duration,
    ) -> Result<String, String> {
        let deadline = Instant::now() + timeout;
        loop {
            let tree = match driver.snapshot(app) {
                Ok(tree) => tree,
                Err(DriverError::NotFound(_)) => Vec::new(),
                Err(e) => return Err(format!("snapshot: {e}")),
            };
            for label in labels {
                if tree.iter().any(|e| e.name == *label) {
                    return Ok((*label).to_string());
                }
            }
            if Instant::now() > deadline {
                return Err(format!("none of {labels:?} appeared within {timeout:?}"));
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    /// Collects every distinct state the pill reports on its control socket,
    /// from now until one of `until` shows or the timeout passes, on a
    /// thread so the caller can make the request that drives the change.
    fn watch_states(
        ctx: &Context<'_>,
        until: &[&str],
        timeout: Duration,
    ) -> std::thread::JoinHandle<Result<Vec<String>, String>> {
        let socket = ctx.profile.socket().with_file_name("osd.sock");
        let until: Vec<String> = until.iter().map(|s| s.to_string()).collect();
        std::thread::spawn(move || {
            let deadline = Instant::now() + timeout;
            let mut seen: Vec<String> = Vec::new();
            loop {
                let status = Self::osd_status_at(&socket)?;
                let state = status["state"].as_str().unwrap_or("").to_string();
                if !state.is_empty() && seen.last() != Some(&state) {
                    seen.push(state.clone());
                }
                if until.contains(&state) {
                    return Ok(seen);
                }
                if Instant::now() > deadline {
                    return Err(format!(
                        "the pill never reported {until:?}; states seen: {seen:?}"
                    ));
                }
                std::thread::sleep(Duration::from_millis(15));
            }
        })
    }
}

impl Scenario for OsdDictation {
    fn id(&self) -> &'static str {
        "osd_dictation"
    }

    fn summary(&self) -> &'static str {
        "dettivo-osd shows Listening, Transcribing and Inserted through a mock-microphone dictation, then hides"
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        local_model(ctx.profile).map(|_| ()).ok_or_else(|| {
            "tiny.en or jfk.wav missing under the model directory (scripts/models/fetch-test-model.sh)".to_string()
        })?;
        binary(ctx.repo_root, "dettivo-osd").map(|_| ())
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let mut app: Option<App> = None;
        let result = Self::drive(driver, ctx, &mut app);
        // A failed step still ends the pill with SIGTERM so its control
        // socket goes with it and the leak check reports the real cause.
        if let (Err(_), Some(app)) = (&result, &app) {
            let _ = driver.close(app);
            Self::wait_exit(app.pid, Duration::from_secs(5));
        }
        result
    }
}

impl OsdDictation {
    fn drive(
        driver: &mut dyn Driver,
        ctx: &mut Context<'_>,
        launched: &mut Option<App>,
    ) -> Result<(), String> {
        let (_, wav) = local_model(ctx.profile).ok_or("model missing")?;
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let osd = binary(ctx.repo_root, "dettivo-osd")?;
        let config = Self::config(ctx.repo_root, false);
        let mock_mic: BTreeMap<String, String> = BTreeMap::from([(
            "DETTIVO_MOCK_MIC".to_string(),
            wav.to_string_lossy().into_owned(),
        )]);
        let mut daemon =
            DaemonHandle::spawn(&dettivod, ctx.profile, &config, &mock_mic, ctx.timeout)?;
        ctx.timings.mark("daemon");

        // The dictation starts first so the pill has a window to show the
        // moment it connects; the driver waits for that window.
        daemon.call("dictation.start", json!({"language": "en", "mode": "raw"}))?;
        let started = Instant::now();
        let pacing_file = ctx.evidence_dir.join("dettivo-osd.pacing.raw.json");
        let mut env = ctx.profile.env();
        env.insert(
            "DETTIVO_QA_PACING".into(),
            pacing_file.to_string_lossy().into_owned(),
        );
        let app = driver
            .launch(
                &Launch {
                    program: osd,
                    args: Vec::new(),
                    env,
                },
                ctx.timeout,
            )
            .map_err(|e| format!("launch dettivo-osd: {e}"))?;
        ctx.profile.track_pid("dettivo-osd", app.pid);
        *launched = Some(app.clone());
        driver
            .wait_for_label(&app, "Listening", ctx.timeout)
            .map_err(|e| format!("label Listening: {e}"))?;
        // The bars animate from Listening to the stop; dropped frames are
        // judged inside those stretches (wall clock, mapped to the
        // collector's clock when the summary is read).
        let mut animating_unix: Vec<(f64, f64)> = Vec::new();
        let listening_at = Self::now_unix_ms();
        ctx.timings.mark("listening");
        Self::snapshot(driver, &app, ctx, "listening")?;

        if let Some(rest) = RECORD_FOR.checked_sub(started.elapsed()) {
            std::thread::sleep(rest);
        }
        // dictation.stop answers only after the transcription and the
        // insertion, longer than the pill's Transcribing beat on a slow
        // machine, so the control socket is watched from before the stop:
        // every state the pill reports is collected until Inserted.
        let watched = Self::watch_states(ctx, &["inserted", "copied", "error"], ctx.timeout);
        animating_unix.push((listening_at, Self::now_unix_ms()));
        daemon.call("dictation.stop", json!({}))?;
        let states = watched
            .join()
            .map_err(|_| "the state watcher panicked".to_string())??;
        if !states.iter().any(|s| s == "transcribing") {
            return Err(format!(
                "the pill went to Inserted without reporting Transcribing (states seen: {states:?})"
            ));
        }
        // The state holds its beat, yet one accessibility snapshot can take
        // longer than that on a loaded runner; the control socket is the
        // proof the state showed, the driver's label is the evidence when
        // it lands in time.
        let seen = Self::wait_for_any(driver, &app, &["Transcribing", "Inserted"], ctx.timeout)?;
        if seen == "Transcribing" {
            Self::snapshot(driver, &app, ctx, "transcribing")?;
        } else {
            ctx.evidence
                .push("transcribing: reported on the control socket, the driver's snapshot arrived after the beat".into());
        }
        ctx.timings.mark("transcribing");
        driver
            .wait_for_label(&app, "Inserted", ctx.timeout)
            .map_err(|e| format!("label Inserted: {e}"))?;
        Self::snapshot(driver, &app, ctx, "inserted")?;
        ctx.timings.mark("inserted");
        Self::wait_hidden(
            ctx,
            Duration::from_millis(HIDE_AFTER_MS) + Duration::from_secs(3),
        )?;
        ctx.timings.mark("hidden");

        // A daemon without the mock backend, pinned to the clipboard: the
        // take is copied (a Wayland or X11 display) or the insertion fails
        // (no display), and the pill says which and why.
        daemon.stop();
        let mut refused = mock_mic.clone();
        refused.insert("DETTIVO_MOCK_INSERT".into(), "0".into());
        let config = Self::config(ctx.repo_root, true);
        let mut daemon =
            DaemonHandle::spawn(&dettivod, ctx.profile, &config, &refused, ctx.timeout)?;
        daemon.call("dictation.start", json!({"language": "en", "mode": "raw"}))?;
        driver
            .wait_for_label(&app, "Listening", ctx.timeout)
            .map_err(|e| format!("label Listening (second take): {e}"))?;
        let listening_at = Self::now_unix_ms();
        std::thread::sleep(Duration::from_secs(3));
        animating_unix.push((listening_at, Self::now_unix_ms()));
        let stopped = daemon.call("dictation.stop", json!({}))?;
        let item = daemon.call("transcripts.get", json!({"ref": stopped["ref"]}))?;
        let insertion = &item["facts"]["insertion"];
        let expected_outcome = match insertion["outcome"].as_str() {
            Some("copied_to_clipboard") => "Copied to clipboard",
            Some("failed") => "Not inserted",
            _ => return Err(format!("unexpected clipboard take outcome: {insertion}")),
        };
        let reason = insertion["reason"]
            .as_str()
            .filter(|reason| !reason.is_empty())
            .ok_or("the clipboard take has no recorded insertion reason")?;
        let outcome = Self::wait_for_any(
            driver,
            &app,
            &["Copied to clipboard", "Not inserted"],
            ctx.timeout,
        )?;
        let tree = driver
            .snapshot(&app)
            .map_err(|e| format!("snapshot refused: {e}"))?;
        Self::snapshot(driver, &app, ctx, "refused")?;
        let displays_reason = tree
            .iter()
            .filter(|e| e.role.contains("text") || e.role.contains("label"))
            .any(|e| e.name.contains(reason));
        if outcome != expected_outcome || !displays_reason {
            return Err(format!(
                "pill shows {outcome}; expected {expected_outcome} with recorded reason {reason}"
            ));
        }
        ctx.timings.mark("refused");

        driver.close(&app).map_err(|e| format!("close: {e}"))?;
        Self::wait_exit(app.pid, Duration::from_secs(5));
        daemon.stop();
        ctx.timings.mark("close");

        // The pill's pacing over the whole drive, recorded beside the
        // screenshots; the budget gates on a real display only.
        let gated = pacing::gate_active();
        let summary: Value = std::fs::read_to_string(&pacing_file)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(Value::Null);
        let windows: Vec<(f64, f64)> = animating_unix
            .iter()
            .map(|(s, e)| {
                (
                    pacing::collector_ms(&summary, *s),
                    pacing::collector_ms(&summary, *e),
                )
            })
            .collect();
        let (verdict, file) = pacing::collect(
            &pacing_file,
            "osd_dictation",
            ctx.evidence_dir,
            None,
            gated,
            Animating::During(&windows),
        )?;
        ctx.evidence.push(file);
        if verdict.ok {
            Ok(())
        } else {
            Err(format!("pacing: {}", verdict.reasons.join("; ")))
        }
    }
}
