//! `dettivo-qa polish-eval` (ADR 0032): the macOS polish bench and scorer
//! on Linux. A held-out set in the macOS JSONL shape runs row by row
//! through a daemon of its own (`polish.test` in `enhanced` mode against
//! the local engine, so the prompt profile, the guards, the repair pass
//! and the fallback are the product's), the ported scorer computes the
//! macOS metrics, the ported gate checks the hard gates and an
//! incumbent, and the report lands under `docs/reports/polish-eval/`
//! with metrics and row ids only. `--engine-only` measures raw
//! generation through `dettivo-engine-llm --prompt --raw` instead, the
//! `polish_bench` number. `--golden` compares the metrics with a
//! checked-in file and fails naming the first that differs, which is
//! what CI runs on the sample set with the fixture LLM.

mod compatibility;
#[cfg(test)]
mod compatibility_tests;
pub mod dataset;
pub mod gate;
pub mod report;
pub mod runner;
pub mod scorer;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde_json::Value;

use dataset::Sample;
use gate::{Incumbent, Targets};
use report::{Baseline, EngineOnly, Report};
use runner::{Candidate, Runner};
use scorer::Runtime;

/// The targets file, relative to the repository root.
pub const TARGETS: &str = "qa/polish-eval/targets.json";
/// Where reports land, relative to the repository root.
pub const REPORTS_DIR: &str = "docs/reports/polish-eval";
/// The macOS baseline, inside `REPORTS_DIR`.
pub const BASELINE: &str = "macos-baseline.json";

/// One run.
#[derive(Debug, Clone)]
pub struct Options {
    /// The set.
    pub set: PathBuf,
    /// The candidate (`--model`).
    pub model: String,
    /// The split.
    pub split: String,
    /// An incumbent report for the relative gates.
    pub incumbent: Option<PathBuf>,
    /// Raw generation instead of the pipeline.
    pub engine_only: bool,
    /// Where the report goes; `None` skips writing.
    pub out: Option<PathBuf>,
    /// A golden the metrics must match.
    pub golden: Option<PathBuf>,
    /// Pin the engine to the CPU.
    pub cpu: bool,
    /// The experiments directory a sideload reads.
    pub experiments_dir: Option<PathBuf>,
    /// The machine's model directory.
    pub models_dir: Option<PathBuf>,
    /// Write every row's output here (outside the repository; for a
    /// reviewer's eyes).
    pub outputs: Option<PathBuf>,
    /// The targets file.
    pub targets: Option<PathBuf>,
    /// The baseline file.
    pub baseline: Option<PathBuf>,
    /// The baseline entry to compare with; the model id by default.
    pub baseline_model: Option<String>,
    /// `[llm] timeout_ms` for the run.
    pub timeout_ms: u64,
    /// The directory holding `dettivo-engine-llm`; the daemon's when `None`.
    pub engines_dir: Option<PathBuf>,
}

/// What a run produced.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// The report.
    pub report: Report,
    /// Where it was written.
    pub path: Option<PathBuf>,
    /// The golden metrics that differed, one line each.
    pub golden_mismatches: Vec<String>,
}

/// The incumbent's metrics and backend out of its report.
fn incumbent(
    path: &Path,
    comparison: &compatibility::Comparison,
    split: &str,
    rows: &[Sample],
) -> Result<Incumbent, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let report: Report =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let incompatible = compatibility::mismatch(&report, comparison, split, rows);
    Ok(Incumbent {
        model: report.model,
        backend: report.backend,
        metrics: report.metrics.as_map(),
        incompatible,
    })
}

/// What the pipeline run produced.
struct Pipeline {
    scored: Vec<(Sample, Runtime)>,
    backend: String,
    model_file: Option<String>,
    /// The SHA-256 of the model file, taken while the profile that holds
    /// it still exists.
    model_sha256: Option<String>,
    engine: Option<PathBuf>,
    /// The id the daemon served the candidate under.
    served_id: String,
    /// The raw generation figure, with `--engine-only`.
    engine_only: Option<EngineOnly>,
}

/// Runs the rows through the daemon.
fn run_pipeline(repo: &Path, opts: &Options, rows: &[Sample]) -> Result<Pipeline, String> {
    let candidate = Candidate::parse(&opts.model);
    let mut runner = Runner::start(
        repo,
        &runner::Options {
            candidate: candidate.clone(),
            cpu: opts.cpu,
            experiments_dir: opts.experiments_dir.clone(),
            models_dir: opts.models_dir.clone(),
            timeout_ms: opts.timeout_ms,
            engines_dir: opts.engines_dir.clone(),
        },
    )?;
    let mut scored = Vec::with_capacity(rows.len());
    let mut backend = String::new();
    for sample in rows {
        let mut runtime = runner.run_row(sample)?;
        // A row the skip heuristic answers without the model leaves the
        // engine unloaded, so the backend is read until a load shows.
        if backend.is_empty() || backend == "unknown" {
            backend = runner.backend();
        }
        runtime.backend = backend.clone();
        scored.push((sample.clone(), runtime));
    }
    for (_, runtime) in &mut scored {
        runtime.backend = backend.clone();
    }
    let engine = candidate
        .is_model()
        .then(|| runner.engine_binary(repo).ok())
        .flatten();
    // The raw generation runs while the profile (and the model file the
    // daemon linked into it) still exists.
    let engine_only = match (opts.engine_only, &engine, &runner.model_file) {
        (true, Some(engine), Some(file)) => {
            Some(run_engine_only(engine, Path::new(file), rows, opts.cpu)?)
        }
        (true, _, _) => return Err("--engine-only needs a real model (not a mock)".into()),
        _ => None,
    };
    // The profile (and the model file the daemon linked into it) goes
    // with the runner at the end of this function: the checksum is taken
    // now, or the report would hash a path that no longer exists.
    let model_sha256 = model_checksum(candidate.is_model(), runner.model_file.as_deref())?;
    Ok(Pipeline {
        scored,
        backend,
        model_file: runner.model_file.clone(),
        model_sha256,
        engine,
        served_id: runner.served_id(),
        engine_only,
    })
}

