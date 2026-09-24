//! Never into self (FR-I8, R3): with the QA target's app id configured as
//! one of Dettivo's own windows, `insert.perform` answers `failed` with
//! `target_is_self` and the field stays empty.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::json;

use super::insertion_matrix::FIELD;
use super::support::{InsertDaemon, SAMPLE};
use super::{Context, Scenario, binary};
use crate::driver::{Driver, Launch};

/// The scenario.
pub struct NeverIntoSelf;

impl Scenario for NeverIntoSelf {
    fn id(&self) -> &'static str {
        "never_into_self"
    }

    fn summary(&self) -> &'static str {
        "a Dettivo window as target is refused with target_is_self and nothing is typed"
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let daemon = InsertDaemon::start(ctx)?;
        let program = binary(ctx.repo_root, "dettivo-insert-target")?;
        let env: BTreeMap<String, String> = ctx.profile.env();
        let app = driver
            .launch(
                &Launch {
                    program,
                    args: Vec::new(),
                    env,
                },
                ctx.timeout,
            )
            .map_err(|e| format!("launch: {e}"))?;
        ctx.profile.track_pid("dettivo-insert-target", app.pid);
        let field = driver
            .wait_for_label(&app, FIELD, ctx.timeout)
            .map_err(|e| format!("field {FIELD:?}: {e}"))?;
        driver
            .click(&app, &field)
            .map_err(|e| format!("click: {e}"))?;
        let target = daemon.wait_focused(app.pid, ctx.timeout)?;
        ctx.timings.mark("focused");

        // The target's app id joins Dettivo's own ids through the contract,
        // so the daemon sees the QA window as one of its own.
        let app_id = target["target"]["app_id"]
            .as_str()
            .ok_or("insert.target has no app_id")?
            .to_string();
        daemon.call(
            "config.set",
            json!({
                "key": "insert.self_app_ids",
                "value": format!("dettivo, dettivo-app, dettivo-osd, dettivo-sheet, {app_id}")
            }),
        )?;
        let deadline = std::time::Instant::now() + ctx.timeout;
        while daemon.call("insert.target", json!({}))?["target"]["is_dettivo"] != json!(true) {
            if std::time::Instant::now() > deadline {
                return Err("the daemon never flagged the target as a Dettivo window".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        let result = daemon.call(
            "insert.perform",
            json!({"mode": "raw", "text": SAMPLE, "expected_target_pid": app.pid.to_string()}),
        )?;
        ctx.timings.mark("perform");
        std::fs::write(
            ctx.evidence_dir.join("result.json"),
            serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("result.json".into());
        if result["outcome"] != json!("failed") || result["reason"] != json!("target_is_self") {
            return Err(format!("expected failed/target_is_self, got {result}"));
        }
        std::thread::sleep(Duration::from_millis(300));
        let field = driver
            .find(&app, FIELD)
            .map_err(|e| format!("field after refusal: {e}"))?;
        let value = driver
            .read_value(&app, &field)
            .map_err(|e| format!("read back: {e}"))?;
        // A driver reports the accessible name for a field without text.
        if !value.trim().is_empty() && value != FIELD {
            return Err(format!(
                "{} characters were typed into the Dettivo window",
                value.chars().count()
            ));
        }
        let shot = ctx.evidence_dir.join("screenshot.png");
        if driver.screenshot(&app, &shot).is_ok() {
            ctx.evidence.push("screenshot.png".into());
        }
        driver.close(&app).map_err(|e| format!("close: {e}"))?;
        ctx.timings.mark("close");
        Ok(())
    }
}
