//! `first_insert`: the dictation pack's timing scenario reused without a
//! window. For each speech engine with a model on disk, a daemon in a
//! bench profile dictates the 11 s fixture through the mock microphone
//! once cold and then `iterations` times warm; each run measures the
//! stop request to the completion event and splits it with the event's
//! `timings` block. The insert leg goes through the mock inserter, so the
//! number is the engine tier's: p50 and p95 against NFR-1 on the GPU
//! tier and NFR-2 on the CPU tier.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::daemon::{BenchDaemon, Config};
use super::{Bench, Comparison, Spread, Status, Step};
use crate::nfr::{self, Tier};
use crate::scenarios::first_insert_timing::{EventTap, FirstInsertTiming, Run, Split};
use crate::stats::percentile;

/// One engine's row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineRow {
    /// The provider.
    pub provider: String,
    /// The model id.
    pub model: String,
    /// The backend the engine loaded on.
    pub backend: Option<String>,
    /// Why that backend.
    pub reason: Option<String>,
    /// The cold run's stop-to-completion time.
    pub cold_ms: Option<u64>,
    /// The warm runs.
    pub warm: Option<Spread>,
    /// p50 of each leg over the warm runs.
    pub split_p50: Split,
    /// Every run as the scenario records it.
    pub runs: Vec<Run>,
    /// Warm runs that did not complete, with why.
    pub failures: Vec<String>,
    /// The leg that went through the mock inserter.
    pub insert_leg: String,
    /// Warm runs the measurement asked for.
    pub warm_required: usize,
    /// Warm runs that completed with `inserted`; `warm` spreads these.
    pub warm_completed: usize,
    /// True when every required warm run completed: only then does the
    /// row carry a figure. A partial `warm` is kept beside the failures
    /// and never becomes the headline.
    pub qualified: bool,
    /// The warm p50, the figure held against the tier's target; absent
    /// until the row qualifies.
    pub figure: Option<f64>,
    /// Whether the p50 meets the tier's target.
    pub met: Option<bool>,
    /// The first take after the daemon started over the warm p50, once
    /// the row qualifies (ADR 0084).
    #[serde(default)]
    pub cold_ratio: Option<f64>,
    /// `cold_ratio` within `warmth::WARM_FACTOR`: the first take is within
    /// 20% of a warm one.
    #[serde(default)]
    pub cold_met: Option<bool>,
    /// How long after the daemon answered its ping the start-up preload
    /// reported the engine warm; the first take starts then.
    #[serde(default)]
    pub warm_wait_ms: Option<u64>,
}

/// Qualifies every row against `iterations` warm runs and the target:
/// a row with every warm run completed gets its figure and verdict, one
/// with fewer keeps its partial spread and failures and is named in the
/// returned list, which is the step's failure reason.
pub fn qualify(rows: &mut [EngineRow], iterations: usize, target: &nfr::Target) -> Vec<String> {
    let mut incomplete = Vec::new();
    for row in rows {
        row.warm_required = iterations;
        row.qualified = row.warm_completed == iterations;
        row.figure = row
            .qualified
            .then(|| row.warm.as_ref().map(|w| w.p50 as f64))
            .flatten();
        row.met = row.figure.map(|f| target.met(f));
        row.cold_ratio = row
            .figure
            .filter(|f| *f > 0.0)
            .zip(row.cold_ms)
            .map(|(warm, cold)| cold as f64 / warm);
        row.cold_met = row.cold_ratio.map(|r| r <= super::warmth::WARM_FACTOR);
        if !row.qualified {
            incomplete.push(format!(
                "{}: {} of {} warm runs completed; {}",
                row.provider,
                row.warm_completed,
                iterations,
                if row.failures.is_empty() {
                    "no failure recorded".to_string()
                } else {
                    row.failures.join("; ")
                }
            ));
        }
    }
    incomplete
}

