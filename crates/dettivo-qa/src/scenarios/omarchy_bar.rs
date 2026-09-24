//! The bar widget on a real Omarchy desktop (fn-29 R3): a dictation
//! started through the CLI turns the glyph to listening with the active
//! underline, the daemon's state agrees at every step, and the bar's
//! right section is captured with `grim` before, during and after the
//! take for the evidence and the pixel check. Quickshell exposes no
//! accessibility tree, so the CLI is the state oracle and the crops are
//! the pixels. The drive runs against the live shell and the live daemon
//! (the plugin talks to the session's socket, not a profile's), so it
//! runs only where the shell has the plugin enabled and the operator has
//! set `DETTIVO_QA_OMARCHY_LIVE=1`; anywhere else it reports why it
//! skipped.

use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::support::installed;
use super::{Context, Scenario, binary};
use crate::driver::Driver;
use crate::visual::render::{compare, diff_tool};

/// The plugin id the shell knows.
pub const PLUGIN_ID: &str = "gmickel.dettivo";
/// The switch that allows a drive against the live daemon.
pub const LIVE_VAR: &str = "DETTIVO_QA_OMARCHY_LIVE";
/// How long a take records before the stop.
const RECORD_FOR: Duration = Duration::from_secs(3);
/// The score at which two crops of the bar are the same picture to the
/// comparator: the listening crop must score under it against idle, and
/// the crop after the take must reach it again.
const IDENTITY: f64 = 0.999;

/// The judgement over the two scores: the bar changed while listening and
/// is the idle picture again after the take. `after` is judged against
/// idle on its own, so a widget stuck in its listening look (after scoring
/// what listening scored) fails, as does any crop that never came back.
fn judge(message: &str, changed: f64, restored: f64) -> Result<(), String> {
    if changed >= IDENTITY {
        return Err(format!(
            "the CLI reported a running dictation ({message}) but the bar's right section did not change (score {changed:.3})"
        ));
    }
    if restored < IDENTITY {
        return Err(format!(
            "the bar did not return to idle after the take (after {restored:.3} against idle, listening {changed:.3}, idle is {IDENTITY})"
        ));
    }
    Ok(())
}

/// The scenario.
pub struct OmarchyBar;

struct Recording {
    cli: std::path::PathBuf,
    job_id: String,
    armed: bool,
}

impl Recording {
    fn cancel(&mut self) -> Result<(), String> {
        if !self.armed {
            return Ok(());
        }
        self.armed = false;
        let params = serde_json::json!({"expected_job_id": self.job_id}).to_string();
        OmarchyBar::cli_json(&self.cli, &["call", "dictation.cancel", &params]).map(|_| ())
    }

    fn complete<T>(&mut self, result: Result<T, String>) -> Result<T, String> {
        match (result, self.cancel()) {
            (Err(error), Err(cleanup)) => {
                Err(format!("{error}; recording cleanup failed: {cleanup}"))
            }
            (Ok(_), Err(cleanup)) => Err(format!("recording cleanup failed: {cleanup}")),
            (result, Ok(())) => result,
        }
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        if let Err(error) = self.cancel() {
            eprintln!("recording cleanup failed for {}: {error}", self.job_id);
        }
    }
}

impl OmarchyBar {
    /// One `dettivo --json` call against the session's own daemon, with
    /// the repository's build of the command.
    fn live_json(ctx: &Context<'_>, args: &[&str]) -> Result<Value, String> {
        let cli = binary(ctx.repo_root, "dettivo")?;
        Self::cli_json(&cli, args)
    }

