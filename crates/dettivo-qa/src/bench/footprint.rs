//! `idle_footprint` and `startup`. The footprint: a daemon in a bench
//! profile with the idle timeout at ten seconds loads the speech engine
//! through one dictation and the language model through one Enhanced
//! `polish.test`, then the step watches `speech.engines` until every
//! engine is gone (an engine still alive after the timeout plus the
//! reaper's interval is named), idles five minutes in all (thirty seconds
//! under `--quick`) sampling the daemon's resident set, and holds the
//! last sample against NFR-6. The startup: `dettivod` is started
//! `iterations` times and the socket-ready time held against NFR-8; the
//! app's first frame comes from the `app_routes` drive when a display and
//! the accessibility bus are present.

use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::daemon::{BenchDaemon, Config};
use super::{Bench, Comparison, Spread, Status, Step};
use crate::nfr;
use crate::scenarios::first_insert_timing::{EventTap, FirstInsertTiming};

/// `[engines] *_idle_seconds` for the footprint daemon.
pub const IDLE_SECONDS: u64 = 10;
/// The daemon's reaper interval (`dettivod::engines::REAP_INTERVAL`).
pub const REAP_INTERVAL: Duration = Duration::from_secs(30);
/// How long the daemon idles in all on a full run.
pub const IDLE_FOR: Duration = Duration::from_secs(300);
/// Under `--quick`.
pub const QUICK_IDLE_FOR: Duration = Duration::from_secs(30);
/// How often the resident set is sampled.
pub const SAMPLE_EVERY: Duration = Duration::from_secs(10);

/// One engine's unload observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineUnload {
    /// The binary.
    pub binary: String,
    /// The backend it had loaded on.
    pub backend: Option<String>,
    /// Its resident set while loaded.
    pub loaded_memory_bytes: Option<u64>,
    /// Whether it was gone within the timeout plus the reaper's interval.
    pub unloaded: bool,
    /// Milliseconds after the last request until it was gone.
    pub unloaded_after_ms: Option<u64>,
}

/// The footprint row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FootprintRow {
    /// The speech provider loaded.
    pub provider: String,
    /// The speech model.
    pub model: String,
    /// The language model loaded, when one was.
    pub llm_model: Option<String>,
    /// What the Enhanced `polish.test` that loads it answered.
    pub llm_probe: Option<String>,
    /// The idle timeout configured.
    pub idle_seconds: u64,
    /// How long the daemon idled in all.
    pub idled_ms: u64,
    /// Every engine that had loaded, with its unload.
    pub engines: Vec<EngineUnload>,
    /// Engines still alive after the timeout plus the reaper's interval.
    pub still_alive: Vec<String>,
    /// The daemon's resident set while the engines were loaded.
    pub rss_loaded_bytes: Option<u64>,
    /// The resident set samples over the idle, ten seconds apart.
    pub rss: Option<Spread>,
    /// The last sample, the figure held against NFR-6.
    pub rss_idle_bytes: Option<u64>,
}

/// `idle_footprint`.
pub fn idle(bench: &Bench) -> Step {
    let name = "idle_footprint";
    let speech = match bench
        .models
        .get("whisper")
        .or_else(|_| bench.models.get("parakeet"))
    {
        Ok(m) => m.clone(),
        Err(why) => return Step::without(name, Status::Skipped, why),
    };
    let llm_model = bench.models.get("llm").ok().map(|m| m.id.clone());
    let daemon = match BenchDaemon::start(
        bench,
        "idle-footprint",
        &Config {
            provider: speech.provider.clone(),
            model: speech.id.clone(),
            llm_model: llm_model.clone(),
            idle_seconds: IDLE_SECONDS,
        },
    ) {
        Ok(d) => d,
        Err(e) => return Step::without(name, Status::Failed, e),
    };
    let result = measure(bench, &daemon, &speech.provider, &speech.id, llm_model);
    daemon.stop();
    match result {
        Ok(row) => {
            let value = row.rss_idle_bytes.map(|b| b as f64);
            let status = if row.still_alive.is_empty() {
                Status::Measured
            } else {
                Status::Failed
            };
            Step {
                name: name.into(),
                status,
                reason: (!row.still_alive.is_empty()).then(|| {
                    format!(
                        "still alive after the idle timeout and the reaper's interval: {}",
                        row.still_alive.join(", ")
                    )
                }),
                comparison: Some(Comparison::of(&nfr::NFR6_IDLE_RSS, value)),
                results: vec![serde_json::to_value(&row).unwrap_or(Value::Null)],
                duration_ms: 0,
            }
        }
        Err(e) => Step::without(name, Status::Failed, e),
    }
}