/// Runs the step; also the tier the daemon reported once the first
/// engine had loaded, which the report takes over.
pub fn run(bench: &Bench, tier_hint: Tier) -> (Step, Option<(Tier, String)>) {
    let mut rows = Vec::new();
    let mut skipped = Vec::new();
    let mut seen_tier = None;
    let mut failed = None;
    for provider in ["whisper", "parakeet"] {
        let model = match bench.models.get(provider) {
            Ok(m) => m.clone(),
            Err(why) => {
                skipped.push(why);
                continue;
            }
        };
        if bench
            .engine(&format!("dettivo-engine-{provider}"))
            .is_none()
        {
            skipped.push(format!("dettivo-engine-{provider} is not built"));
            continue;
        }
        match measure(bench, &model.provider, &model.id) {
            Ok((row, tier)) => {
                if seen_tier.is_none() {
                    seen_tier = Some(tier);
                }
                rows.push(row);
            }
            Err(e) => failed = Some(format!("{provider}: {e}")),
        }
    }
    let tier = seen_tier.as_ref().map(|(t, _)| *t).unwrap_or(tier_hint);
    let target = nfr::first_insert(tier);
    // A row short of its warm runs fails the step: the failures stay in
    // the JSON and the partial spread beside them, but no figure is
    // reported from the runs that happened to succeed.
    let incomplete = qualify(&mut rows, bench.iterations(), &target);
    if !incomplete.is_empty() {
        let note = incomplete.join("; ");
        failed = Some(match failed {
            Some(f) => format!("{f}; {note}"),
            None => note,
        });
    }
    let results = rows
        .iter()
        .map(|row| serde_json::to_value(row).unwrap_or(Value::Null))
        .collect();
    // The headline figure is the tier's engine's (ADR 0029); the other
    // rows carry their own verdict.
    let headline = rows
        .iter()
        .find(|r| r.provider == nfr::headline_provider(tier))
        .or(rows.first())
        .and_then(|r| r.figure);
    let status = if failed.is_some() {
        Status::Failed
    } else if rows.is_empty() {
        Status::Skipped
    } else {
        Status::Measured
    };
    let mut reason = failed;
    if !skipped.is_empty() {
        let note = skipped.join("; ");
        reason = Some(match reason {
            Some(r) => format!("{r}; {note}"),
            None => note,
        });
    }
    (
        Step {
            name: "first_insert".into(),
            status,
            reason,
            comparison: Some(Comparison::of(&target, headline)),
            results,
            duration_ms: 0,
        },
        seen_tier,
    )
}

fn measure(
    bench: &Bench,
    provider: &str,
    model: &str,
) -> Result<(EngineRow, (Tier, String)), String> {
    let daemon = BenchDaemon::start(
        bench,
        &format!("first-insert-{provider}"),
        &Config {
            provider: provider.into(),
            model: model.into(),
            llm_model: None,
            idle_seconds: 300,
        },
    )?;
    let insert = daemon.insert_daemon();
    let target = daemon.call("insert.target", json!({}))?;
    let pid = u32::try_from(target["target"]["pid"].as_u64().unwrap_or(0)).unwrap_or(0);
    let tap = EventTap::subscribe(&daemon.handle.socket)?;
    let warm_wait_ms = wait_warm(&daemon, provider)?;
    let mut runs = Vec::new();
    let mut failures = Vec::new();
    for i in 0..=bench.iterations() {
        match FirstInsertTiming::dictate(&insert, &tap, &target, pid, i == 0) {
            Ok(run) => {
                if run.outcome != "inserted" {
                    failures.push(format!("{}: {}", run.job_id, run.outcome));
                }
                runs.push(run);
            }
            Err(e) => failures.push(e),
        }
    }
    let tier = daemon.tier()?;
    let engine = daemon
        .engines()?
        .into_iter()
        .find(|e| e["binary"] == json!(format!("dettivo-engine-{provider}")))
        .unwrap_or(Value::Null);
    daemon.stop();
    let warm: Vec<&Run> = runs
        .iter()
        .filter(|r| !r.cold && r.outcome == "inserted")
        .collect();
    let pick = |f: fn(&Run) -> u64| -> Vec<u64> { warm.iter().map(|r| f(r)).collect() };
    let split_p50 = Split {
        capture_ms: percentile(&pick(|r| r.capture_ms), 0.5).unwrap_or(0),
        transcribe_ms: percentile(&pick(|r| r.transcribe_ms), 0.5).unwrap_or(0),
        insert_ms: percentile(&pick(|r| r.insert_ms), 0.5).unwrap_or(0),
    };
    let warm_completed = warm.len();
    let row = EngineRow {
        provider: provider.into(),
        model: model.into(),
        backend: engine["backend"].as_str().map(str::to_string),
        reason: engine["reason"].as_str().map(str::to_string),
        cold_ms: runs.iter().find(|r| r.cold).map(|r| r.total_ms),
        warm: Spread::of(pick(|r| r.total_ms)),
        split_p50,
        runs,
        failures,
        insert_leg: "mock".into(),
        warm_required: bench.iterations(),
        warm_completed,
        qualified: false,
        figure: None,
        met: None,
        cold_ratio: None,
        cold_met: None,
        warm_wait_ms: Some(warm_wait_ms),
    };
    Ok((row, tier))
}

