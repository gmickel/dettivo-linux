//! `dettivo-qa bench` (S-16, ADR 0029): the NFR measurements from the same
//! fixtures the tests use, through the engines' CLI modes and the
//! daemon's dictation path, written as one report per host and tier with
//! every number beside its target. The steps: `first_insert` (NFR-1 or
//! NFR-2 by tier), `stt_throughput` (NFR-4 or NFR-5), `llm_rewrite`
//! (recorded), `idle_footprint` (NFR-6), `startup` (NFR-8) and the
//! `diarization` placeholder. `--cpu` forces the CPU tier, `--quick` runs
//! three iterations for CI, `--out <dir>` also files the report as
//! `<date>-<host>-<tier>.json` there and renders the README table.

pub mod daemon;
pub mod first_insert;
pub mod footprint;
pub mod host;
pub mod meetings;
pub mod render;
pub mod report;
pub mod throughput;

pub use report::{
    Comparison, LoadAverage, Report, SCHEMA_VERSION, Spread, Status, Step, date_of, load_average,
};

use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::nfr;

/// The steps, in run order.
pub const STEPS: &[&str] = &[
    "first_insert",
    "stt_throughput",
    "llm_rewrite",
    "idle_footprint",
    "startup",
];
/// Iterations of a timed step on a full run, and under `--quick`.
pub const ITERATIONS: usize = 10;
/// Iterations under `--quick`.
pub const QUICK_ITERATIONS: usize = 3;

/// How a bench run is configured.
#[derive(Debug, Clone)]
pub struct Options {
    /// The repository root.
    pub repo_root: PathBuf,
    /// Force the CPU tier (`DETTIVO_FORCE_CPU=1` for the engines).
    pub cpu: bool,
    /// Three iterations and shorter idles, for CI.
    pub quick: bool,
    /// One step by name, or every step.
    pub step: Option<String>,
    /// Where the report is also filed as `<date>-<host>-<tier>.json`
    /// with the README table.
    pub out: Option<PathBuf>,
    /// The evidence base directory.
    pub evidence_base: PathBuf,
    /// The engine binaries (a Vulkan build); the workspace build otherwise.
    pub engines_dir: Option<PathBuf>,
    /// The real model directory, when it exists.
    pub models_dir: Option<PathBuf>,
}

impl Options {
    /// Iterations for this run.
    pub fn iterations(&self) -> usize {
        if self.quick {
            QUICK_ITERATIONS
        } else {
            ITERATIONS
        }
    }
}

/// What the steps share.
pub struct Bench {
    /// The options.
    pub opts: Options,
    /// The run directory the evidence lands in.
    pub run_dir: PathBuf,
    /// The models resolved per provider.
    pub models: host::Models,
    /// The engine binaries.
    pub engines: Vec<host::EngineBinary>,
    /// The speech fixture.
    pub fixture: PathBuf,
}

impl Bench {
    /// Iterations for this run.
    pub fn iterations(&self) -> usize {
        self.opts.iterations()
    }

    /// The engine binary for `name`.
    pub fn engine(&self, name: &str) -> Option<&Path> {
        self.engines
            .iter()
            .find(|e| e.binary == name)
            .map(|e| e.path.as_path())
    }

