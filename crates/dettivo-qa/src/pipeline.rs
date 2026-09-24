//! `dettivo-qa pipeline parakeet-alignment`: both engines' CLI modes over
//! the golden-aligned fixture, per-word and segment-boundary precision,
//! the rule from `alignment`, and `alignment-report.json` with the
//! verdict `system.capabilities.speech.providers[].meeting_capable` and
//! `docs/engines.md` carry (ADR 0018).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::alignment::{self, Golden, Offsets, Span, WordPrecision};
use crate::scenarios;

/// The pipelines this verb runs, in the order the usage names them.
pub const PIPELINES: &[&str] = &["parakeet-alignment", "import-merge", "diarization"];

/// The golden fixture, relative to the repository root.
pub const GOLDEN: &str = "crates/dettivo-qa/fixtures/alignment/jfk.words.json";

/// One engine and model measured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Run {
    /// `whisper` or `parakeet`.
    pub provider: String,
    /// The model id.
    pub model: String,
    /// The backend the engine reported.
    pub backend: String,
    /// Why that backend (the engine's `model loaded` log line).
    pub reason: String,
    /// Word precision (Whisper reports no words; zeros then).
    pub words: WordPrecision,
    /// Segment boundary precision.
    pub segment_boundaries: Offsets,
    /// The transcript the engine returned.
    pub text: String,
    /// The verdict for this run under the rule (`null` for Whisper).
    pub passes: Option<bool>,
}

/// The report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// The golden fixture and how it was made.
    pub golden: String,
    /// The aligner.
    pub aligner: String,
    /// The rule, in words, fixed before the run.
    pub rule: String,
    /// Every run.
    pub runs: Vec<Run>,
    /// `meeting_capable`, `dictation_only` or `undecided`.
    pub verdict: String,
    /// Why (a missing fixture, engine or model names itself).
    pub verdict_reason: String,
}

fn models_dir() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let dir = data.join("dettivo/models");
    dir.is_dir().then_some(dir)
}

fn rule() -> String {
    format!(
        "meeting_capable when every Parakeet model measured pairs at least {}% of the golden words and keeps the pooled start/end offset p95 within {} ms and the median within {} ms; dictation_only otherwise; undecided when the fixture, an engine or a model is unavailable",
        (alignment::MIN_MATCHED_SHARE * 100.0) as u32,
        alignment::P95_TOLERANCE_MS,
        alignment::MEDIAN_TOLERANCE_MS
    )
}

/// Runs the spike; `force_cpu` pins both engines to the CPU.
pub fn parakeet_alignment(repo: &Path, force_cpu: bool) -> Result<Report, String> {
    let golden_path = repo.join(GOLDEN);
    let golden: Golden = std::fs::read_to_string(&golden_path)
        .map_err(|e| format!("{}: {e}", golden_path.display()))
        .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))?;
    let mut report = Report {
        golden: GOLDEN.into(),
        aligner: golden.aligner.clone(),
        rule: rule(),
        runs: Vec::new(),
        verdict: "undecided".into(),
        verdict_reason: String::new(),
    };
    let Some(models) = models_dir() else {
        report.verdict_reason = "no models directory (scripts/models/fetch-test-model.sh)".into();
        return Ok(report);
    };
    let wav = models.join(&golden.fixture);
    if !wav.is_file() {
        report.verdict_reason = format!("fixture {} is missing", wav.display());
        return Ok(report);
    }
    let candidates = [
        (
            "whisper",
            "tiny.en",
            models.join("whisper/tiny.en/ggml-tiny.en.bin"),
        ),
        (
            "parakeet",
            "parakeet-v2",
            models.join("parakeet/parakeet-v2/tdt-0.6b-v2-q8_0.gguf"),
        ),
        (
            "parakeet",
            "parakeet-v3",
            models.join("parakeet/parakeet-v3/tdt-0.6b-v3-q8_0.gguf"),
        ),
    ];
    let mut missing = Vec::new();
    for (provider, model, path) in candidates {
        if !path.is_file() {
            missing.push(format!("{provider}/{model}"));
            continue;
        }
        let binary = match scenarios::binary(repo, &format!("dettivo-engine-{provider}")) {
            Ok(b) => b,
            Err(e) => {
                missing.push(e);
                continue;
            }
        };
        report.runs.push(run(
            &binary, provider, model, &path, &wav, &golden, force_cpu,
        )?);
    }
    let parakeet_runs: Vec<&Run> = report
        .runs
        .iter()
        .filter(|r| r.provider == "parakeet")
        .collect();
    let v2_missing = missing.iter().any(|m| m.contains("parakeet-v2"))
        || missing
            .iter()
            .any(|m| m.contains("dettivo-engine-parakeet"));
    if parakeet_runs.is_empty() || v2_missing {
        report.verdict_reason = format!("unavailable: {}", missing.join(", "));
    } else if parakeet_runs.iter().all(|r| r.passes == Some(true)) {
        report.verdict = "meeting_capable".into();
        report.verdict_reason = format!(
            "every Parakeet run met the rule{}",
            if missing.is_empty() {
                String::new()
            } else {
                format!(" (not measured: {})", missing.join(", "))
            }
        );
    } else {
        report.verdict = "dictation_only".into();
        report.verdict_reason = parakeet_runs
            .iter()
            .filter(|r| r.passes != Some(true))
            .map(|r| {
                format!(
                    "{}: matched {}/{}, median {} ms, p95 {} ms",
                    r.model,
                    r.words.matched_words,
                    r.words.golden_words,
                    r.words.combined.median_ms,
                    r.words.combined.p95_ms
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
    }
    Ok(report)
}

fn run(
    binary: &Path,
    provider: &str,
    model: &str,
    model_path: &Path,
    wav: &Path,
    golden: &Golden,
    force_cpu: bool,
) -> Result<Run, String> {
    let mut cmd = Command::new(binary);
    cmd.arg("--wav")
        .arg(wav)
        .arg("--model")
        .arg(model_path)
        .arg("--json");
    if force_cpu {
        cmd.arg("--cpu");
    }
    let out = cmd
        .output()
        .map_err(|e| format!("{}: {e}", binary.display()))?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(format!("{provider}/{model}: {}", stderr.trim()));
    }
    let result: Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("{provider}/{model}: engine JSON: {e}"))?;
    let reason = stderr
        .lines()
        .find(|l| l.contains("model loaded"))
        .and_then(|l| {
            l.split_once("model loaded")
                .map(|(_, rest)| rest.trim().to_string())
        })
        .unwrap_or_default();
    let mut words = Vec::new();
    let mut segments = Vec::new();
    for s in result["segments"].as_array().into_iter().flatten() {
        segments.push(Span {
            text: String::new(),
            start_ms: s["start_ms"].as_u64().unwrap_or(0),
            end_ms: s["end_ms"].as_u64().unwrap_or(0),
        });
        for w in s["words"].as_array().into_iter().flatten() {
            words.push(Span {
                text: w["text"].as_str().unwrap_or("").to_string(),
                start_ms: w["start_ms"].as_u64().unwrap_or(0),
                end_ms: w["end_ms"].as_u64().unwrap_or(0),
            });
        }
    }
    let precision = alignment::word_precision(&golden.words, &words);
    let passes = (provider == "parakeet").then(|| alignment::passes(&precision));
    Ok(Run {
        provider: provider.into(),
        model: model.into(),
        backend: result["backend"].as_str().unwrap_or("").to_string(),
        reason,
        words: precision,
        segment_boundaries: alignment::boundary_precision(&golden.words, &segments),
        text: result["text"].as_str().unwrap_or("").to_string(),
        passes,
    })
}

