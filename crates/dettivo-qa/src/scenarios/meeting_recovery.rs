//! Kill and recover (fn-24 R3): a meeting records both tracks from the
//! mock fixtures, the daemon dies with SIGKILL mid-meeting, the next
//! daemon on the same profile promotes the meeting to `partial` with its
//! takes intact and the chunk counts from the checkpoint, `meetings.status`
//! lists it under `recoverable`, `meetings.recover` keeps it and finalises
//! it from the takes (fn-28 R3), and a second interrupted meeting is
//! removed by `meetings.discard`. Drives the socket alone, so it needs no
//! driver, no display and no PipeWire; the finalisation needs the test
//! model.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};

use super::daemon::{DaemonHandle, local_model};
use super::{Context, Scenario, binary};
use crate::driver::Driver;

/// How long each meeting records before the kill.
const RECORD_FOR: Duration = Duration::from_millis(2600);

/// The scenario.
pub struct MeetingRecovery;

/// What `recovery.json` records.
#[derive(Debug, Serialize)]
struct Evidence {
    recovered_id: String,
    partial_after_restart: bool,
    microphone_samples: u32,
    system_samples: u32,
    chunks_completed: u64,
    chunks_total: u64,
    recovered_status: String,
    discarded_id: String,
    discarded_reason: String,
    discarded_directory_gone: bool,
}

fn config(repo_root: &Path) -> String {
    let bin_dir = binary(repo_root, "dettivod")
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    format!(
        "[engines]\ndirectory = \"{}\"\n[speech]\nmodel = \"tiny.en\"\n[dictation]\nlanguage = \"en\"\n[meetings]\ncheckpoint_interval_seconds = 1\nlive = false\n",
        bin_dir.display()
    )
}

fn samples(path: &Path) -> u32 {
    hound::WavReader::open(path).map(|r| r.len()).unwrap_or(0)
}

