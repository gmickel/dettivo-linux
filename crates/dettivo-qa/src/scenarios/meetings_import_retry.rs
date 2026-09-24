use super::super::app_support::app_command;
use super::*;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

struct PausedEngine {
    pid: u32,
    daemon: u32,
    executable: PathBuf,
    paused: bool,
}

impl PausedEngine {
    fn owned(&self) -> bool {
        let proc = PathBuf::from(format!("/proc/{}", self.pid));
        let parent = std::fs::read_to_string(proc.join("status"))
            .ok()
            .and_then(|s| {
                s.lines()
                    .find_map(|line| line.strip_prefix("PPid:")?.trim().parse::<u32>().ok())
            });
        parent == Some(self.daemon)
            && std::fs::read_link(proc.join("exe")).ok().as_ref() == Some(&self.executable)
    }

    fn signal(&self, signal: &str) -> Result<(), String> {
        if !self.owned() {
            return Err(format!(
                "refusing {signal}: engine {} no longer belongs to disposable daemon {}",
                self.pid, self.daemon
            ));
        }
        let status = Command::new("kill")
            .args([signal, &self.pid.to_string()])
            .status()
            .map_err(|e| format!("{signal} owned engine: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{signal} owned engine: {status}"))
        }
    }

    fn pause(daemon: &DaemonHandle, repo: &Path) -> Result<Self, String> {
        let pid = daemon.pid().ok_or("disposable daemon is not running")?;
        let executable = binary(repo, "dettivo-engine-whisper")?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let tasks = std::fs::read_dir(format!("/proc/{pid}/task")).map_err(|e| e.to_string())?;
        for task in tasks.flatten() {
            let children =
                std::fs::read_to_string(task.path().join("children")).unwrap_or_default();
            for child in children.split_whitespace().filter_map(|s| s.parse().ok()) {
                let mut engine = Self {
                    pid: child,
                    daemon: pid,
                    executable: executable.clone(),
                    paused: false,
                };
                if engine.owned() {
                    engine.signal("-STOP")?;
                    engine.paused = true;
                    return Ok(engine);
                }
            }
        }
        Err("the disposable daemon has no owned running Whisper engine to pause".into())
    }

    fn resume(&mut self) -> Result<(), String> {
        if self.paused {
            self.signal("-CONT")?;
            self.paused = false;
        }
        Ok(())
    }
}

impl Drop for PausedEngine {
    fn drop(&mut self) {
        if let Err(error) = self.resume() {
            eprintln!("QA engine cleanup: {error}");
        }
    }
}