/// The model's SHA-256 for the report: a real candidate whose file
/// cannot be hashed is an evidence failure; a mock has none.
fn model_checksum(real: bool, model_file: Option<&str>) -> Result<Option<String>, String> {
    let hashed = model_file.and_then(|f| report::sha256(Path::new(f)));
    if real && hashed.is_none() {
        return Err(format!(
            "the model file {} could not be hashed; a report needs the candidate's checksum",
            model_file.unwrap_or("(none)")
        ));
    }
    Ok(hashed)
}

/// Runs the rows through the engine's CLI mode without the guards.
fn run_engine_only(
    engine: &Path,
    model: &Path,
    rows: &[Sample],
    cpu: bool,
) -> Result<EngineOnly, String> {
    let mut walls = Vec::with_capacity(rows.len());
    let mut rates = Vec::with_capacity(rows.len());
    let mut backend = String::new();
    for sample in rows {
        let started = Instant::now();
        let mut cmd = Command::new(engine);
        cmd.args([
            "--prompt",
            &sample.input,
            "--raw",
            "--json",
            "--max-tokens",
            "160",
        ])
        .arg("--model")
        .arg(model);
        if cpu {
            cmd.arg("--cpu");
        }
        let out = cmd
            .output()
            .map_err(|e| format!("{}: {e}", engine.display()))?;
        let wall = started.elapsed();
        if !out.status.success() {
            return Err(format!(
                "row {}: dettivo-engine-llm exited {}",
                sample.id,
                out.status.code().unwrap_or(-1)
            ));
        }
        let v: Value = serde_json::from_slice(&out.stdout)
            .map_err(|e| format!("row {}: engine output: {e}", sample.id))?;
        let tokens = v["tokens"].as_u64().unwrap_or(0) as f64;
        if backend.is_empty() {
            backend = v["backend"].as_str().unwrap_or("unknown").to_string();
        }
        let secs = wall.as_secs_f64().max(0.001);
        walls.push(wall.as_millis() as u64);
        rates.push(tokens / secs);
    }
    rates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_rate = if rates.is_empty() {
        0.0
    } else {
        rates[rates.len() / 2]
    };
    Ok(EngineOnly {
        rows: rows.len(),
        backend,
        tokens_per_second_median: (median_rate * 10.0).round() / 10.0,
        median_wall_ms: scorer::median(&walls),
        p95_wall_ms: scorer::percentile(&walls, 0.95),
    })
}

/// The metrics named in a golden file that differ from the run's.
pub fn golden_mismatches(
    golden: &Path,
    metrics: &BTreeMap<String, f64>,
) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(golden).map_err(|e| format!("{}: {e}", golden.display()))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", golden.display()))?;
    let want = v
        .get("metrics")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{}: no metrics object", golden.display()))?;
    let mut mismatches = Vec::new();
    for (name, value) in want {
        let Some(expected) = value.as_f64() else {
            continue;
        };
        match metrics.get(name) {
            Some(got) if (got - expected).abs() < 1e-9 => {}
            Some(got) => mismatches.push(format!("{name}: got {got}, golden {expected}")),
            None => mismatches.push(format!("{name}: missing from the run, golden {expected}")),
        }
    }
    Ok(mismatches)
}

