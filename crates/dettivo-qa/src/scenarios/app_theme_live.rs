//! The live theme (fn-17 R3): the app starts on a Black Gold theme
//! published the way Omarchy does (`current/theme` is a symlink into the
//! themes directory), the symlink is swapped to a Tokyo Night theme the
//! way `omarchy theme set` swaps it, and the app's own journal reports the
//! palette applied on the next frame with the time it took; screenshots
//! before and after are the evidence. With `DETTIVO_TIMING_GATE=1` the
//! 100 ms budget is asserted.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use serde_json::json;

use super::app_support::{
    THEME_APPLY_BUDGET_MS, app_env, capture, close, reset_journal, timing_gate, wait_journal,
};
use super::{Context, Scenario, binary};
use crate::driver::{Driver, Launch};

const BLACK_GOLD_COLORS: &str = "accent = \"#F5BF03\"\ncursor = \"#a3850e\"\nforeground = \"#ebdbb2\"\nbackground = \"#0D0D0D\"\nselection_foreground = \"#0D0D0D\"\nselection_background = \"#ebdbb2\"\ncolor0 = \"#0D0D0D\"\ncolor1 = \"#D35F5F\"\ncolor2 = \"#a3850e\"\ncolor3 = \"#4D574E\"\ncolor4 = \"#6E6A58\"\ncolor5 = \"#BFA75D\"\ncolor6 = \"#7A6A2C\"\ncolor7 = \"#F6F1DD\"\ncolor8 = \"#303531\"\n";
const BLACK_GOLD_SHELL: &str = "[bar]\nbackground = \"#120F02\"\nactive = \"#F5BF03\"\n[font]\nbase-size = 12\nheading = 16\n[spacing]\nxs = 2\n[controls]\nnormal-color = \"#EBDBB2\"\nfocus-color = \"#F5BF03\"\nselected-color = \"#A3850E\"\n";
const TOKYO_NIGHT_COLORS: &str = "accent = \"#7aa2f7\"\ncursor = \"#c0caf5\"\nforeground = \"#c0caf5\"\nbackground = \"#1a1b26\"\ncolor0 = \"#1a1b26\"\ncolor1 = \"#f7768e\"\ncolor2 = \"#9ece6a\"\ncolor3 = \"#e0af68\"\ncolor4 = \"#7aa2f7\"\ncolor5 = \"#bb9af7\"\ncolor6 = \"#7dcfff\"\ncolor7 = \"#a9b1d6\"\ncolor8 = \"#414868\"\n";
const TOKYO_NIGHT_SHELL: &str = "[bar]\nbackground = \"#1a1b26\"\nactive = \"#7aa2f7\"\n[font]\nbase-size = 12\nheading = 16\n[spacing]\nxs = 2\n[controls]\nnormal-color = \"#c0caf5\"\nfocus-color = \"#7aa2f7\"\nselected-color = \"#7aa2f7\"\n";

/// The scenario.
pub struct AppThemeLive;

impl AppThemeLive {
    fn write_theme(dir: &Path, colors: &str, shell: &str) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        std::fs::write(dir.join("colors.toml"), colors).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("shell.toml"), shell).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// `current/theme -> ../themes/<name>`, replaced atomically through a
    /// rename, the way `omarchy theme set` publishes a theme.
    fn publish(current: &Path, name: &str) -> Result<(), String> {
        let link = current.join("theme");
        let staged = current.join("theme.new");
        let _ = std::fs::remove_file(&staged);
        std::os::unix::fs::symlink(format!("../themes/{name}"), &staged)
            .map_err(|e| e.to_string())?;
        std::fs::rename(&staged, &link).map_err(|e| e.to_string())
    }
}

impl Scenario for AppThemeLive {
    fn id(&self) -> &'static str {
        "app_theme_live"
    }

    fn summary(&self) -> &'static str {
        "dettivo-app re-skins on an Omarchy theme swap with no restart and reports the apply time"
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        binary(ctx.repo_root, "dettivo-app").map(|_| ())
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let app_binary = binary(ctx.repo_root, "dettivo-app")?;
        let omarchy = ctx.profile.root.join("state/omarchy");
        Self::write_theme(
            &omarchy.join("themes/black-gold"),
            BLACK_GOLD_COLORS,
            BLACK_GOLD_SHELL,
        )?;
        Self::write_theme(
            &omarchy.join("themes/tokyo-night"),
            TOKYO_NIGHT_COLORS,
            TOKYO_NIGHT_SHELL,
        )?;
        let current = omarchy.join("current");
        std::fs::create_dir_all(&current).map_err(|e| e.to_string())?;
        Self::publish(&current, "black-gold")?;

        reset_journal(ctx);
        let mut env: BTreeMap<String, String> = app_env(ctx, "home", None);
        env.insert(
            "DETTIVO_OMARCHY_THEME_DIR".into(),
            current.join("theme").to_string_lossy().into_owned(),
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
        let result = Self::drive(driver, ctx, &app, &current);
        close(driver, &app)?;
        result
    }
}

impl AppThemeLive {
    fn drive(
        driver: &mut dyn Driver,
        ctx: &mut Context<'_>,
        app: &crate::driver::App,
        current: &Path,
    ) -> Result<(), String> {
        driver
            .wait_for_label(app, "Home", ctx.timeout)
            .map_err(|e| format!("label Home: {e}"))?;
        let first = wait_journal(ctx, "first_frame", ctx.timeout)?;
        if first["theme"] != "omarchy" {
            return Err(format!(
                "the first frame did not carry the Omarchy theme: {first}"
            ));
        }
        capture(driver, app, ctx, "black-gold")?;
        ctx.timings.mark("black-gold");

        let swapped = std::time::Instant::now();
        Self::publish(current, "tokyo-night")?;
        let applied = wait_journal(ctx, "theme_applied", Duration::from_secs(5))?;
        // The wall clock from the rename to the journal line, an upper bound
        // that includes the watcher's delivery and the journal poll.
        let observed_ms = swapped.elapsed().as_millis() as u64;
        let ms = applied["ms"].as_u64().unwrap_or(u64::MAX);
        let accent = applied["accent"].as_str().unwrap_or("").to_lowercase();
        if applied["source"] != "omarchy" || accent != "#7aa2f7" {
            return Err(format!(
                "the swap did not land as the Tokyo Night palette: {applied}"
            ));
        }
        ctx.timings.mark("tokyo-night");
        capture(driver, app, ctx, "tokyo-night")?;
        let gate = timing_gate();
        let report = json!({
            "apply_ms": ms,
            "observed_ms": observed_ms,
            "budget_ms": THEME_APPLY_BUDGET_MS,
            "accent_before": "#f5bf03",
            "accent_after": accent,
            "within_budget": ms <= THEME_APPLY_BUDGET_MS,
            "asserted": gate,
        });
        std::fs::write(
            ctx.evidence_dir.join("theme.json"),
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("theme.json".into());
        if gate && ms > THEME_APPLY_BUDGET_MS {
            return Err(format!(
                "the theme applied {ms} ms after the swap, over the {THEME_APPLY_BUDGET_MS} ms budget"
            ));
        }
        Ok(())
    }
}