fn measure(
    bench: &Bench,
    daemon: &BenchDaemon,
    provider: &str,
    model: &str,
    llm_model: Option<String>,
) -> Result<FootprintRow, String> {
    let insert = daemon.insert_daemon();
    let target = daemon.call("insert.target", json!({}))?;
    let pid = u32::try_from(target["target"]["pid"].as_u64().unwrap_or(0)).unwrap_or(0);
    let tap = EventTap::subscribe(&daemon.handle.socket)?;
    FirstInsertTiming::dictate(&insert, &tap, &target, pid, true)?;
    let mut llm_probe = None;
    if llm_model.is_some() {
        // An Enhanced rewrite loads the language model; a fallback to
        // the deterministic text still answers, so the engines list below
        // is what says whether the process is there.
        llm_probe = Some(
            match daemon.call(
                "polish.test",
                json!({
                    "input": "um so i think we should ship this tomorrow",
                    "mode": "enhanced",
                    "preset": "generic",
                    "rules": [],
                }),
            ) {
                Ok(v) => format!("polish.test answered with model {}", v["model"]),
                Err(e) => format!("polish.test failed: {e}"),
            },
        );
    }
    let last_request = Instant::now();
    let loaded: Vec<Value> = daemon
        .engines()?
        .into_iter()
        .filter(|e| e["running"] == json!(true))
        .collect();
    let llm_loaded = loaded
        .iter()
        .any(|e| e["binary"] == json!("dettivo-engine-llm"));
    let rss_loaded_bytes = daemon.rss_bytes();
    let mut unloads: Vec<EngineUnload> = loaded
        .iter()
        .map(|e| EngineUnload {
            binary: e["binary"].as_str().unwrap_or("").to_string(),
            backend: e["backend"].as_str().map(str::to_string),
            loaded_memory_bytes: e["memory_bytes"].as_u64(),
            unloaded: false,
            unloaded_after_ms: None,
        })
        .collect();
    let deadline =
        last_request + Duration::from_secs(IDLE_SECONDS) + REAP_INTERVAL + Duration::from_secs(15);
    while unloads.iter().any(|u| !u.unloaded) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_secs(2));
        let running: Vec<String> = daemon
            .engines()?
            .into_iter()
            .filter(|e| e["running"] == json!(true))
            .filter_map(|e| e["binary"].as_str().map(str::to_string))
            .collect();
        for u in unloads.iter_mut().filter(|u| !u.unloaded) {
            if !running.contains(&u.binary) {
                u.unloaded = true;
                u.unloaded_after_ms =
                    Some(u64::try_from(last_request.elapsed().as_millis()).unwrap_or(u64::MAX));
            }
        }
    }
    let still_alive: Vec<String> = unloads
        .iter()
        .filter(|u| !u.unloaded)
        .map(|u| u.binary.clone())
        .collect();
    let idle_for = if bench.opts.quick {
        QUICK_IDLE_FOR
    } else {
        IDLE_FOR
    };
    let mut samples = Vec::new();
    while last_request.elapsed() < idle_for {
        std::thread::sleep(
            SAMPLE_EVERY
                .min(idle_for.saturating_sub(last_request.elapsed()))
                .max(Duration::from_millis(100)),
        );
        if let Some(rss) = daemon.rss_bytes() {
            samples.push(rss);
        }
    }
    let rss_idle_bytes = samples.last().copied();
    Ok(FootprintRow {
        provider: provider.into(),
        model: model.into(),
        llm_model: llm_loaded.then_some(llm_model).flatten(),
        llm_probe,
        idle_seconds: IDLE_SECONDS,
        idled_ms: u64::try_from(last_request.elapsed().as_millis()).unwrap_or(u64::MAX),
        engines: unloads,
        still_alive,
        rss_loaded_bytes,
        rss: Spread::of(samples),
        rss_idle_bytes,
    })
}

/// The startup row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StartupRow {
    /// Socket-ready times: `dettivod` started until `system.ping` answers.
    pub socket_ready: Option<Spread>,
    /// The app's first frame from the `app_routes` drive.
    pub app_first_frame_ms: Option<u64>,
    /// The app's resident set after five seconds, from the same drive.
    pub app_rss_kb_after_5s: Option<u64>,
    /// Why the app was not driven.
    pub app_reason: Option<String>,
    /// Whether the first frame meets NFR-8.
    pub app_first_frame_met: Option<bool>,
}