/// Runs the eval.
pub fn run(repo: &Path, opts: &Options) -> Result<Outcome, String> {
    let rows = dataset::load(&opts.set, &opts.split)?;
    let targets = Targets::load(&opts.targets.clone().unwrap_or_else(|| repo.join(TARGETS)))?;
    let Pipeline {
        scored,
        backend,
        model_file,
        model_sha256,
        engine,
        served_id,
        engine_only,
    } = run_pipeline(repo, opts, &rows)?;
    if scored.len() != rows.len() {
        return Err(format!(
            "{} of {} rows ran; a report needs every row",
            scored.len(),
            rows.len()
        ));
    }
    if let Some(outputs) = &opts.outputs {
        let mut text = String::new();
        for (s, r) in &scored {
            text.push_str(&serde_json::json!({"id": s.id, "output": r.output, "model": r.model, "wall_ms": r.wall_ms, "guard_rejection": r.guard_rejection, "fallback_used": r.fallback_used}).to_string());
            text.push('\n');
        }
        std::fs::write(outputs, text).map_err(|e| format!("{}: {e}", outputs.display()))?;
    }
    let summary = scorer::summarize(&scored).ok_or("no rows scored")?;
    let metrics = summary.metrics.as_map();
    let comparison =
        compatibility::Comparison::capture(repo, opts, &rows, &targets, engine.as_deref())?;
    let inc = match &opts.incumbent {
        Some(path) => Some(incumbent(path, &comparison, &opts.split, &rows)?),
        None => None,
    };
    let decision = gate::check(&targets, &metrics, &backend, inc.as_ref());
    let candidate_id = served_id;
    let baseline_path = opts
        .baseline
        .clone()
        .unwrap_or_else(|| repo.join(REPORTS_DIR).join(BASELINE));
    let macos_baseline = if baseline_path.is_file() {
        let entry = opts
            .baseline_model
            .clone()
            .unwrap_or_else(|| candidate_id.clone());
        Baseline::load(&baseline_path)?.compare(&entry, &metrics)
    } else {
        None
    };
    let report = Report {
        comparison: Some(comparison),
        date: report::today(),
        model: candidate_id,
        model_sha256,
        model_file,
        backend,
        engine_version: report::engine_version(engine.as_deref()),
        git_sha: report::git_sha(repo),
        set: opts
            .set
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
        split: opts.split.clone(),
        rows: summary.count,
        gold_rows: summary.gold_rows,
        row_ids: rows.iter().map(|s| s.id.clone()).collect(),
        metrics: summary.metrics,
        cuts: summary.cuts,
        engine_only,
        gate: decision,
        macos_baseline,
    };
    let golden_mismatches = match &opts.golden {
        Some(g) => golden_mismatches(g, &metrics)?,
        None => Vec::new(),
    };
    let path = match &opts.out {
        Some(dir) => Some(report::write(dir, &report)?),
        None => None,
    };
    Ok(Outcome {
        report,
        path,
        golden_mismatches,
    })
}

/// The human rendering of an outcome.
pub fn human(outcome: &Outcome) -> String {
    let r = &outcome.report;
    let m = &r.metrics;
    let mut out = format!(
        "polish-eval  {} on {} ({} rows of {}, split {})\n",
        r.model, r.backend, r.rows, r.set, r.split
    );
    out.push_str(&format!(
        "metrics      exact {:.3}  protected {:.3}  required {:.3}  forbidden {:.3}  structure {:.3}  language {:.3}  guard {:.3}  fallback {:.3}  median {} ms  p95 {} ms\n",
        m.exact_match_rate,
        m.protected_token_preservation_rate,
        m.required_fragment_retention_rate,
        m.forbidden_fragment_violation_rate,
        m.structure_retention_rate,
        m.language_anchor_retention_rate,
        m.guard_rejection_rate,
        m.fallback_rate,
        m.median_wall_ms,
        m.p95_wall_ms
    ));
    for g in &r.gate.hard_gates {
        out.push_str(&format!(
            "gate         {:<36} {:<8} {}\n",
            g.metric,
            g.value
                .map(|v| format!("{v}"))
                .unwrap_or_else(|| "-".into()),
            if g.pass { "pass" } else { "FAIL" }
        ));
    }
    if let Some(rel) = &r.gate.relative {
        out.push_str(&format!(
            "incumbent    {} on {}: {} regression(s), improved {}\n",
            rel.incumbent_model,
            rel.incumbent_backend,
            rel.regressions.len(),
            if rel.improved_metrics.is_empty() {
                "nothing".to_string()
            } else {
                rel.improved_metrics.join(", ")
            }
        ));
    }
    if let Some(why) = &r.gate.relative_skipped {
        out.push_str(&format!("incumbent    not compared: {why}\n"));
    }
    if let Some(b) = &r.macos_baseline {
        for row in &b.rows {
            out.push_str(&format!(
                "macos        {:<36} linux {} macos {} {}\n",
                row.metric, row.linux, row.macos, row.verdict
            ));
        }
    }
    if let Some(e) = &r.engine_only {
        out.push_str(&format!(
            "engine-only  {} rows on {}: {} tokens/s median, {} ms median, {} ms p95\n",
            e.rows, e.backend, e.tokens_per_second_median, e.median_wall_ms, e.p95_wall_ms
        ));
    }
    for f in &r.gate.failures {
        out.push_str(&format!("failure      {f}\n"));
    }
    out.push_str(&format!(
        "absolute     {}\n",
        if r.gate.absolute_passed {
            "pass"
        } else {
            "fail"
        }
    ));
    for g in &outcome.golden_mismatches {
        out.push_str(&format!("golden       {g}\n"));
    }
    out.push_str(&format!(
        "verdict      {}{}\n",
        if r.gate.passed { "pass" } else { "fail" },
        outcome
            .path
            .as_ref()
            .map(|p| format!("  report {}", p.display()))
            .unwrap_or_default()
    ));
    out
}
