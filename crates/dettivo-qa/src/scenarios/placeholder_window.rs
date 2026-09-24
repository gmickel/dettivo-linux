//! The first scenario (R1): launch the app skeleton's placeholder window in
//! an isolated profile, assert its accessible label through the driver,
//! and leave a screenshot, the accessibility snapshot and a receipt.

use std::collections::BTreeMap;

use super::{Context, Scenario, binary};
use crate::driver::{Driver, Launch};

/// The placeholder window carries the label the scenario asserts.
pub const EXPECTED_LABEL: &str = "Dettivo";

/// The scenario.
pub struct PlaceholderWindow;

impl Scenario for PlaceholderWindow {
    fn id(&self) -> &'static str {
        "placeholder_window"
    }

    fn summary(&self) -> &'static str {
        "dettivo-app shows its placeholder window with the accessible label"
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let program = binary(ctx.repo_root, "dettivo-app")?;
        let env: BTreeMap<String, String> = ctx.profile.env();
        let launch = Launch {
            program,
            args: Vec::new(),
            env,
        };
        let app = driver
            .launch(&launch, ctx.timeout)
            .map_err(|e| format!("launch: {e}"))?;
        ctx.profile.track_pid("dettivo-app", app.pid);
        ctx.timings.mark("launch");

        let element = driver
            .wait_for_label(&app, EXPECTED_LABEL, ctx.timeout)
            .map_err(|e| format!("label {EXPECTED_LABEL:?}: {e}"))?;
        ctx.timings.mark("label");
        if element.name != EXPECTED_LABEL {
            return Err(format!(
                "label is {:?}, expected {EXPECTED_LABEL:?}",
                element.name
            ));
        }

        let tree = driver
            .snapshot(&app)
            .map_err(|e| format!("snapshot: {e}"))?;
        std::fs::write(
            ctx.evidence_dir.join("tree.json"),
            serde_json::to_string_pretty(&tree).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("tree.json".into());
        let shot = ctx.evidence_dir.join("screenshot.png");
        driver
            .screenshot(&app, &shot)
            .map_err(|e| format!("screenshot: {e}"))?;
        ctx.evidence.push("screenshot.png".into());
        ctx.timings.mark("evidence");

        driver.close(&app).map_err(|e| format!("close: {e}"))?;
        ctx.timings.mark("close");
        Ok(())
    }
}
