//! The pill hand-over on Omarchy (fn-29 R4): with `[omarchy] osd =
//! "panel"` the plugin's claimer (`dettivo osd host-panel`) holds
//! `dev.dettivo.OmarchyPanel` on the session bus, `dettivo-osd` exits
//! with the notice and `dettivo osd status` reports the host absent with
//! it, so the panel's pill is the only one; with `osd = "service"` the
//! claimer exits at once without claiming and `dettivo-osd` shows the
//! pill. Runs on any desktop with a session bus and a display; no driver.

use crate::child::ChildOwner;

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::daemon::DaemonHandle;
use super::{Context, Scenario, binary};
use crate::driver::Driver;

/// The bus name the plugin's panel claims (ADR 0015, ADR 0030).
pub const PANEL_BUS_NAME: &str = "dev.dettivo.OmarchyPanel";

/// The scenario.
pub struct OmarchyOsdHost;

impl OmarchyOsdHost {
    fn config(repo_root: &std::path::Path, osd: &str) -> String {
        let bin_dir = binary(repo_root, "dettivod")
            .ok()
            .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_default();
        format!(
            "[engines]\ndirectory = \"{}\"\n[osd]\nhost = \"window\"\n[omarchy]\nosd = \"{osd}\"\n",
            bin_dir.display()
        )
    }