    fn cli_json(cli: &std::path::Path, args: &[&str]) -> Result<Value, String> {
        let output = Command::new(cli)
            .arg("--json")
            .args(args)
            .output()
            .map_err(|e| format!("run dettivo {}: {e}", args.join(" ")))?;
        if !output.status.success() {
            return Err(format!(
                "dettivo {} failed (exit {:?}): {}",
                args.join(" "),
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .lines()
            .find(|l| l.starts_with('{'))
            .and_then(|l| serde_json::from_str(l).ok())
            .ok_or_else(|| {
                format!(
                    "dettivo {} answered no JSON (exit {:?}): {}",
                    args.join(" "),
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr).trim()
                )
            })
    }

    /// The focused monitor's name and width from Hyprland.
    fn focused_monitor() -> Result<(String, u64), String> {
        let output = Command::new("hyprctl")
            .args(["monitors", "-j"])
            .output()
            .map_err(|e| format!("hyprctl monitors: {e}"))?;
        let monitors: Value =
            serde_json::from_slice(&output.stdout).map_err(|e| format!("hyprctl monitors: {e}"))?;
        let focused = monitors
            .as_array()
            .and_then(|rows| {
                rows.iter()
                    .find(|m| m["focused"] == Value::Bool(true))
                    .or_else(|| rows.first())
            })
            .ok_or("hyprctl reported no monitor")?;
        let width = focused["width"].as_u64().unwrap_or(0);
        let scale = focused["scale"].as_f64().unwrap_or(1.0).max(0.1);
        Ok((
            focused["name"].as_str().unwrap_or("").to_string(),
            (width as f64 / scale) as u64,
        ))
    }

    /// The right third of the bar on the focused monitor, as `grim` cuts it.
    fn capture(ctx: &mut Context<'_>, name: &str) -> Result<std::path::PathBuf, String> {
        let (monitor, width) = Self::focused_monitor()?;
        let x = width * 2 / 3;
        let file = ctx.evidence_dir.join(format!("bar-{name}.png"));
        let status = Command::new("grim")
            .args(["-o", &monitor, "-g"])
            .arg(format!("{x},0 {} 30", width - x))
            .arg(&file)
            .status()
            .map_err(|e| format!("grim: {e}"))?;
        if !status.success() {
            return Err(format!("grim exited {status} capturing the bar"));
        }
        ctx.evidence.push(format!("bar-{name}.png"));
        Ok(file)
    }

    fn dictation_state(ctx: &Context<'_>) -> Result<(bool, String), String> {
        let status = Self::live_json(ctx, &["dictation", "status"])?;
        Ok((
            status["is_active"] == Value::Bool(true),
            status["job"]["message"].as_str().unwrap_or("").to_string(),
        ))
    }

    fn wait_inactive(ctx: &Context<'_>, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            if !Self::dictation_state(ctx)?.0 {
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err("the dictation never completed after the stop".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Scenario for OmarchyBar {
    fn id(&self) -> &'static str {
        "omarchy_bar"
    }

    fn summary(&self) -> &'static str {
        "a CLI-started dictation turns the Omarchy bar widget to listening; the daemon's state and the bar's pixels agree"
    }

    fn needs_driver(&self) -> bool {
        false
    }

    fn preflight(&self) -> Result<(), String> {
        for program in ["omarchy", "omarchy-shell", "hyprctl", "grim"] {
            if !installed(program) {
                return Err(format!("{program} is not on PATH (an Omarchy desktop)"));
            }
        }
        if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
            return Err("not inside a Hyprland session".into());
        }
        let list = Command::new("omarchy")
            .args(["plugin", "list", "--json"])
            .output()
            .map_err(|e| format!("omarchy plugin list: {e}"))?;
        let rows: Value = serde_json::from_slice(&list.stdout).unwrap_or(Value::Null);
        let enabled = rows.as_array().is_some_and(|rows| {
            rows.iter()
                .any(|r| r["id"] == PLUGIN_ID && r["enabled"] == Value::Bool(true))
        });
        if !enabled {
            return Err(format!(
                "the shell has no enabled {PLUGIN_ID} plugin (run `dettivo setup omarchy`)"
            ));
        }
        if std::env::var(LIVE_VAR).ok().as_deref() != Some("1") {
            return Err(format!(
                "drives the live daemon and the live shell; set {LIVE_VAR}=1 to allow it"
            ));
        }
        Ok(())
    }

    fn run(&self, _driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let tool = diff_tool(ctx.repo_root)?;
        let (active, _) = Self::dictation_state(ctx)?;
        if active {
            return Err("a dictation is already running; the drive needs an idle daemon".into());
        }
        let idle = Self::capture(ctx, "idle")?;
        ctx.timings.mark("idle");

        let cli = binary(ctx.repo_root, "dettivo")?;
        let start = Self::cli_json(&cli, &["dictation", "start"])?;
        let mut recording = Recording {
            cli,
            job_id: start["job"]["job_id"]
                .as_str()
                .ok_or("dictation start returned no job_id; cannot safely cancel the recording")?
                .to_string(),
            armed: true,
        };
        let result = (|| {
            let started = Instant::now();
            std::thread::sleep(Duration::from_millis(400));
            let (active, message) = Self::dictation_state(ctx)?;
            if !active {
                return Err("dettivo dictation status says no session runs after the start".into());
            }
            let listening = Self::capture(ctx, "listening")?;
            ctx.timings.mark("listening");
            if let Some(rest) = RECORD_FOR.checked_sub(started.elapsed()) {
                std::thread::sleep(rest);
            }
            Self::live_json(ctx, &["dictation", "stop"])?;
            recording.armed = false;
            Self::wait_inactive(ctx, ctx.timeout)?;
            let after = Self::capture(ctx, "after")?;
            ctx.timings.mark("after");

            // The widget changed while listening (the accent mark and the
            // active underline) and changed back afterwards: the listening
            // crop scores below identity against the idle crop, the after
            // crop scores identity against idle again.
            let diff_listening = ctx.evidence_dir.join("bar-idle-vs-listening.png");
            let changed = compare(&tool, &idle, &listening, 0.55, Some(&diff_listening))?;
            ctx.evidence.push("bar-idle-vs-listening.png".into());
            let restored = compare(&tool, &idle, &after, 0.55, None)?;
            let evidence = serde_json::json!({
                "listening_message": message,
                "idle_vs_listening": changed,
                "idle_vs_after": restored,
            });
            std::fs::write(
                ctx.evidence_dir.join("bar-scores.json"),
                serde_json::to_string_pretty(&evidence).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            ctx.evidence.push("bar-scores.json".into());
            judge(&message, changed.score, restored.score)
        })();
        recording.complete(result)
    }
}

#[cfg(test)]
#[path = "omarchy_bar_cleanup_tests.rs"]
mod cleanup_tests;

#[cfg(test)]
mod tests {
    use super::judge;

    #[test]
    fn the_bar_must_change_while_listening_and_be_idle_again_after() {
        assert!(judge("listening", 0.71, 0.9995).is_ok());
        assert!(
            judge("listening", 0.9995, 0.9995).is_err(),
            "nothing changed"
        );
        // A widget frozen in its active look: after scores what listening
        // scored, which the old no-worse-than-listening rule accepted.
        let frozen = judge("listening", 0.71, 0.71).unwrap_err();
        assert!(frozen.contains("did not return to idle"), "{frozen}");
        assert!(judge("listening", 0.71, 0.72).is_err(), "still not idle");
    }
}