/// `startup`: the profile is prepared first, and the clock runs from the
/// process spawn to the answered ping.
pub fn startup(bench: &Bench) -> Step {
    let name = "startup";
    let config = Config {
        provider: "whisper".into(),
        model: "tiny.en".into(),
        llm_model: None,
        idle_seconds: 300,
    };
    let mut i = 0;
    let samples = match spawn_times(
        bench.iterations(),
        || {
            i += 1;
            BenchDaemon::prepare(bench, &format!("startup-{i}"), &config)
        },
        |prepared| prepared.spawn().map(BenchDaemon::stop),
    ) {
        Ok(samples) => samples,
        Err(e) => return Step::without(name, Status::Failed, e),
    };
    let socket_ready = Spread::of(samples);
    let (app_first_frame_ms, app_rss_kb_after_5s, app_reason) = app_first_frame(bench);
    let row = StartupRow {
        socket_ready: socket_ready.clone(),
        app_first_frame_ms,
        app_rss_kb_after_5s,
        app_reason,
        app_first_frame_met: app_first_frame_ms.map(|ms| nfr::NFR8_APP_FIRST_FRAME.met(ms as f64)),
    };
    Step {
        name: name.into(),
        status: Status::Measured,
        reason: row
            .app_reason
            .clone()
            .map(|r| format!("app first frame: {r}")),
        comparison: Some(Comparison::of(
            &nfr::NFR8_SOCKET_READY,
            socket_ready.map(|s| s.p50 as f64),
        )),
        results: vec![serde_json::to_value(&row).unwrap_or(Value::Null)],
        duration_ms: 0,
    }
}

/// `iterations` spawn times in milliseconds: `prepare` runs outside the
/// clock, `spawn` inside it.
fn spawn_times<P>(
    iterations: usize,
    mut prepare: impl FnMut() -> Result<P, String>,
    mut spawn: impl FnMut(P) -> Result<(), String>,
) -> Result<Vec<u64>, String> {
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let prepared = prepare()?;
        let at = Instant::now();
        spawn(prepared)?;
        samples.push(u64::try_from(at.elapsed().as_millis()).unwrap_or(u64::MAX));
    }
    Ok(samples)
}

/// The app's first frame through the `app_routes` drive on the fallback
/// driver, when a display and the accessibility bus are present.
fn app_first_frame(bench: &Bench) -> (Option<u64>, Option<u64>, Option<String>) {
    let checks = crate::doctor::checks();
    if let Err(why) = crate::doctor::drive_ready(&checks) {
        return (None, None, Some(why));
    }
    if crate::scenarios::binary(&bench.opts.repo_root, "dettivo-app").is_err() {
        return (None, None, Some("dettivo-app is not built".into()));
    }
    let Some(scenario) = crate::scenarios::by_id("app_routes") else {
        return (None, None, Some("app_routes is not a scenario".into()));
    };
    let options = crate::runner::Options {
        driver: "atspi".into(),
        evidence_base: bench.opts.evidence_base.clone(),
        models_dir: bench.opts.models_dir.clone(),
        keep_profile: false,
        timeout: Duration::from_secs(15),
        run_dir: Some(bench.run_dir.clone()),
        env: Default::default(),
    };
    match crate::runner::drive(&bench.opts.repo_root, scenario.as_ref(), &options) {
        Ok(receipt) => {
            let startup = bench.run_dir.join("app_routes.atspi/startup.json");
            match read_startup(&startup) {
                Some((frame, rss)) => (Some(frame), rss, None),
                None => (
                    None,
                    None,
                    Some(format!(
                        "app_routes {:?}: {}",
                        receipt.outcome,
                        receipt.reason.unwrap_or_else(|| "no startup.json".into())
                    )),
                ),
            }
        }
        Err(e) => (None, None, Some(format!("app_routes: {e}"))),
    }
}

fn read_startup(path: &Path) -> Option<(u64, Option<u64>)> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    Some((v["first_frame_ms"].as_u64()?, v["rss_kb_after_5s"].as_u64()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_startup_clock_excludes_the_profile_preparation() {
        // A preparation ten times the spawn must leave the sample the
        // spawn's alone.
        let samples = spawn_times(
            3,
            || {
                std::thread::sleep(Duration::from_millis(60));
                Ok(())
            },
            |()| {
                std::thread::sleep(Duration::from_millis(5));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(samples.len(), 3);
        assert!(
            samples.iter().all(|&ms| (5..50).contains(&ms)),
            "{samples:?}"
        );
        let failed = spawn_times(2, || Err::<(), _>("no profile".to_string()), |()| Ok(()));
        assert_eq!(failed.unwrap_err(), "no profile");
    }

    #[test]
    fn the_startup_json_is_read_for_the_first_frame() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("startup.json");
        std::fs::write(
            &path,
            r#"{"first_frame_ms": 120, "rss_kb_after_5s": 40000}"#,
        )
        .unwrap();
        assert_eq!(read_startup(&path), Some((120, Some(40_000))));
        std::fs::write(&path, r#"{"theme": "x"}"#).unwrap();
        assert_eq!(read_startup(&path), None);
    }
}
