//! The app and a daemon that comes and goes (fn-17 R5): the daemon is
//! stopped under a running app, the daemon-unavailable state shows, a new
//! daemon on the same socket clears it and the event stream resumes (a
//! mock-microphone dictation puts Listening on the Home sentence), a
//! second `dettivo-app --open history` raises the first window on
//! History and exits 0, and `dettivo app open meetings` routes it too.

use std::collections::BTreeMap;
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde_json::json;

use super::app_support::{app_env, capture, close, daemon_config, wait_status};
use super::daemon::{DaemonHandle, local_model};
use super::{Context, Scenario, binary};
use crate::driver::{App, Driver, DriverError, Launch};

/// The scenario.
pub struct AppDaemon;

impl Scenario for AppDaemon {
    fn id(&self) -> &'static str {
        "app_daemon"
    }

    fn summary(&self) -> &'static str {
        "dettivo-app shows the daemon-unavailable state, reconnects when dettivod restarts, and a second launch raises the first window"
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        local_model(ctx.profile).map(|_| ()).ok_or_else(|| {
            "tiny.en or jfk.wav missing under the model directory (scripts/models/fetch-test-model.sh)".to_string()
        })?;
        binary(ctx.repo_root, "dettivo-app").map(|_| ())?;
        binary(ctx.repo_root, "dettivo").map(|_| ())
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let (_, wav) = local_model(ctx.profile).ok_or("model missing")?;
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let app_binary = binary(ctx.repo_root, "dettivo-app")?;
        let mock_mic: BTreeMap<String, String> = BTreeMap::from([(
            "DETTIVO_MOCK_MIC".to_string(),
            wav.to_string_lossy().into_owned(),
        )]);
        let config = daemon_config(ctx.repo_root);
        let mut daemon =
            DaemonHandle::spawn(&dettivod, ctx.profile, &config, &mock_mic, ctx.timeout)?;
        ctx.timings.mark("daemon");

        let app = driver
            .launch(
                &Launch {
                    program: app_binary.clone(),
                    args: Vec::new(),
                    env: app_env(ctx, "home", None),
                },
                ctx.timeout,
            )
            .map_err(|e| format!("launch dettivo-app: {e}"))?;
        ctx.profile.track_pid("dettivo-app", app.pid);
        let result = Self::drive(
            driver,
            ctx,
            &app,
            &app_binary,
            &dettivod,
            &config,
            &mock_mic,
            &mut daemon,
        );
        close(driver, &app)?;
        daemon.stop();
        result
    }
}