/// Waits until the start-up preload reports `provider`'s engine warm
/// (`speech.engines`), the state a first dictation after daemon start
/// meets once the daemon has had its moment; at most `WARM_WAIT`.
fn wait_warm(daemon: &BenchDaemon, provider: &str) -> Result<u64, String> {
    let binary = format!("dettivo-engine-{provider}");
    let started = std::time::Instant::now();
    while started.elapsed() < WARM_WAIT {
        let warm = daemon
            .engines()?
            .iter()
            .any(|e| e["binary"] == json!(binary) && e["warm"] == json!(true));
        if warm {
            return Ok(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    Err(format!(
        "{binary} was not warm {} s after start",
        WARM_WAIT.as_secs()
    ))
}

/// The longest the first take waits for the start-up preload.
const WARM_WAIT: std::time::Duration = std::time::Duration::from_secs(120);

#[cfg(test)]
mod tests {
    use super::*;

    fn run(i: usize, ok: bool) -> Run {
        Run {
            job_id: format!("job_dict_{i}"),
            cold: false,
            total_ms: 500 + i as u64,
            capture_ms: 10,
            transcribe_ms: 400,
            insert_ms: 5,
            outcome: if ok { "inserted" } else { "failed" }.into(),
            backend: ok.then(|| "mock".to_string()),
        }
    }

    /// A whisper row with `ok` of ten warm runs completed, as `measure`
    /// records it before qualification.
    fn row(ok: usize) -> EngineRow {
        let runs: Vec<Run> = (0..10).map(|i| run(i, i < ok)).collect();
        let warm: Vec<u64> = runs
            .iter()
            .filter(|r| r.outcome == "inserted")
            .map(|r| r.total_ms)
            .collect();
        EngineRow {
            provider: "whisper".into(),
            model: "tiny.en".into(),
            backend: None,
            reason: None,
            cold_ms: Some(900),
            warm: Spread::of(warm),
            split_p50: Split::default(),
            failures: runs
                .iter()
                .filter(|r| r.outcome != "inserted")
                .map(|r| format!("{}: {}", r.job_id, r.outcome))
                .collect(),
            runs,
            insert_leg: "mock".into(),
            warm_required: 0,
            warm_completed: ok,
            qualified: false,
            figure: None,
            met: None,
            cold_ratio: None,
            cold_met: None,
            warm_wait_ms: None,
        }
    }

    #[test]
    fn a_row_qualifies_only_with_every_warm_run_completed() {
        let target = nfr::first_insert(Tier::Gpu);
        for (ok, expect_figure) in [(1, false), (9, false), (10, true)] {
            let mut rows = vec![row(ok)];
            let incomplete = qualify(&mut rows, 10, &target);
            let row = &rows[0];
            assert_eq!(row.qualified, expect_figure, "{ok} of 10");
            assert_eq!(row.figure.is_some(), expect_figure, "{ok} of 10");
            assert_eq!(row.met.is_some(), expect_figure, "{ok} of 10");
            assert_eq!(incomplete.is_empty(), expect_figure, "{ok} of 10");
            // The partial spread and every failure stay in the row.
            assert_eq!(row.warm.as_ref().map(|w| w.iterations), Some(ok));
            assert_eq!(row.failures.len(), 10 - ok);
            assert_eq!(row.warm_required, 10);
            // The first take is held against the warm p50 only with a figure.
            assert_eq!(row.cold_ratio.is_some(), expect_figure, "{ok} of 10");
            if expect_figure {
                assert_eq!(row.cold_met, Some(false), "900 ms against a ~505 ms p50");
            }
            if !expect_figure {
                assert!(
                    incomplete[0].starts_with(&format!("whisper: {ok} of 10 warm runs completed;")),
                    "{incomplete:?}"
                );
            }
        }
    }
}
