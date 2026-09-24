//! First-insert timing (fn-14 R3, NFR-1): eleven dictations of the speech
//! fixture through the mock microphone into the Qt insert target over the
//! real insertion chain. Each run measures the path from the stop request
//! to the completion event on the daemon's stream, split into capture,
//! transcribe and insert from the event's `timings` block. The first run
//! warms the engine and is excluded; p50 and p95 over the ten warm runs
//! land in `first-insert-timing.json` with the engine's backend, and the
//! pack report compares them with the NFR-1 target for the machine's tier.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::daemon::local_model;
use super::insertion_matrix::FIELD;
use super::support::InsertDaemon;
use super::{Context, Scenario, binary};
use crate::driver::{Driver, Launch};
use crate::stats::percentile;

/// Warm runs the percentiles need.
pub const WARM_RUNS: usize = 10;
/// How much of the 11 s fixture is recorded before the stop.
const RECORD_FOR: Duration = Duration::from_secs(10);
/// The longest wait for one completion (a CPU tier on a loaded runner).
const COMPLETION_WAIT: Duration = Duration::from_secs(120);

/// One run's figures, as `first-insert-timing.json` records them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    /// The session's job id.
    pub job_id: String,
    /// The first run, excluded from the percentiles.
    pub cold: bool,
    /// Stop request to completion event, in milliseconds.
    pub total_ms: u64,
    /// From the daemon's `timings` block.
    pub capture_ms: u64,
    /// From the daemon's `timings` block.
    pub transcribe_ms: u64,
    /// From the daemon's `timings` block.
    pub insert_ms: u64,
    /// The completion's insertion outcome.
    pub outcome: String,
    /// The backend that inserted.
    pub backend: Option<String>,
}

/// The p50 of each part of the path over the warm runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Split {
    /// Capture p50.
    pub capture_ms: u64,
    /// Transcribe p50.
    pub transcribe_ms: u64,
    /// Insert p50.
    pub insert_ms: u64,
}

/// `first-insert-timing.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timing {
    /// Audio recorded per run.
    pub record_for_ms: u64,
    /// Every attempted run, the cold one first.
    pub runs: Vec<Run>,
    /// Warm runs that completed with an insertion.
    pub warm_completed: usize,
    /// p50 over the warm runs' `total_ms`.
    pub p50_ms: Option<u64>,
    /// p95 over the warm runs' `total_ms`.
    pub p95_ms: Option<u64>,
    /// p50 of each part over the warm runs.
    pub split_p50: Split,
    /// The backend the Whisper engine reported after the runs.
    pub engine_backend: Option<String>,
    /// The `platform` block of `system.capabilities`.
    pub platform: Value,
}

impl Timing {
    /// Builds the summary from the attempted runs.
    pub fn summarise(
        record_for_ms: u64,
        runs: Vec<Run>,
        engine_backend: Option<String>,
        platform: Value,
    ) -> Self {
        let warm: Vec<&Run> = runs
            .iter()
            .filter(|r| !r.cold && r.outcome == "inserted")
            .collect();
        let pick = |f: fn(&Run) -> u64| -> Vec<u64> { warm.iter().map(|r| f(r)).collect() };
        let totals = pick(|r| r.total_ms);
        Self {
            record_for_ms,
            warm_completed: warm.len(),
            p50_ms: percentile(&totals, 0.5),
            p95_ms: percentile(&totals, 0.95),
            split_p50: Split {
                capture_ms: percentile(&pick(|r| r.capture_ms), 0.5).unwrap_or(0),
                transcribe_ms: percentile(&pick(|r| r.transcribe_ms), 0.5).unwrap_or(0),
                insert_ms: percentile(&pick(|r| r.insert_ms), 0.5).unwrap_or(0),
            },
            runs,
            engine_backend,
            platform,
        }
    }
}

/// Collects `dictation.state` payloads with the instant they arrived.
pub(crate) struct EventTap {
    seen: Arc<Mutex<Vec<(Value, Instant)>>>,
}

