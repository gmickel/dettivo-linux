//! `dettivo-qa bench`: the arguments and the run of the benchmark suites
//! (ADR 0029, ADR 0087, ADR 0102), kept beside `main.rs` so the command
//! line stays under the file limit.

use std::path::{Path, PathBuf};

use clap::Args;
use dettivo_qa::bench;

/// Measure the NFRs (first insert, transcription throughput, the
/// language model, the idle footprint, startup) into one report per
/// host and tier (docs/qa.md, ADR 0029); `bench vocabulary` scores
/// term recall with the vocabulary off and on (ADR 0087), `bench
/// windowed` the cost of decoding in rounds (ADR 0102).
#[derive(Debug, Args)]
pub struct BenchArgs {
    /// `vocabulary` or `windowed` for those suites; the NFR suite otherwise.
    #[arg(value_name = "SUITE")]
    suite: Option<String>,
    /// Force the CPU tier (`DETTIVO_FORCE_CPU=1` for every engine).
    #[arg(long)]
    cpu: bool,
    /// Three iterations and shorter idles, for CI.
    #[arg(long)]
    quick: bool,
    /// One step by name (`first_insert`, `warm_up`, `gpu_threads`,
    /// `stt_throughput`, `llm_rewrite`, `idle_footprint`, `startup`,
    /// `language_wer`).
    #[arg(long, value_name = "NAME")]
    step: Option<String>,
    /// Also file the report as `<date>-<host>-<tier>.json` in this
    /// directory and render its README table (docs/reports/benchmarks).
    #[arg(long, value_name = "DIR")]
    out: Option<PathBuf>,
    /// The engine binaries to measure (a Vulkan build); the workspace
    /// build otherwise.
    #[arg(long, value_name = "DIR")]
    engines: Option<PathBuf>,
    /// The FLEURS languages `language_wer` scores (scripts/qa/fetch-fleurs.sh).
    #[arg(long, value_name = "DIR")]
    languages: Option<PathBuf>,
    /// `bench vocabulary` only: prime the engines with this vocabulary, a
    /// JSON list of `{ term, sounds_like }` entries, instead of the set's own.
    #[arg(long, value_name = "FILE")]
    vocabulary: Option<PathBuf>,
    /// Evidence base directory; the report lands under `<dir>/<run>/bench/`.
    #[arg(long, default_value = "qa-evidence", value_name = "DIR")]
    evidence: PathBuf,
}

/// Runs the suite the arguments name; the process exit code.
pub fn run(args: &BenchArgs, repo: &Path, models_dir: Option<PathBuf>, json: bool) -> u8 {
    let opts = bench::Options {
        repo_root: repo.to_path_buf(),
        cpu: args.cpu,
        quick: args.quick,
        step: args.step.clone(),
        out: args.out.clone(),
        evidence_base: args.evidence.clone(),
        engines_dir: args.engines.clone(),
        models_dir,
        languages_dir: args.languages.clone(),
        vocabulary: args.vocabulary.clone(),
    };
    match args.suite.as_deref() {
        None => bench::command(&opts, json),
        Some(name) => bench::suite(name, &opts, json),
    }
}