fn start(daemon: &DaemonHandle, title: &str) -> Result<String, String> {
    let started = daemon.call(
        "meetings.start",
        json!({"capture": {"microphone": true, "system_audio": true}, "title": title, "acknowledge_meeting_disclosure": true}),
    )?;
    started["ref"]["id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("meetings.start carries no ref.id: {started}"))
}

impl Scenario for MeetingRecovery {
    fn id(&self) -> &'static str {
        "meeting_recovery"
    }

    fn summary(&self) -> &'static str {
        "a meeting killed mid-capture comes back partial with its takes; recover keeps it, discard removes it"
    }

    fn needs_driver(&self) -> bool {
        false
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        local_model(ctx.profile).map(|_| ()).ok_or_else(|| {
            "tiny.en or jfk.wav missing under the model directory (scripts/models/fetch-test-model.sh)".to_string()
        })
    }

    fn run(&self, _driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let config = config(ctx.repo_root);
        let fixture = ctx.evidence_dir.join("fixture.wav");
        crate::audio::write_fixture(&fixture).map_err(|e| format!("fixture: {e}"))?;
        let env: BTreeMap<String, String> = BTreeMap::from([
            (
                "DETTIVO_MOCK_MIC".to_string(),
                fixture.to_string_lossy().into_owned(),
            ),
            (
                "DETTIVO_MOCK_SYSTEM_AUDIO".to_string(),
                fixture.to_string_lossy().into_owned(),
            ),
        ]);
        let meetings_dir = ctx.profile.root.join("data/dettivo/meetings");
        let mut daemon = DaemonHandle::spawn(&dettivod, ctx.profile, &config, &env, ctx.timeout)?;
        let id = start(&daemon, "Interrupted")?;
        std::thread::sleep(RECORD_FOR);
        let dir = meetings_dir.join(&id);
        if !dir.join("live-checkpoint.json").is_file() {
            return Err("no live-checkpoint.json after 2.6 s at a 1 s interval".into());
        }
        daemon.kill();
        ctx.timings.mark("killed");

        let mut daemon = DaemonHandle::spawn(&dettivod, ctx.profile, &config, &env, ctx.timeout)?;
        let status = daemon.call("meetings.status", json!({"meeting_id": id}))?;
        if status["status"] != json!("partial") {
            return Err(format!(
                "meeting {id} is not partial after the restart: {status}"
            ));
        }
        let recoverable = status["recoverable"]
            .as_array()
            .map(|r| r.iter().any(|m| m["ref"]["id"] == json!(id)))
            .unwrap_or(false);
        if !recoverable {
            return Err(format!(
                "meetings.status does not list {id} as recoverable: {status}"
            ));
        }
        let microphone_samples = samples(&dir.join("microphone.wav"));
        let system_samples = samples(&dir.join("system.wav"));
        if microphone_samples < 16_000 || system_samples < 16_000 {
            return Err(format!(
                "the takes are not intact: microphone {microphone_samples} samples, system {system_samples} samples"
            ));
        }
        ctx.timings.mark("partial");
        let recovered = daemon.call("meetings.recover", json!({"meeting_id": id}))?;
        let recovered_status = recovered["status"].as_str().unwrap_or("").to_string();
        if recovered_status != "transcribing" || !dir.join("microphone.wav").is_file() {
            return Err(format!(
                "meetings.recover did not keep the meeting and finalise it: {recovered}"
            ));
        }
        // The recovery finalises from the takes: the row completes with
        // its audio still there and the partial flag gone.
        let settled = wait_settled(&daemon, &id, Duration::from_secs(120))?;
        if settled["status"] != json!("completed")
            || settled["capture"]["is_partial"] == json!(true)
        {
            return Err(format!(
                "the recovered meeting did not finalise to completed: {settled}"
            ));
        }
        if !dir.join("microphone.wav").is_file() {
            return Err("the finalisation removed the audio".into());
        }
        ctx.timings.mark("recovered");

        // A second interruption whose checkpoint cannot be read: the
        // audio is still there and the reason is named; discard removes
        // the meeting and its directory.
        let second = start(&daemon, "Discarded")?;
        std::thread::sleep(Duration::from_millis(1600));
        let second_dir = meetings_dir.join(&second);
        daemon.kill();
        std::fs::write(second_dir.join("live-checkpoint.json"), "{")
            .map_err(|e| format!("corrupt checkpoint: {e}"))?;
        let mut daemon = DaemonHandle::spawn(&dettivod, ctx.profile, &config, &env, ctx.timeout)?;
        let status = daemon.call("meetings.status", json!({"meeting_id": second}))?;
        let reason = status["capture"]["reason"]
            .as_str()
            .unwrap_or("")
            .to_string();
        if status["status"] != json!("partial") || !reason.contains("not a checkpoint") {
            return Err(format!(
                "the unreadable checkpoint was not reported: {status}"
            ));
        }
        if samples(&second_dir.join("microphone.wav")) < 8_000 {
            return Err("the audio of the second meeting was not preserved".into());
        }
        let discarded = daemon.call("meetings.discard", json!({"meeting_id": second}))?;
        if discarded["discarded"] != json!(true) || second_dir.exists() {
            return Err(format!(
                "meetings.discard did not remove the meeting: {discarded}"
            ));
        }
        ctx.timings.mark("discarded");
        daemon.stop();

        let evidence = Evidence {
            recovered_id: id,
            partial_after_restart: true,
            microphone_samples,
            system_samples,
            chunks_completed: status_u64(&status, "chunks_completed"),
            chunks_total: status_u64(&status, "chunks_total"),
            recovered_status,
            discarded_id: second,
            discarded_reason: reason,
            discarded_directory_gone: true,
        };
        std::fs::write(
            ctx.evidence_dir.join("recovery.json"),
            serde_json::to_string_pretty(&evidence).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("recovery.json".into());
        Ok(())
    }
}

/// Polls `meetings.status` until the meeting settles after a finalisation.
fn wait_settled(daemon: &DaemonHandle, id: &str, budget: Duration) -> Result<Value, String> {
    let deadline = std::time::Instant::now() + budget;
    loop {
        let status = daemon.call("meetings.status", json!({"meeting_id": id}))?;
        if matches!(
            status["status"].as_str(),
            Some("completed" | "failed" | "stopped")
        ) && status["is_finalizing"] != json!(true)
        {
            return Ok(status);
        }
        if std::time::Instant::now() > deadline {
            return Err(format!("the meeting never settled: {status}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn status_u64(status: &Value, key: &str) -> u64 {
    status["capture"][key].as_u64().unwrap_or(0)
}