impl AppDaemon {
    /// Waits until a label is present, or absent, in the tree.
    fn wait_label(
        driver: &mut dyn Driver,
        app: &App,
        label: &str,
        present: bool,
        timeout: Duration,
    ) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            let tree = match driver.snapshot(app) {
                Ok(tree) => tree,
                Err(DriverError::NotFound(_)) => Vec::new(),
                Err(e) => return Err(format!("snapshot: {e}")),
            };
            if tree.iter().any(|e| e.name == label) == present {
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "{label:?} is {} after {timeout:?}",
                    if present {
                        "still absent"
                    } else {
                        "still shown"
                    }
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn drive(
        driver: &mut dyn Driver,
        ctx: &mut Context<'_>,
        app: &App,
        app_binary: &std::path::Path,
        dettivod: &std::path::Path,
        config: &str,
        mock_mic: &BTreeMap<String, String>,
        daemon: &mut DaemonHandle,
    ) -> Result<(), String> {
        driver
            .wait_for_label(app, "Home", ctx.timeout)
            .map_err(|e| format!("label Home: {e}"))?;
        wait_status(ctx, ctx.timeout, "a connected daemon", |s| {
            s["daemon_connected"] == true
        })?;
        Self::wait_label(driver, app, "Daemon unavailable", false, ctx.timeout)?;
        capture(driver, app, ctx, "connected")?;
        ctx.timings.mark("connected");

        // The daemon goes away: the state shows after the grace period.
        daemon.stop();
        wait_status(ctx, ctx.timeout, "the daemon away", |s| {
            s["daemon_state"] == "away"
        })?;
        Self::wait_label(driver, app, "Daemon unavailable", true, ctx.timeout)?;
        capture(driver, app, ctx, "away")?;
        ctx.timings.mark("away");

        // A new daemon on the same socket: the client's backoff finds it
        // (half a second to eight), the state clears, the stream resumes.
        *daemon = DaemonHandle::spawn(dettivod, ctx.profile, config, mock_mic, ctx.timeout)?;
        wait_status(ctx, Duration::from_secs(12), "the daemon back", |s| {
            s["daemon_connected"] == true
        })?;
        Self::wait_label(driver, app, "Daemon unavailable", false, ctx.timeout)?;
        daemon.call("dictation.start", json!({"language": "en", "mode": "raw"}))?;
        wait_status(ctx, ctx.timeout, "a recording dictation", |s| {
            s["dictation_state"] == "recording"
        })?;
        driver
            .wait_for_label(app, "Listening.", ctx.timeout)
            .map_err(|e| format!("label Listening after the restart: {e}"))?;
        capture(driver, app, ctx, "listening")?;
        daemon.call("dictation.cancel", json!({}))?;
        wait_status(ctx, ctx.timeout, "an idle dictation", |s| {
            s["dictation_state"] == "idle"
        })?;
        ctx.timings.mark("resumed");

        // A refused start must be visible in Home, not only in stderr.
        // This profile carries tiny.en only, so the other model is absent.
        daemon.call(
            "config.set",
            json!({"key": "speech.model", "value": "large-v3-turbo"}),
        )?;
        let start = driver
            .wait_for_label(app, "Start dictation", ctx.timeout)
            .map_err(|e| format!("start action: {e}"))?;
        driver
            .click(app, &start)
            .map_err(|e| format!("start missing model: {e}"))?;
        driver
            .wait_for_label(app, "Dictation unavailable.", ctx.timeout)
            .map_err(|e| format!("start refusal was not shown: {e}"))?;
        capture(driver, app, ctx, "start-refused")?;
        let stopped = daemon.call("dictation.status", json!({}))?;
        if stopped["is_active"] != false {
            return Err("a missing-model refusal left a dictation active".into());
        }
        daemon.call(
            "config.set",
            json!({"key": "speech.model", "value": "tiny.en"}),
        )?;
        ctx.timings.mark("start-refused");

        // A second launch raises the first window and exits 0.
        let second = crate::profile::command(app_binary, &ctx.profile.env())
            .args(["--open", "history"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map_err(|e| format!("second dettivo-app: {e}"))?;
        if !second.status.success() {
            return Err(format!(
                "the second dettivo-app exited {:?} instead of 0",
                second.status.code()
            ));
        }
        let said = String::from_utf8_lossy(&second.stdout);
        if !said.contains("raised") {
            return Err(format!(
                "the second dettivo-app did not report a raise: {said}"
            ));
        }
        wait_status(ctx, ctx.timeout, "the history route", |s| {
            s["route"] == "history"
        })?;
        driver
            .wait_for_label(app, "History", ctx.timeout)
            .map_err(|e| format!("label History after the second launch: {e}"))?;
        ctx.timings.mark("second-launch");

        // `dettivo app open meetings` routes it through the same socket.
        let cli = binary(ctx.repo_root, "dettivo")?;
        let routed = crate::profile::command(cli, &ctx.profile.env())
            .args(["app", "open", "meetings"])
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("dettivo app open: {e}"))?;
        if !routed.status.success() {
            return Err(format!(
                "dettivo app open meetings exited {:?}: {}",
                routed.status.code(),
                String::from_utf8_lossy(&routed.stderr)
            ));
        }
        wait_status(ctx, ctx.timeout, "the meetings route", |s| {
            s["route"] == "meetings"
        })?;
        driver
            .wait_for_label(app, "Meetings", ctx.timeout)
            .map_err(|e| format!("label Meetings after dettivo app open: {e}"))?;
        capture(driver, app, ctx, "routed")?;
        ctx.timings.mark("cli-open");
        Ok(())
    }
}
