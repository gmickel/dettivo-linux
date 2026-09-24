//! A fresh profile walks the three screens (fn-21 R1 to R4, ADR 0024): the
//! app opens on Keys with the snippet `dettivo setup hyprland --stdout`
//! prints, a press through the daemon's action path confirms live,
//! Continue writes the snippet (once, byte for byte), Models lists the
//! catalogue with the test model the fixture server offers, a pick writes
//! `[speech]` and the download lands `ready`, Try it dictates the mock
//! microphone through the real chain into the app's own field with the
//! self-target allowance, the result names the backend and the time, and
//! Done records the completion and opens Home.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::app_support::{app_env, capture, close, reset_journal};
use super::daemon::local_model;
use super::first_run_support::{
    ModelServer, golden_snippet, real_tiny_dir, stage_models, wait_first_run_state, write_config,
};
use super::support::InsertDaemon;
use super::{Context, Scenario, binary};
use crate::driver::{App, Driver, Element, Launch};

/// How much of the fixture is recorded before stop.
const RECORD_FOR: Duration = Duration::from_secs(6);

/// The scenario.
pub struct FirstRunFresh;

impl FirstRunFresh {
    fn find_prefix(
        driver: &mut dyn Driver,
        app: &App,
        prefix: &str,
        timeout: Duration,
    ) -> Result<Element, String> {
        let deadline = Instant::now() + timeout;
        loop {
            let tree = driver.snapshot(app).map_err(|e| format!("snapshot: {e}"))?;
            if let Some(e) = tree.into_iter().find(|e| e.name.starts_with(prefix)) {
                return Ok(e);
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "no element starting with {prefix:?} within {timeout:?}"
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

    fn wait_model(
        daemon: &InsertDaemon,
        model: &str,
        want: &str,
        timeout: Duration,
    ) -> Result<(Value, u64), String> {
        let deadline = Instant::now() + timeout;
        let mut seen_bytes = 0;
        loop {
            let status = daemon.call(
                "speech.models.status",
                json!({"provider": "whisper", "model": model}),
            )?;
            let row = status["models"][0].clone();
            seen_bytes = seen_bytes.max(row["bytes_done"].as_u64().unwrap_or(0));
            if row["readiness"] == want {
                return Ok((row, seen_bytes));
            }
            if Instant::now() > deadline {
                return Err(format!("{model} never became {want}; last row {row}"));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn config_value(daemon: &InsertDaemon, key: &str) -> Result<String, String> {
        let got = daemon.call("config.get", json!({"key": key}))?;
        Ok(got["entries"][0]["value"]
            .as_str()
            .unwrap_or_default()
            .to_string())
    }
}

impl Scenario for FirstRunFresh {
    fn id(&self) -> &'static str {
        "first_run_fresh"
    }

    fn summary(&self) -> &'static str {
        "a fresh profile shows Keys, Models and Try it: the golden snippet is written, a model is picked and downloaded, the mock microphone lands in the field"
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        binary(ctx.repo_root, "dettivo-app")?;
        binary(ctx.repo_root, "dettivo")?;
        local_model(ctx.profile).ok_or(
            "the local tiny.en model and jfk.wav are missing (scripts/models/fetch-test-model.sh)",
        )?;
        Ok(())
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let server = ModelServer::serve(real_tiny_dir(ctx.profile)?)?;
        let (_, catalogue) = stage_models(ctx.profile, &server.url)?;
        write_config(ctx, Some(&catalogue))?;
        self.drive(driver, ctx, &server)
    }
}

impl FirstRunFresh {
    fn drive(
        &self,
        driver: &mut dyn Driver,
        ctx: &mut Context<'_>,
        _server: &ModelServer,
    ) -> Result<(), String> {
        let app_binary = binary(ctx.repo_root, "dettivo-app")?;
        let (_, wav) = local_model(ctx.profile).ok_or("no fixture")?;
        let mut extra = BTreeMap::new();
        extra.insert("XDG_CURRENT_DESKTOP".to_string(), "Hyprland".to_string());
        extra.insert(
            "DETTIVO_MOCK_MIC".to_string(),
            wav.to_string_lossy().into_owned(),
        );
        let daemon = InsertDaemon::start_with(ctx, &extra)?;
        ctx.timings.mark("daemon");
        let golden = golden_snippet(ctx, &extra)?;
        std::fs::write(ctx.evidence_dir.join("snippet.conf"), &golden)
            .map_err(|e| e.to_string())?;
        ctx.evidence.push("snippet.conf".into());
        let mut report = serde_json::Map::new();

        // Keys: the fresh profile opens on first run with no route asked.
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
            .wait_for_label(&app, "Keys", ctx.timeout)
            .map_err(|e| format!("Keys never opened: {e}"))?;
        driver
            .wait_for_label(&app, "Bindings file", ctx.timeout)
            .map_err(|e| format!("the snippet box never appeared: {e}"))?;
        let tree = capture(driver, &app, ctx, "keys-open")?;
        // The snippet on screen is the golden: read through the tree where
        // the driver exposes the long text (the in-repo driver does), and
        // through the daemon's own answer, which the step shows verbatim.
        let first_line = golden.lines().next().unwrap_or_default();
        let shown = tree.iter().find(|e| e.name.starts_with(first_line));
        if let Some(shown) = shown {
            if shown.name != golden.trim() {
                return Err(format!(
                    "the Keys step shows a snippet that differs from the golden: {:?}",
                    shown.name
                ));
            }
        }
        let rendered = daemon.call("hotkeys.snippet", json!({}))?;
        if rendered["text"] != golden {
            return Err(format!(
                "hotkeys.snippet differs from `dettivo setup hyprland --stdout`: {rendered}"
            ));
        }
        report.insert("snippet_in_tree".into(), Value::Bool(shown.is_some()));
        let snippet_path = ctx.profile.root.join("cfg/hypr/dettivo.conf");
        if snippet_path.exists() {
            return Err("the snippet was written before Continue".into());
        }
        // The live confirmation: a press through the daemon's action path
        // (the mock microphone records) shows on the Key check panel.
        daemon.call("dictation.start", json!({"language": "en", "mode": "raw"}))?;
        let held = Self::find_prefix(driver, &app, "F9 held", ctx.timeout)?;
        report.insert("press_line".into(), Value::String(held.name.clone()));
        daemon.call("dictation.cancel", json!({}))?;
        capture(driver, &app, ctx, "keys")?;
        ctx.timings.mark("keys");
        Self::click_named(driver, &app, "Continue", ctx.timeout)?;
        driver
            .wait_for_label(&app, "Models", ctx.timeout)
            .map_err(|e| format!("Models never opened: {e}"))?;
        let written = std::fs::read_to_string(&snippet_path)
            .map_err(|e| format!("snippet not written: {e}"))?;
        if written != golden {
            return Err("the written snippet differs from the golden".into());
        }
        let setup = daemon.call(
            "hotkeys.setup",
            json!({"compositor": "hyprland", "write": true}),
        )?;
        if setup["written"] != Value::Bool(true)
            || std::fs::read_to_string(&snippet_path).map_err(|e| e.to_string())? != golden
        {
            return Err(format!("a second write changed the snippet: {setup}"));
        }
        report.insert("snippet_written".into(), Value::Bool(true));
        report.insert(
            "snippet_path".into(),
            Value::String(snippet_path.to_string_lossy().into_owned()),
        );

        // Models: the catalogue rows, a pick writes [speech] and downloads.
        driver
            .wait_for_label(&app, "Whisper test", ctx.timeout)
            .map_err(|e| format!("the test model row: {e}"))?;
        driver
            .wait_for_label(&app, "Whisper tiny.en", ctx.timeout)
            .map_err(|e| format!("the ready tiny.en row: {e}"))?;
        capture(driver, &app, ctx, "models")?;
        Self::click_named(driver, &app, "Whisper test", ctx.timeout)?;
        let (row, seen) = Self::wait_model(
            &daemon,
            "test",
            "ready",
            ctx.timeout.max(Duration::from_secs(60)),
        )?;
        if Self::config_value(&daemon, "speech.model")? != "test" {
            return Err("picking a row did not write [speech] model".into());
        }
        report.insert(
            "download".into(),
            json!({"model": "test", "bytes_seen_before_ready": seen, "size_bytes": row["size_bytes"], "readiness": row["readiness"]}),
        );
        driver
            .wait_for_label(&app, "Continue", ctx.timeout)
            .map_err(|e| format!("Continue never enabled after the download: {e}"))?;
        // Back to the real model for the take.
        Self::click_named(driver, &app, "Whisper tiny.en", ctx.timeout)?;
        let deadline = Instant::now() + ctx.timeout;
        while Self::config_value(&daemon, "speech.model")? != "tiny.en" {
            if Instant::now() > deadline {
                return Err("picking tiny.en did not write [speech] model".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        ctx.timings.mark("models");
        Self::click_named(driver, &app, "Continue", ctx.timeout)?;
        driver
            .wait_for_label(&app, "Try it", ctx.timeout)
            .map_err(|e| format!("Try it never opened: {e}"))?;

        // Try it: the field is focused, the take lands in it through the
        // real chain with the allowance armed.
        let field = driver
            .wait_for_label(&app, "Try it field", ctx.timeout)
            .map_err(|e| format!("the field: {e}"))?;
        driver
            .click(&app, &field)
            .map_err(|e| format!("focus the field: {e}"))?;
        daemon.wait_focused(app.pid, ctx.timeout)?;
        capture(driver, &app, ctx, "try")?;
        daemon.call("dictation.start", json!({"language": "en", "mode": "raw"}))?;
        std::thread::sleep(RECORD_FOR);
        let stopped = daemon.call("dictation.stop", json!({}))?;
        let outcome = stopped["insertion"]["outcome"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let backend = stopped["insertion"]["backend"]["name"]
            .as_str()
            .unwrap_or("")
            .to_string();
        report.insert("insertion".into(), stopped["insertion"].clone());
        if outcome != "inserted" {
            return Err(format!(
                "the take was not inserted into the field: {}",
                stopped["insertion"]
            ));
        }
        let deadline = Instant::now() + ctx.timeout;
        let text = loop {
            let text = driver
                .read_value(&app, &field)
                .map_err(|e| format!("read the field: {e}"))?;
            if text.to_lowercase().contains("fellow") {
                break text;
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "the field never carried the fixture words; it holds {text:?}"
                ));
            }
            std::thread::sleep(Duration::from_millis(200));
        };
        report.insert("field_text".into(), Value::String(text));
        let via = Self::find_prefix(driver, &app, "inserted via: ", ctx.timeout)?;
        let stop_to_insert = Self::find_prefix(driver, &app, "stop to insert: ", ctx.timeout)?;
        // The result line reads the completion event: the backend of the
        // take's own insertion.
        if !via.name.contains(&backend.replace('_', " ")) {
            return Err(format!(
                "the result names {:?}, the daemon inserted through {backend}",
                via.name
            ));
        }
        if !stop_to_insert.name.ends_with(" s") {
            return Err(format!(
                "the result has no stop-to-insert time: {:?}",
                stop_to_insert.name
            ));
        }
        report.insert("result_via".into(), Value::String(via.name));
        report.insert(
            "result_stop_to_insert".into(),
            Value::String(stop_to_insert.name),
        );
        capture(driver, &app, ctx, "try-inserted")?;
        ctx.timings.mark("try");

        // Done: the completion is recorded and Home opens.
        Self::click_named(driver, &app, "Done", ctx.timeout)?;
        driver
            .wait_for_label(&app, "Home", ctx.timeout)
            .map_err(|e| format!("Home never opened after Done: {e}"))?;
        // The allowance ended with the step: an insertion into the app's
        // window is refused as `target_is_self` even when the caller asks
        // for it, and nothing lands anywhere.
        daemon.wait_focused(app.pid, ctx.timeout)?;
        let allowance = daemon.call(
            "insert.perform",
            json!({"mode": "raw", "text": "after done", "allow_self_target": true}),
        )?;
        report.insert("allowance_after_done".into(), allowance.clone());
        if allowance["outcome"] != json!("failed") || allowance["reason"] != json!("target_is_self")
        {
            return Err(format!(
                "after Done the app must refuse an insertion into itself, the daemon answered {allowance}"
            ));
        }
        if allowance["backend"] != Value::Null {
            return Err(format!(
                "after Done no backend may run for the app's window: {allowance}"
            ));
        }
        close(driver, &app)?;
        let state = wait_first_run_state(ctx.profile, ctx.timeout, true)?;
        report.insert(
            "state".into(),
            Value::String(toml::to_string(&state).unwrap_or_default()),
        );
        std::fs::write(
            ctx.evidence_dir.join("first-run.json"),
            serde_json::to_string_pretty(&Value::Object(report)).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("first-run.json".into());
        Ok(())
    }
}
