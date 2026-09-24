//! The report `docs/reports/polish-eval/<date>-<model>-<set>.json`
//! carries: the metrics and the cuts, the model id with its file and
//! checksum, the backend, the engine version, the row count and ids, the
//! git sha, the gate decision and the comparison with the macOS baseline.
//! It never carries a row's text; a test scans the directory for that.
//! `README.md` beside the reports is regenerated from them: the latest
//! run per model and set with its gate result.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::gate::Decision;
use super::scorer::{Cut, Metrics};

/// The `--engine-only` figure: the rows through `dettivo-engine-llm
/// --prompt --raw`, no guards, tokens per second.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineOnly {
    /// Rows generated.
    pub rows: usize,
    /// The backend the engine chose.
    pub backend: String,
    /// The median tokens per second over the rows.
    pub tokens_per_second_median: f64,
    /// The median wall time of one generation.
    pub median_wall_ms: u64,
    /// The p95 wall time.
    pub p95_wall_ms: u64,
}

/// One metric of the macOS baseline set against the Linux value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaselineRow {
    /// The metric.
    pub metric: String,
    /// The Linux value.
    pub linux: f64,
    /// The macOS value.
    pub macos: f64,
    /// `pass` when Linux matches or beats macOS on a quality metric;
    /// `informational` for a latency metric, which a different runtime
    /// does not share.
    pub verdict: String,
}

/// The comparison with `macos-baseline.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaselineComparison {
    /// The baseline entry compared against (a model id).
    pub entry: String,
    /// Where the macOS numbers came from.
    pub source: String,
    /// The rows.
    pub rows: Vec<BaselineRow>,
    /// Every quality row passed.
    pub quality_matched: bool,
}

/// The report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Inputs, scoring rules and runtime fingerprints for relative promotion.
    /// Older reports remain readable but cannot establish comparability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison: Option<super::compatibility::Comparison>,
    /// `YYYY-MM-DD` of the run.
    pub date: String,
    /// The candidate id.
    pub model: String,
    /// The file the engine loaded, when a real one ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_file: Option<String>,
    /// Its SHA-256.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_sha256: Option<String>,
    /// The engine backend.
    pub backend: String,
    /// `dettivo-engine-llm --version`.
    pub engine_version: String,
    /// The repository commit.
    pub git_sha: String,
    /// The set's file name.
    pub set: String,
    /// The split run.
    pub split: String,
    /// Rows run.
    pub rows: usize,
    /// Rows with a gold rewrite.
    pub gold_rows: usize,
    /// Their ids, in order.
    pub row_ids: Vec<String>,
    /// The metrics.
    pub metrics: Metrics,
    /// The cuts by preset and by language.
    pub cuts: BTreeMap<String, BTreeMap<String, Cut>>,
    /// The raw generation figure, with `--engine-only`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_only: Option<EngineOnly>,
    /// The gate.
    pub gate: Decision,
    /// The macOS comparison, when the baseline file names the model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macos_baseline: Option<BaselineComparison>,
}

/// `macos-baseline.json`: metrics per model id, no rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    /// Where the numbers come from.
    pub source: String,
    /// The runtime they were measured through.
    pub runtime: String,
    /// Model id to the metrics recorded for it.
    pub models: BTreeMap<String, BTreeMap<String, f64>>,
}

impl Baseline {
    /// Reads the file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Compares the Linux metrics with the entry `model`, or `None` when
    /// the file has no entry for it.
    pub fn compare(
        &self,
        model: &str,
        linux: &BTreeMap<String, f64>,
    ) -> Option<BaselineComparison> {
        let macos = self.models.get(model)?;
        let rows: Vec<BaselineRow> = macos
            .iter()
            .filter_map(|(metric, m)| {
                let l = *linux.get(metric)?;
                let verdict = if metric.ends_with("_ms") {
                    "informational"
                } else if super::gate::LOWER_IS_BETTER.contains(&metric.as_str()) {
                    if l <= *m { "pass" } else { "fail" }
                } else if l >= *m {
                    "pass"
                } else {
                    "fail"
                };
                Some(BaselineRow {
                    metric: metric.clone(),
                    linux: l,
                    macos: *m,
                    verdict: verdict.into(),
                })
            })
            .collect();
        Some(BaselineComparison {
            entry: model.to_string(),
            source: self.source.clone(),
            quality_matched: rows.iter().all(|r| r.verdict != "fail"),
            rows,
        })
    }
}