    /// The directory the daemon looks for engines in.
    pub fn engines_dir(&self) -> Option<PathBuf> {
        self.engines
            .first()
            .and_then(|e| e.path.parent().map(Path::to_path_buf))
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Runs the suite and writes the report; the exit code: 0 unless a step
/// failed (a skip, a placeholder and a missed target all exit 0: the
/// numbers are recorded, never gated).
pub fn run(opts: &Options) -> Result<(Report, PathBuf), String> {
    let started = unix_now();
    let run_dir = crate::evidence::Evidence::run_for_process(&opts.evidence_base)
        .map(|e| e.run_dir.join("bench"))
        .map_err(|e| format!("evidence: {e}"))?;
    std::fs::create_dir_all(&run_dir).map_err(|e| format!("{}: {e}", run_dir.display()))?;
    let engines = host::engine_binaries(&opts.repo_root, opts.engines_dir.as_deref());
    let models = host::Models::resolve(&opts.repo_root, opts.models_dir.as_deref())?;
    let fixture = opts
        .models_dir
        .as_ref()
        .map(|m| m.join("fixtures/jfk.wav"))
        .unwrap_or_default();
    let bench = Bench {
        opts: opts.clone(),
        run_dir: run_dir.clone(),
        models,
        engines,
        fixture,
    };
    let load_at_start = load_average();
    let (tier, tier_reason) = daemon::probe_tier(&bench)?;
    let mut steps = Vec::new();
    let mut tier = tier;
    let mut tier_reason = tier_reason;
    for name in STEPS {
        if opts.step.as_deref().is_some_and(|s| s != *name) {
            continue;
        }
        let at = Instant::now();
        let mut step = match *name {
            "first_insert" => {
                let (step, seen) = first_insert::run(&bench, tier);
                if let Some((t, why)) = seen {
                    tier = t;
                    tier_reason = why;
                }
                step
            }
            "stt_throughput" => throughput::stt(&bench, tier),
            "llm_rewrite" => throughput::llm(&bench),
            "idle_footprint" => footprint::idle(&bench),
            "startup" => footprint::startup(&bench),
            other => Step::without(other, Status::Failed, "not a bench step".into()),
        };
        step.duration_ms = u64::try_from(at.elapsed().as_millis()).unwrap_or(u64::MAX);
        eprintln!(
            "bench: {} {:?}{}",
            step.name,
            step.status,
            step.reason
                .as_deref()
                .map(|r| format!(": {r}"))
                .unwrap_or_default()
        );
        steps.push(step);
    }
    let mut notes = vec![
        "the insert leg of first_insert runs through the mock inserter; the dictation pack's first_insert_timing measures the real chain".into(),
    ];
    if opts.cpu {
        notes.push(
            "the CPU run is --cpu on the GPU desktop until a CPU-only VM exists (S-16 boundary)"
                .into(),
        );
    }
    let host = host::Host::detect();
    let report = Report {
        schema_version: SCHEMA_VERSION,
        generated_unix: started,
        date: date_of(started),
        git_sha: host::git_sha(&opts.repo_root),
        quick: opts.quick,
        iterations: opts.iterations(),
        tier,
        tier_reason,
        forced_cpu: opts.cpu,
        notes,
        fixtures: host::fixtures(&bench),
        engines: bench.engines.clone(),
        models: bench.models.refs(),
        host,
        load_average: LoadAverage {
            start: load_at_start,
            end: load_average(),
        },
        steps,
        targets: nfr::all(),
        meetings: None,
    };
    let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())? + "\n";
    let path = run_dir.join("bench-report.json");
    std::fs::write(&path, &json).map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some(out) = &opts.out {
        std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
        let filed = out.join(format!(
            "{}-{}-{}.json",
            report.date,
            report.host.hostname,
            report.tier.as_str()
        ));
        std::fs::write(&filed, &json).map_err(|e| format!("{}: {e}", filed.display()))?;
        let readme = render::readme(out)?;
        std::fs::write(out.join("README.md"), readme)
            .map_err(|e| format!("{}: {e}", out.display()))?;
    }
    Ok((report, path))
}

/// `dettivo-qa bench`: runs, prints (JSON or human) and exits 0 unless a
/// step failed or the suite could not start (2).
pub fn command(opts: &Options, json: bool) -> u8 {
    if let Some(step) = &opts.step {
        if !STEPS.contains(&step.as_str()) {
            eprintln!(
                "bench: unknown step {step:?}; the steps are: {}",
                STEPS.join(", ")
            );
            return 2;
        }
    }
    match run(opts) {
        Ok((report, path)) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                );
            } else {
                print!("{}", render::human(&report));
                println!("  report: {}", path.display());
            }
            u8::from(report.steps.iter().any(|s| s.status == Status::Failed))
        }
        Err(e) => {
            eprintln!("bench: {e}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_runs_three_iterations() {
        let opts = Options {
            repo_root: PathBuf::new(),
            cpu: true,
            quick: true,
            step: None,
            out: None,
            evidence_base: PathBuf::new(),
            engines_dir: None,
            models_dir: None,
        };
        assert_eq!(opts.iterations(), 3);
        assert_eq!(
            Options {
                quick: false,
                ..opts
            }
            .iterations(),
            10
        );
    }
}
