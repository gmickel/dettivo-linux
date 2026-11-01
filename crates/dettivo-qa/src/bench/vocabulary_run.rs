//! The run behind `dettivo-qa bench vocabulary`: every clip of the fixture
//! set through each speech engine's CLI mode, once with the vocabulary off
//! and once on, scored and written as one report. `--out <dir>` also files
//! it as `vocabulary-<date>-<host>.json` with a Markdown table beside it.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::vocabulary::{Entry, FIXTURE_DIR, Set, VocabularyScore, prompt_args, score};
use super::{Options, Status, date_of, host, throughput, unix_now};

/// The report's shape version.
pub const SCHEMA_VERSION: u32 = 1;
/// The speech engines measured, in report order.
pub const PROVIDERS: &[&str] = &["whisper", "parakeet"];

/// One clip's transcript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transcript {
    /// The clip id.
    pub id: String,
    /// What the engine returned.
    pub text: String,
}

/// One engine and model, with the vocabulary off or on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    /// The provider.
    pub provider: String,
    /// The model id, empty when none was found.
    pub model: String,
    /// The backend the engine reported.
    pub backend: String,
    /// Whether the engine was primed with the vocabulary.
    pub vocabulary: bool,
    /// Measured, skipped or failed.
    pub status: Status,
    /// Why the row was skipped or failed.
    pub reason: Option<String>,
    /// The score of a measured row.
    pub score: Option<VocabularyScore>,
    /// Every clip's transcript.
    pub transcripts: Vec<Transcript>,
    /// Wall time of the pass, model loads included.
    pub wall_ms: u64,
}

impl Row {
    fn without(provider: &str, model: &str, vocabulary: bool, status: Status, why: String) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            backend: String::new(),
            vocabulary,
            status,
            reason: Some(why),
            score: None,
            transcripts: Vec::new(),
            wall_ms: 0,
        }
    }
}

/// `vocabulary-report.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VocabularyReport {
    /// [`SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Unix time the run started.
    pub generated_unix: u64,
    /// The date, `YYYY-MM-DD`.
    pub date: String,
    /// The commit the run measured.
    pub git_sha: String,
    /// The host name.
    pub hostname: String,
    /// `--cpu`.
    pub forced_cpu: bool,
    /// SHA-256 of `set.json`.
    pub fixture_sha256: String,
    /// The vocabulary the passes with it on were primed with.
    pub vocabulary: Vec<Entry>,
    /// Every row.
    pub rows: Vec<Row>,
}

/// One pass of every clip through `binary`.
fn pass(
    opts: &Options,
    set: &Set,
    binary: &std::path::Path,
    model: &host::ModelRef,
    provider: &str,
    on: bool,
) -> Row {
    let prompt = if on {
        match prompt_args(provider, &set.terms()) {
            Some(args) => args,
            None => {
                let why = format!("dettivo-engine-{provider} takes no vocabulary");
                return Row::without(provider, &model.id, on, Status::Skipped, why);
            }
        }
    } else {
        Vec::new()
    };
    let dir = opts.repo_root.join(FIXTURE_DIR);
    let model_arg = model.path.to_string_lossy().into_owned();
    let mut row = Row::without(provider, &model.id, on, Status::Measured, String::new());
    row.reason = None;
    for clip in &set.clips {
        let wav = dir
            .join(format!("{}.wav", clip.id))
            .to_string_lossy()
            .into_owned();
        let mut args = vec!["--wav", wav.as_str(), "--model", model_arg.as_str()];
        args.extend(prompt.iter().map(String::as_str));
        match throughput::cli(binary, &args, opts.cpu) {
            Ok(run) => {
                row.wall_ms += run.wall_ms;
                row.backend = run.result["backend"].as_str().unwrap_or("").to_string();
                let text = run.result["text"].as_str().unwrap_or("").trim().to_string();
                row.transcripts.push(Transcript {
                    id: clip.id.clone(),
                    text,
                });
            }
            Err(e) => {
                row.status = Status::Failed;
                row.reason = Some(format!("{}: {e}", clip.id));
                return row;
            }
        }
    }
    let heard: Vec<(String, String)> = row
        .transcripts
        .iter()
        .map(|t| (t.id.clone(), t.text.clone()))
        .collect();
    row.score = Some(score(set, &heard));
    row
}