fn wait_state(
    daemon: &DaemonHandle,
    id: &str,
    expected: &str,
    timeout: Duration,
) -> Result<Value, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let status = daemon.call("meetings.status", json!({"meeting_id": id}))?;
        if status["status"] == expected {
            return Ok(status);
        }
        if expected == "cancelled" && status["status"] == "completed" {
            return Err(format!(
                "meeting {id} completed despite acknowledged GUI cancellation: {status}"
            ));
        }
        if Instant::now() >= deadline {
            return Err(format!("meeting {id} did not become {expected}: {status}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn click_row_action(
    driver: &mut dyn Driver,
    app: &App,
    title: &str,
    action: &str,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        let tree = driver.snapshot(app).map_err(|e| e.to_string())?;
        if let Some((_, y, _, height)) = tree
            .iter()
            .find(|e| e.role == "list item" && e.name.contains(title))
            .and_then(|e| e.bounds)
        {
            if let Some(button) = tree.iter().find(|e| {
                e.name == action
                    && e.bounds
                        .is_some_and(|(_, by, _, bh)| by >= y && by + bh <= y + height)
            }) {
                return driver
                    .click(app, button)
                    .map_err(|e| format!("{action} on {title}: {e}"));
            }
        }
        if Instant::now() >= deadline {
            return Err(format!("no {action} action on meeting row {title}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub(super) fn drive(
    driver: &mut dyn Driver,
    ctx: &mut Context<'_>,
    app: &App,
    daemon: &DaemonHandle,
    clip: &Path,
) -> Result<(), String> {
    const TITLE: &str = "retry-cancel";
    const NOTES: &str = "Keep these user notes through GUI cancellation and recovery.";
    let copy = ctx.evidence_dir.join(format!("{TITLE}.wav"));
    std::fs::copy(clip, &copy).map_err(|e| e.to_string())?;
    app_command(ctx, &json!({"cmd":"open", "route":"meetings"}))?;
    driver
        .wait_for_label(app, "Meetings", ctx.timeout)
        .map_err(|e| e.to_string())?;
    let mut engine = PausedEngine::pause(daemon, ctx.repo_root)?;
    click_named(driver, app, "Import audio", ctx.timeout)?;
    MeetingsImportGui::wait_dialog(driver, app, true, ctx.timeout)?;
    click_named(driver, app, "Import file", ctx.timeout)?;
    driver
        .type_text(app, &copy.to_string_lossy())
        .map_err(|e| e.to_string())?;
    click_named(driver, app, "Import", ctx.timeout)?;
    MeetingsImportGui::wait_dialog(driver, app, false, ctx.timeout)?;
    let deadline = Instant::now() + ctx.timeout;
    let id = loop {
        let listed = daemon.call("meetings.list", json!({"limit":20,"cursor":null}))?;
        if let Some(id) = listed["items"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| r["title"].as_str().is_some_and(|t| t.contains(TITLE)))
            .and_then(|r| r["ref"]["id"].as_str())
        {
            break id.to_string();
        }
        if Instant::now() >= deadline {
            return Err("retry import did not create its row".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    daemon.call(
        "meetings.notes.set",
        json!({"meeting_id":id,"markdown":NOTES}),
    )?;
    wait_state(daemon, &id, "transcribing", ctx.timeout)?;
    capture(driver, app, ctx, "retry-transcribing")?;
    let log = ctx.evidence_dir.join("daemon.log");
    let offset = std::fs::read_to_string(&log)
        .map_err(|e| e.to_string())?
        .len();
    click_row_action(driver, app, TITLE, "Cancel transcription", ctx.timeout)?;
    let deadline = Instant::now() + ctx.timeout;
    loop {
        let text = std::fs::read_to_string(&log).map_err(|e| e.to_string())?;
        if text
            .get(offset..)
            .is_some_and(|s| s.contains("job cancel requested"))
        {
            break;
        }
        if Instant::now() >= deadline {
            return Err("GUI Cancel never reached the daemon's job cancellation handler".into());
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    engine.resume()?;
    let cancelled = match wait_state(daemon, &id, "cancelled", Duration::from_secs(90)) {
        Ok(status) => status,
        Err(error) => {
            let _ = capture(driver, app, ctx, "retry-cancel-failed");
            return Err(error);
        }
    };
    let retained = ctx
        .profile
        .root
        .join("data/dettivo/meetings")
        .join(&id)
        .join("microphone.wav");
    if !retained.is_file() {
        return Err("cancelled import lost its retained audio".into());
    }
    let cancelled_tree = capture(driver, app, ctx, "retry-cancelled")?;
    if let Some(notice) = cancelled_tree.iter().find(|e| {
        e.name == "Meetings notice"
            && e.bounds
                .is_some_and(|(_, _, width, height)| width > 0 && height > 0)
            && e.value.as_deref().is_some_and(|s| !s.is_empty())
    }) {
        return Err(format!(
            "GUI cancellation reported a failure: {:?}",
            notice.value
        ));
    }
    click_row_action(driver, app, TITLE, "Recover", ctx.timeout)?;
    let completed = wait_state(daemon, &id, "completed", Duration::from_secs(180))?;
    let got = daemon.call("meetings.get", json!({"meeting_id":id}))?;
    if got["ref"]["id"] != id
        || got["notes"] != NOTES
        || !got["transcript"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase()
            .contains("country")
    {
        return Err(format!(
            "GUI recovery changed identity/notes or lost transcript: {got}"
        ));
    }
    capture(driver, app, ctx, "retry-completed")?;
    let evidence = json!({"meeting_id":id,"cancelled":cancelled,"completed":completed,"notes":got["notes"],"retained_audio":true,"engine_pid":engine.pid,"daemon_pid":engine.daemon});
    std::fs::write(
        ctx.evidence_dir.join("meetings-import-retry.json"),
        serde_json::to_string_pretty(&evidence).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    ctx.evidence.push("meetings-import-retry.json".into());
    ctx.timings.mark("retry-completed");
    Ok(())
}