/// Today as `YYYY-MM-DD`.
pub fn today() -> String {
    Command::new("date")
        .arg("+%F")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

/// The repository's `HEAD`.
pub fn git_sha(repo: &Path) -> String {
    Command::new("git")
        .args(["-C", &repo.display().to_string(), "rev-parse", "HEAD"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

/// `sha256sum <file>`.
pub fn sha256(file: &Path) -> Option<String> {
    let out = Command::new("sha256sum").arg(file).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout))
        .and_then(|s| s.split_whitespace().next().map(str::to_string))
}

/// `<binary> --version`, first line.
pub fn engine_version(binary: Option<&Path>) -> String {
    let Some(binary) = binary else {
        return "none (mock run)".into();
    };
    Command::new(binary)
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

/// A file name component: lowercase, `[a-z0-9.-]`.
fn slug(text: &str) -> String {
    text.chars()
        .map(|c| match c.to_ascii_lowercase() {
            c if c.is_ascii_alphanumeric() || c == '.' || c == '-' => c,
            _ => '-',
        })
        .collect()
}

/// `<date>-<model>-<set>.json` under `dir`.
pub fn report_path(dir: &Path, report: &Report) -> PathBuf {
    let set = Path::new(&report.set)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| report.set.clone());
    dir.join(format!(
        "{}-{}-{}.json",
        report.date,
        slug(&report.model),
        slug(&set)
    ))
}

/// Writes the report and regenerates the README beside it.
pub fn write(dir: &Path, report: &Report) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = report_path(dir, report);
    let text = serde_json::to_string_pretty(report).map_err(|e| e.to_string())?;
    std::fs::write(&path, format!("{text}\n")).map_err(|e| format!("{}: {e}", path.display()))?;
    write_readme(dir)?;
    Ok(path)
}

/// The reports under `dir`, newest first by file name.
pub fn reports(dir: &Path) -> Vec<(PathBuf, Report)> {
    let mut found: Vec<(PathBuf, Report)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).ok()?;
            let report: Report = serde_json::from_str(&text).ok()?;
            Some((p, report))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found
}

fn pct(v: f64) -> String {
    format!("{:.1}%", v * 100.0)
}

/// Regenerates `README.md`: the latest report per model and set.
pub fn write_readme(dir: &Path) -> Result<(), String> {
    let mut latest: BTreeMap<(String, String), (PathBuf, Report)> = BTreeMap::new();
    for (path, report) in reports(dir) {
        latest
            .entry((report.model.clone(), report.set.clone()))
            .or_insert((path, report));
    }
    let mut out = String::from(
        "# Polish evaluation reports\n\n\
         Every file here is one `dettivo-qa polish-eval` run: a held-out set through the daemon's own Enhanced pipeline against the local engine, scored with the macOS metrics and checked against the macOS hard gates ([docs/polish-models.md](../../polish-models.md)). A report carries metrics, row ids and the model's checksum, never a row's text. `macos-baseline.json` holds the macOS figures the comparison reads; `dictionary-eval-v1.json` is the dictionary-correction eval of `dettivo-language::raw`.\n\n\
         This table is regenerated by every run: the latest report per model and set.\n\n\
         | Date | Model | Set | Backend | Rows | Protected | Required | Forbidden | Guard | Fallback | Median ms | p95 ms | Gate | Report |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for ((model, set), (path, r)) in &latest {
        let m = &r.metrics;
        out.push_str(&format!(
            "| {} | `{}` | `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | [{}]({}) |\n",
            r.date,
            model,
            set,
            r.backend,
            r.rows,
            pct(m.protected_token_preservation_rate),
            pct(m.required_fragment_retention_rate),
            pct(m.forbidden_fragment_violation_rate),
            pct(m.guard_rejection_rate),
            pct(m.fallback_rate),
            m.median_wall_ms,
            m.p95_wall_ms,
            if r.gate.passed { "pass" } else { "fail" },
            path.file_name().unwrap_or_default().to_string_lossy(),
            path.file_name().unwrap_or_default().to_string_lossy(),
        ));
    }
    std::fs::write(dir.join("README.md"), out).map_err(|e| format!("README.md: {e}"))
}