/// Runs every pass and writes the report; the report's path.
pub fn run(opts: &Options) -> Result<(VocabularyReport, PathBuf), String> {
    let started = unix_now();
    let run_dir = crate::evidence::Evidence::run_for_process(&opts.evidence_base)
        .map(|e| e.run_dir.join("bench-vocabulary"))
        .map_err(|e| format!("evidence: {e}"))?;
    std::fs::create_dir_all(&run_dir).map_err(|e| format!("{}: {e}", run_dir.display()))?;
    let fixture_dir = opts.repo_root.join(FIXTURE_DIR);
    let set = Set::load(&fixture_dir)?;
    let engines = host::engine_binaries(&opts.repo_root, opts.engines_dir.as_deref());
    let models = host::Models::resolve(&opts.repo_root, opts.models_dir.as_deref())?;
    let mut rows = Vec::new();
    for provider in PROVIDERS {
        let binary_name = format!("dettivo-engine-{provider}");
        let binary = engines.iter().find(|e| e.binary == binary_name);
        for on in [false, true] {
            let row = match (models.get(provider), binary) {
                (Err(why), _) => Row::without(provider, "", on, Status::Skipped, why),
                (Ok(model), None) => Row::without(
                    provider,
                    &model.id,
                    on,
                    Status::Skipped,
                    format!("{binary_name} is not built"),
                ),
                (Ok(model), Some(b)) => pass(opts, &set, &b.path, model, provider, on),
            };
            eprintln!(
                "bench vocabulary: {provider} vocabulary={on} {:?}",
                row.status
            );
            rows.push(row);
        }
    }
    let report = VocabularyReport {
        schema_version: SCHEMA_VERSION,
        generated_unix: started,
        date: date_of(started),
        git_sha: host::git_sha(&opts.repo_root),
        hostname: host::Host::detect().hostname,
        forced_cpu: opts.cpu,
        fixture_sha256: host::fixture_ref("set.json", &fixture_dir.join("set.json"))
            .map(|f| f.sha256)
            .unwrap_or_default(),
        vocabulary: set.vocabulary.clone(),
        rows,
    };
    let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())? + "\n";
    let path = run_dir.join("vocabulary-report.json");
    std::fs::write(&path, &json).map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some(out) = &opts.out {
        std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
        // Not `-gpu.json` or `-cpu.json`: the NFR README reads only those.
        for (ext, text) in [("json", json.clone()), ("md", markdown(&report))] {
            let name = format!("vocabulary-{}-{}.{ext}", report.date, report.hostname);
            let filed = out.join(name);
            std::fs::write(&filed, text).map_err(|e| format!("{}: {e}", filed.display()))?;
        }
    }
    Ok((report, path))
}

/// The table a person reads: one line per row.
pub fn markdown(report: &VocabularyReport) -> String {
    let mut out = format!(
        "# Vocabulary term recall, {} on {}\n\n`dettivo-qa bench vocabulary` at `{}`{}. Term recall counts target clips whose term came back spelt exactly; false insertions count terms in distractor and guard clips; guard WER is the word error rate over the guard clips.\n\n| Engine | Model | Backend | Vocabulary | Term recall | False insertions | Guard WER |\n|---|---|---|---|---|---|---|\n",
        report.date,
        report.hostname,
        report.git_sha.get(..12).unwrap_or(&report.git_sha),
        if report.forced_cpu {
            ", CPU forced"
        } else {
            ""
        },
    );
    for row in &report.rows {
        let on = if row.vocabulary { "on" } else { "off" };
        let cells = match (&row.score, &row.reason) {
            (Some(s), _) => format!(
                "{}/{} | {} | {:.3}",
                s.term_recall.0, s.term_recall.1, s.false_insertions, s.guard_wer
            ),
            (None, why) => {
                format!("{:?}: {} | - | -", row.status, why.as_deref().unwrap_or("")).to_lowercase()
            }
        };
        out.push_str(&format!(
            "| {} | {} | {} | {on} | {cells} |\n",
            row.provider, row.model, row.backend
        ));
    }
    out
}

/// `dettivo-qa bench vocabulary`: exits 0 unless a pass failed (1) or
/// the run could not start (2).
pub fn command(opts: &Options, json: bool) -> u8 {
    match run(opts) {
        Ok((report, path)) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                );
            } else {
                print!("{}", markdown(&report));
                println!("\n  report: {}", path.display());
            }
            u8::from(report.rows.iter().any(|r| r.status == Status::Failed))
        }
        Err(e) => {
            eprintln!("bench vocabulary: {e}");
            2
        }
    }
}