impl EventTap {
    pub(crate) fn subscribe(socket: &std::path::Path) -> Result<Self, String> {
        let mut stream = UnixStream::connect(socket).map_err(|e| format!("subscribe: {e}"))?;
        let req = json!({"jsonrpc": "2.0", "id": "t", "method": "events.subscribe", "params": {"topics": ["dictation.state"], "buffer": 256}});
        stream
            .write_all(format!("{req}\n").as_bytes())
            .map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        std::thread::spawn(move || {
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|n| n > 0) {
                if let Ok(v) = serde_json::from_str::<Value>(line.trim_end()) {
                    if v["method"] == "events.notify" {
                        sink.lock()
                            .unwrap()
                            .push((v["params"]["payload"].clone(), Instant::now()));
                    }
                }
                line.clear();
            }
        });
        Ok(Self { seen })
    }

    /// The completion payload of `job_id` and when it arrived.
    fn completion(&self, job_id: &str, timeout: Duration) -> Result<(Value, Instant), String> {
        let deadline = Instant::now() + timeout;
        loop {
            let found = self.seen.lock().unwrap().iter().rev().find_map(|(p, at)| {
                (p["job_id"] == json!(job_id)
                    && p["state"] == json!("idle")
                    && p["previous_state"] == json!("inserting"))
                .then(|| (p.clone(), *at))
            });
            if let Some(hit) = found {
                return Ok(hit);
            }
            if Instant::now() > deadline {
                let states: Vec<String> = self
                    .seen
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|(p, _)| p["job_id"] == json!(job_id))
                    .map(|(p, _)| p["state"].as_str().unwrap_or("?").to_string())
                    .collect();
                return Err(format!(
                    "{job_id}: no completion within {timeout:?}; states seen {states:?}"
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// The scenario.
pub struct FirstInsertTiming;

impl FirstInsertTiming {
    fn config(repo_root: &std::path::Path) -> String {
        let bin_dir = binary(repo_root, "dettivod")
            .ok()
            .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_default();
        format!(
            "[engines]\ndirectory = \"{}\"\n[speech]\nmodel = \"tiny.en\"\n[dictation]\nlanguage = \"en\"\n[hotkeys]\nbackend = \"none\"\n",
            bin_dir.display()
        )
    }

    /// One dictation: start with the target guard, record, stop, and read
    /// the completion off the stream.
    pub(crate) fn dictate(
        daemon: &InsertDaemon,
        tap: &EventTap,
        target: &Value,
        pid: u32,
        cold: bool,
    ) -> Result<Run, String> {
        let started = daemon.call(
            "dictation.start",
            json!({
                "language": "en",
                "mode": "raw",
                "expected_target_bundle_id": target["target"]["app_id"],
                "expected_target_pid": pid.to_string(),
            }),
        )?;
        let job_id = started["job"]["job_id"]
            .as_str()
            .ok_or("dictation.start carries no job_id")?
            .to_string();
        std::thread::sleep(RECORD_FOR);
        let released = Instant::now();
        daemon.call("dictation.stop", json!({}))?;
        let (payload, arrived) = tap.completion(&job_id, COMPLETION_WAIT)?;
        // The split is the measurement; a payload without it fails the run
        // instead of reading as a zero.
        let timings = payload
            .get("timings")
            .filter(|t| t.is_object())
            .ok_or_else(|| format!("completion payload for {job_id} carries no timings block"))?;
        let ms = |key: &str| -> Result<u64, String> {
            timings[key]
                .as_u64()
                .ok_or_else(|| format!("timings.{key} is missing or not a number: {timings}"))
        };
        Ok(Run {
            job_id,
            cold,
            total_ms: u64::try_from(arrived.duration_since(released).as_millis())
                .unwrap_or(u64::MAX),
            capture_ms: ms("capture_ms")?,
            transcribe_ms: ms("transcribe_ms")?,
            insert_ms: ms("insert_ms")?,
            outcome: payload["insertion"]["outcome"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            backend: payload["insertion"]["backend"].as_str().map(str::to_string),
        })
    }

    fn whisper_backend(daemon: &InsertDaemon) -> Option<String> {
        let engines = daemon.call("speech.engines", json!({})).ok()?;
        engines["engines"]
            .as_array()?
            .iter()
            .find(|e| e["binary"] == json!("dettivo-engine-whisper"))
            .and_then(|e| e["backend"].as_str().map(str::to_string))
    }
}

impl Scenario for FirstInsertTiming {
    fn id(&self) -> &'static str {
        "first_insert_timing"
    }

    fn summary(&self) -> &'static str {
        "ten warm dictations into the Qt target time the release-to-inserted path for NFR-1"
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        local_model(ctx.profile).map(|_| ()).ok_or_else(|| {
            "tiny.en or jfk.wav missing under the model directory (scripts/models/fetch-test-model.sh)".to_string()
        })?;
        binary(ctx.repo_root, "dettivo-insert-target").map(|_| ())
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let (_, wav) = local_model(ctx.profile).ok_or("model missing")?;
        std::fs::write(
            ctx.profile.root.join("cfg/dettivo/config.toml"),
            Self::config(ctx.repo_root),
        )
        .map_err(|e| format!("config: {e}"))?;
        let mock_mic = BTreeMap::from([(
            "DETTIVO_MOCK_MIC".to_string(),
            wav.to_string_lossy().into_owned(),
        )]);
        let daemon = InsertDaemon::start_with(ctx, &mock_mic)?;
        let platform = daemon.call("system.capabilities", json!({}))?["platform"].clone();
        ctx.timings.mark("daemon");

        let program = binary(ctx.repo_root, "dettivo-insert-target")?;
        let app = driver
            .launch(
                &Launch {
                    program,
                    args: Vec::new(),
                    env: ctx.profile.env(),
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

        let tap = EventTap::subscribe(&daemon.socket)?;
        let mut runs = Vec::new();
        let mut failures = Vec::new();
        for i in 0..=WARM_RUNS {
            match Self::dictate(&daemon, &tap, &target, app.pid, i == 0) {
                Ok(run) => {
                    if run.outcome != "inserted" {
                        failures.push(format!("{}: {}", run.job_id, run.outcome));
                    }
                    runs.push(run);
                }
                Err(e) => failures.push(e),
            }
        }
        ctx.timings.mark("runs");
        let timing = Timing::summarise(
            u64::try_from(RECORD_FOR.as_millis()).unwrap_or(u64::MAX),
            runs,
            Self::whisper_backend(&daemon),
            platform,
        );
        std::fs::write(
            ctx.evidence_dir.join("first-insert-timing.json"),
            serde_json::to_string_pretty(&timing).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("first-insert-timing.json".into());

        let read_back = driver
            .find(&app, FIELD)
            .and_then(|f| driver.read_value(&app, &f))
            .map_err(|e| format!("read back: {e}"))?;
        let shot = ctx.evidence_dir.join("dettivo-insert-target.png");
        if driver.screenshot(&app, &shot).is_ok() {
            ctx.evidence.push("dettivo-insert-target.png".into());
        }
        let _ = driver.close(&app);
        if timing.warm_completed < WARM_RUNS {
            return Err(format!(
                "{} of {WARM_RUNS} warm runs completed: {}",
                timing.warm_completed,
                failures.join("; ")
            ));
        }
        if !read_back.to_lowercase().contains("fellow") {
            return Err(format!(
                "the target field holds {} characters without the fixture's words",
                read_back.chars().count()
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(n: u64, cold: bool, outcome: &str) -> Run {
        Run {
            job_id: format!("job_dict_{n}"),
            cold,
            total_ms: n * 100,
            capture_ms: 10,
            transcribe_ms: n * 80,
            insert_ms: 5,
            outcome: outcome.into(),
            backend: Some("mock".into()),
        }
    }

    #[test]
    fn the_cold_run_and_failed_runs_stay_out_of_the_percentiles() {
        let mut runs = vec![run(50, true, "inserted")];
        runs.extend((1..=10).map(|n| run(n, false, "inserted")));
        let t = Timing::summarise(10_000, runs.clone(), Some("cpu".into()), json!({}));
        assert_eq!(t.warm_completed, 10);
        assert_eq!((t.p50_ms, t.p95_ms), (Some(500), Some(1000)));
        assert_eq!(t.split_p50.transcribe_ms, 400);
        runs[10].outcome = "failed".into();
        let t = Timing::summarise(10_000, runs, None, json!({}));
        assert_eq!(t.warm_completed, 9);
        assert_eq!(t.p95_ms, Some(900));
    }
}
