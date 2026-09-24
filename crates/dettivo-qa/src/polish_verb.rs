//! `dettivo-qa polish-eval`: the arguments and the run of the polish
//! evaluation harness (ADR 0032), kept beside `main.rs` so the command
//! line stays under the file limit.

use std::path::{Path, PathBuf};

use clap::Args;
use dettivo_qa::polish_eval;

/// Run a held-out polish set through the Enhanced pipeline against a
/// candidate model and score it with the macOS metrics and hard gates.
#[derive(Debug, Args)]
pub struct PolishEvalArgs {
    /// The set (macOS canonical JSONL).
    #[arg(long, value_name = "FILE")]
    set: PathBuf,
    /// A catalogue id, `sideload[:<manifest>]`, a GGUF path, `fixture:<dir>` or `echo`.
    #[arg(long)]
    model: String,
    /// The split to run (`all` for every row).
    #[arg(long, default_value = "heldout")]
    split: String,
    /// An incumbent report for the relative gates.
    #[arg(long, value_name = "FILE")]
    incumbent: Option<PathBuf>,
    /// Also measure raw generation through the engine's CLI mode.
    #[arg(long)]
    engine_only: bool,
    /// Where the report goes (`docs/reports/polish-eval` by default; `none` skips writing).
    #[arg(long, value_name = "DIR")]
    out: Option<PathBuf>,
    /// A golden file whose metrics the run must match.
    #[arg(long, value_name = "FILE")]
    golden: Option<PathBuf>,
    /// Pin the engine to the CPU.
    #[arg(long)]
    cpu: bool,
    /// The experiments directory a sideload reads.
    #[arg(long, value_name = "DIR")]
    experiments_dir: Option<PathBuf>,
    /// Write every row's output to this file (outside the repository).
    #[arg(long, value_name = "FILE")]
    outputs: Option<PathBuf>,
    /// The targets file (`qa/polish-eval/targets.json` by default).
    #[arg(long, value_name = "FILE")]
    targets: Option<PathBuf>,
    /// The baseline entry to compare with (the model id by default).
    #[arg(long)]
    baseline_model: Option<String>,
    /// `[llm] timeout_ms` for the run.
    #[arg(long, default_value_t = 60000)]
    timeout_ms: u64,
    /// The directory holding dettivo-engine-llm (a Vulkan build kept beside the workspace build).
    #[arg(long, value_name = "DIR")]
    engines_dir: Option<PathBuf>,
}

/// The exit code: 0 when the run matches its golden (or has none), 1 on a
/// golden mismatch, 2 when the run fails.
pub fn run(json: bool, repo: &Path, args: &PolishEvalArgs, models_dir: Option<PathBuf>) -> u8 {
    let out = match &args.out {
        Some(p) if p.as_os_str() == "none" => None,
        Some(p) => Some(p.clone()),
        None => Some(repo.join(polish_eval::REPORTS_DIR)),
    };
    match polish_eval::run(
        repo,
        &polish_eval::Options {
            set: args.set.clone(),
            model: args.model.clone(),
            split: args.split.clone(),
            incumbent: args.incumbent.clone(),
            engine_only: args.engine_only,
            out,
            golden: args.golden.clone(),
            cpu: args.cpu,
            experiments_dir: args.experiments_dir.clone(),
            models_dir,
            outputs: args.outputs.clone(),
            targets: args.targets.clone(),
            baseline: None,
            baseline_model: args.baseline_model.clone(),
            timeout_ms: args.timeout_ms,
            engines_dir: args.engines_dir.clone(),
        },
    ) {
        Ok(outcome) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&outcome.report).unwrap_or_default()
                );
            } else {
                print!("{}", polish_eval::human(&outcome));
            }
            u8::from(!outcome.golden_mismatches.is_empty())
        }
        Err(e) => {
            eprintln!("polish-eval: {e}");
            2
        }
    }
}