/// Human rendering.
pub fn human(report: &Report) -> String {
    let mut out = format!(
        "parakeet-alignment: {} ({})\n",
        report.verdict, report.verdict_reason
    );
    for r in &report.runs {
        out.push_str(&format!(
            "  {:<9} {:<12} {:<7} words {}/{} median {} ms p95 {} ms max {} ms | segments median {} ms p95 {} ms{}\n",
            r.provider,
            r.model,
            r.backend,
            r.words.matched_words,
            r.words.golden_words,
            r.words.combined.median_ms,
            r.words.combined.p95_ms,
            r.words.combined.max_ms,
            r.segment_boundaries.median_ms,
            r.segment_boundaries.p95_ms,
            match r.passes {
                Some(true) => " PASS",
                Some(false) => " FAIL",
                None => "",
            }
        ));
    }
    out
}

/// The `pipeline` subcommand: runs `name`, files the report under
/// `evidence`, prints it, and mirrors it to `report_path` when asked.
#[allow(clippy::too_many_arguments)]
pub fn run_cli(
    json_out: bool,
    repo: &Path,
    name: &str,
    cpu: bool,
    bench: bool,
    evidence: &Path,
    report_path: Option<&Path>,
) -> u8 {
    if name == "import-merge" {
        return import_merge_cli(json_out, repo, cpu, evidence);
    }
    if name == "diarization" {
        return crate::pipeline_diarize::run_cli(json_out, repo, bench, evidence);
    }
    if name != "parakeet-alignment" {
        eprintln!(
            "pipeline: unknown pipeline {name:?}; the ones available are {}",
            PIPELINES.join(", ")
        );
        return 4;
    }
    let report = match parakeet_alignment(repo, cpu) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("pipeline: {e}");
            return 1;
        }
    };
    let json = serde_json::to_string_pretty(&report).unwrap_or_default() + "\n";
    let written = crate::evidence::Evidence::run_for_process(evidence).and_then(|run| {
        let dir = run.run_dir.join(name);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("alignment-report.json"), &json)?;
        Ok(dir)
    });
    let dir = match written {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("pipeline: evidence: {e}");
            return 1;
        }
    };
    if let Some(path) = report_path {
        if let Err(e) = std::fs::write(path, &json) {
            eprintln!("pipeline: {}: {e}", path.display());
            return 1;
        }
    }
    if json_out {
        print!("{json}");
    } else {
        print!("{}", human(&report));
        println!("  report: {}", dir.join("alignment-report.json").display());
    }
    u8::from(report.verdict == "undecided" && report.runs.is_empty())
}

/// `pipeline import-merge`: the report lands under `evidence` with the
/// chunk files; the exit code is 1 on `fail`, 0 on `pass` or `skipped`.
fn import_merge_cli(json_out: bool, repo: &Path, cpu: bool, evidence: &Path) -> u8 {
    let dir = match crate::evidence::Evidence::run_for_process(evidence).and_then(|run| {
        let dir = run.run_dir.join("import-merge");
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }) {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("pipeline: evidence: {e}");
            return 1;
        }
    };
    let report = match crate::pipeline_import::import_merge(repo, &dir, cpu) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("pipeline: {e}");
            return 1;
        }
    };
    let json = serde_json::to_string_pretty(&report).unwrap_or_default() + "\n";
    if let Err(e) = std::fs::write(dir.join("import-merge-report.json"), &json) {
        eprintln!("pipeline: {}: {e}", dir.display());
        return 1;
    }
    if json_out {
        print!("{json}");
    } else {
        print!("{}", crate::pipeline_import::human(&report));
        println!(
            "  report: {}",
            dir.join("import-merge-report.json").display()
        );
    }
    u8::from(report.verdict == "fail")
}