    /// `dettivo --json osd host-panel` in the profile: the child and its
    /// first line (the claim, or the reason it did not claim).
    fn claimer(ctx: &mut Context<'_>) -> Result<(ChildOwner, Value), String> {
        let cli = binary(ctx.repo_root, "dettivo")?;
        let mut child = crate::profile::command(&cli, &ctx.profile.env())
            .args(["--json", "osd", "host-panel"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map(ChildOwner::new)
            .map_err(|e| format!("start dettivo osd host-panel: {e}"))?;
        ctx.profile.track("dettivo osd host-panel", &child);
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let mut line = String::new();
        BufReader::new(stdout)
            .read_line(&mut line)
            .map_err(|e| format!("read the claimer: {e}"))?;
        let answer: Value = serde_json::from_str(line.trim_end()).map_err(|e| {
            let stderr = child
                .stderr
                .take()
                .map(|s| {
                    let mut text = String::new();
                    let _ = BufReader::new(s).read_line(&mut text);
                    text
                })
                .unwrap_or_default();
            format!("the claimer answered no JSON ({e}): {line}{stderr}")
        })?;
        Ok((child, answer))
    }

    fn cli_json(ctx: &Context<'_>, args: &[&str]) -> Result<(Option<i32>, Value, String), String> {
        let cli = binary(ctx.repo_root, "dettivo")?;
        let output = crate::profile::command(&cli, &ctx.profile.env())
            .arg("--json")
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("run dettivo {}: {e}", args.join(" ")))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let value = stdout
            .lines()
            .find(|l| l.starts_with('{'))
            .and_then(|l| serde_json::from_str(l).ok())
            .unwrap_or(Value::Null);
        Ok((output.status.code(), value, format!("{stdout}{stderr}")))
    }

    /// Runs dettivo-osd in the profile and waits for it to exit or to open
    /// its control socket.
    fn run_osd(ctx: &mut Context<'_>, expect_exit: bool) -> Result<(ChildOwner, String), String> {
        let osd = binary(ctx.repo_root, "dettivo-osd")?;
        let mut child = crate::profile::command(&osd, &ctx.profile.env())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map(ChildOwner::new)
            .map_err(|e| format!("start dettivo-osd: {e}"))?;
        ctx.profile.track("dettivo-osd", &child);
        let socket = ctx.profile.socket().with_file_name("osd.sock");
        let deadline = Instant::now() + ctx.timeout;
        loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                let mut text = String::new();
                if let Some(out) = child.stdout.take() {
                    for line in BufReader::new(out).lines().map_while(Result::ok) {
                        text.push_str(&line);
                        text.push('\n');
                    }
                }
                if !expect_exit {
                    return Err(format!(
                        "dettivo-osd exited {status} instead of showing the pill: {text}"
                    ));
                }
                return Ok((child, text));
            }
            if socket.exists() && !expect_exit {
                return Ok((child, String::new()));
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                return Err(if expect_exit {
                    "dettivo-osd kept running beside the panel's claim".to_string()
                } else {
                    "dettivo-osd opened no control socket".to_string()
                });
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn stop(child: &mut Child) {
        let _ = Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .output();
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = child.kill();
        let _ = child.wait();
    }

    fn write_evidence(ctx: &mut Context<'_>, name: &str, value: &Value) -> Result<(), String> {
        std::fs::write(
            ctx.evidence_dir.join(name),
            serde_json::to_string_pretty(value).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push(name.to_string());
        Ok(())
    }
}

impl Scenario for OmarchyOsdHost {
    fn id(&self) -> &'static str {
        "omarchy_osd_host"
    }

    fn summary(&self) -> &'static str {
        "the panel's bus-name claim makes dettivo-osd step aside; [omarchy] osd = \"service\" gives the pill back"
    }

    fn needs_driver(&self) -> bool {
        false
    }

    fn preflight(&self) -> Result<(), String> {
        if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none()
            && std::env::var_os("XDG_RUNTIME_DIR").is_none()
        {
            return Err("no session bus (DBUS_SESSION_BUS_ADDRESS or XDG_RUNTIME_DIR)".into());
        }
        if std::env::var_os("DISPLAY").is_none() {
            return Err("no DISPLAY for the pill's window host".into());
        }
        Ok(())
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        binary(ctx.repo_root, "dettivo-osd").map(|_| ())?;
        binary(ctx.repo_root, "dettivo").map(|_| ())
    }

    fn run(&self, _driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let config = Self::config(ctx.repo_root, "panel");
        let extra: BTreeMap<String, String> = BTreeMap::new();
        let mut daemon = DaemonHandle::spawn(&dettivod, ctx.profile, &config, &extra, ctx.timeout)?;
        ctx.timings.mark("daemon");

        // Panel mode: the claimer holds the name, the pill process leaves.
        let (mut claimer, claim) = Self::claimer(ctx)?;
        if claim["claimed"] != Value::Bool(true) {
            Self::stop(&mut claimer);
            return Err(format!(
                "the claimer did not claim {PANEL_BUS_NAME}: {claim}"
            ));
        }
        let (mut osd, notice_text) = Self::run_osd(ctx, true)?;
        let _ = osd.wait();
        if !notice_text.contains(PANEL_BUS_NAME) {
            Self::stop(&mut claimer);
            return Err(format!(
                "dettivo-osd exited without naming {PANEL_BUS_NAME}: {notice_text:?}"
            ));
        }
        let (code, status, raw) = Self::cli_json(ctx, &["osd", "status"])?;
        Self::write_evidence(ctx, "osd-status-panel.json", &status)?;
        ctx.timings.mark("panel_mode");
        if code != Some(0) || status["host"] != "disabled" {
            Self::stop(&mut claimer);
            return Err(format!(
                "dettivo osd status in panel mode: exit {code:?}, expected host disabled: {raw}"
            ));
        }
        let notice = status["notice"].as_str().unwrap_or("");
        if !notice.contains(PANEL_BUS_NAME) {
            Self::stop(&mut claimer);
            return Err(format!(
                "the status notice does not name the panel: {notice}"
            ));
        }
        let socket = ctx.profile.socket().with_file_name("osd.sock");
        if socket.exists() {
            Self::stop(&mut claimer);
            return Err("two pills: dettivo-osd kept its control socket beside the panel".into());
        }
        Self::stop(&mut claimer);

        // Service mode: the claimer exits at once, dettivo-osd shows the pill.
        daemon.call(
            "config.set",
            json!({ "key": "omarchy.osd", "value": "service" }),
        )?;
        let (mut claimer, claim) = Self::claimer(ctx)?;
        let exited = claimer.wait().map_err(|e| e.to_string())?;
        if claim["claimed"] != Value::Bool(false) || !exited.success() {
            return Err(format!(
                "the claimer should exit 0 without claiming in service mode: {claim} ({exited})"
            ));
        }
        let (mut osd, _) = Self::run_osd(ctx, false)?;
        let (code, shown, raw) = Self::cli_json(ctx, &["osd", "show", "listening"])?;
        if code != Some(0) {
            Self::stop(&mut osd);
            return Err(format!("dettivo osd show listening failed: {raw}"));
        }
        let (code, status, raw) = Self::cli_json(ctx, &["osd", "status"])?;
        Self::write_evidence(ctx, "osd-status-service.json", &status)?;
        ctx.timings.mark("service_mode");
        Self::stop(&mut osd);
        daemon.stop();
        if code != Some(0)
            || status["visible"] != Value::Bool(true)
            || status["state"] != "listening"
            || status["host"] == "disabled"
        {
            return Err(format!(
                "dettivo osd status in service mode should show one listening pill: {raw} (show answered {shown})"
            ));
        }
        Ok(())
    }
}
