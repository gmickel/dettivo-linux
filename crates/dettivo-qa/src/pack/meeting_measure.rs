//! The meetings pack's measurements (ADR 0039): what the throughput
//! steps, the GPU proof and the token scenario wrote under the run
//! directory, aggregated into the report's `measurements.meetings` block
//! against the NFR-4 and NFR-5 targets, and `--record`, which files the
//! same figures as the `meetings` block of the checked-in benchmark
//! report for the host and tier and re-renders the README.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::meeting_steps::DiarizationThroughput;
use super::meeting_throughput::Throughput;
use super::report::Report;
use super::{StepOutcome, StepResult};
use crate::bench::{self, Comparison, host, meetings::MeetingsBlock};
use crate::nfr;
use crate::scenarios::meeting_token_coverage::TokenCoverage;

/// Where the checked-in reports live, relative to the repository root.
pub const REPORTS_DIR: &str = "docs/reports/benchmarks";

/// The block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MeetingMeasurements {
    /// The meeting realtime factor.
    pub meeting_rtf: Option<f64>,
    /// The NFR it is held to.
    pub meeting_rtf_nfr: String,
    /// The calibrated target.
    pub meeting_rtf_target: Option<f64>,
    /// Whether the factor meets it.
    pub meeting_rtf_met: Option<bool>,
    /// The diarization realtime factor.
    pub diarization_rtf: Option<f64>,
    /// The diarization target (NFR-4).
    pub diarization_rtf_target: Option<f64>,
    /// Whether the factor meets it.
    pub diarization_rtf_met: Option<bool>,
    /// The daemon's tier.
    pub tier: String,
    /// The engine.
    pub engine: String,
    /// The model id.
    pub model: String,
    /// The backend the engine reported.
    pub backend: String,
    /// The audio imported.
    pub audio_ms: u64,
    /// The finalisation's wall time.
    pub wall_ms: u64,
    /// The fixture's SHA-256.
    pub fixture_sha: String,
    /// The harness's CPU share during the import.
    pub harness_cpu_pct: Option<f64>,
    /// The warning when the harness share passed the line.
    pub harness_warning: Option<String>,
    /// What the GPU proof concluded.
    pub gpu_workload_proof: Option<String>,
    /// The token scenario's coverage.
    pub token_coverage: Option<TokenCoverage>,
    /// `--cpu`.
    pub forced_cpu: bool,
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn evidence_dir(run_dir: &Path, steps: &[StepResult], id: &str) -> Option<PathBuf> {
    steps
        .iter()
        .filter(|s| s.id == id)
        .filter_map(|s| s.evidence.as_ref())
        .map(|e| run_dir.join(e))
        .find(|d| d.is_dir())
}

/// Aggregates the meeting evidence under `run_dir`; `None` when no
/// meeting step wrote anything (every other pack).
pub fn read(run_dir: &Path, steps: &[StepResult]) -> Option<MeetingMeasurements> {
    let throughput: Option<Throughput> = evidence_dir(run_dir, steps, "meeting_throughput")
        .and_then(|d| read_json(&d.join("meeting-throughput.json")));
    let diarization: Option<DiarizationThroughput> =
        evidence_dir(run_dir, steps, "diarization_throughput")
            .and_then(|d| read_json(&d.join("diarization-throughput.json")));
    let tokens: Option<TokenCoverage> = evidence_dir(run_dir, steps, "meeting_token_coverage")
        .and_then(|d| read_json(&d.join("token-coverage.json")));
    let proof = steps
        .iter()
        .find(|s| s.id == "gpu_workload_proof" && s.outcome != StepOutcome::NotRun)
        .map(|s| {
            format!(
                "{}: {}",
                s.outcome.as_str(),
                s.reason.as_deref().unwrap_or("")
            )
        });
    if throughput.is_none() && diarization.is_none() && tokens.is_none() {
        return None;
    }
    let mut m = MeetingMeasurements {
        token_coverage: tokens,
        gpu_workload_proof: proof,
        ..MeetingMeasurements::default()
    };
    if let Some(t) = throughput {
        m.meeting_rtf = Some(t.realtime_factor);
        m.meeting_rtf_nfr = t.nfr;
        m.meeting_rtf_target = Some(t.target);
        m.meeting_rtf_met = Some(t.met);
        m.tier = t.tier;
        m.engine = t.engine;
        m.model = t.model;
        m.backend = t.backend;
        m.audio_ms = t.audio_ms;
        m.wall_ms = t.wall_ms;
        m.fixture_sha = t.fixture_sha256;
        m.harness_cpu_pct = Some(t.harness_cpu_pct);
        m.harness_warning = t.harness_warning;
        m.forced_cpu = t.forced_cpu;
    }
    if let Some(d) = diarization {
        m.diarization_rtf = Some(d.realtime_factor);
        m.diarization_rtf_target = Some(d.target);
        m.diarization_rtf_met = Some(d.met);
    }
    Some(m)
}

/// The block `--record` files, from a pack report.
pub fn block(repo_root: &Path, report: &Report) -> Result<MeetingsBlock, String> {
    let m = report
        .measurements
        .meetings
        .as_ref()
        .ok_or("the report carries no meeting measurements (did meeting_throughput run?)")?;
    let tier = if m.tier == "gpu" {
        nfr::Tier::Gpu
    } else {
        nfr::Tier::Cpu
    };
    Ok(MeetingsBlock {
        recorded_unix: report.started_unix,
        date: bench::date_of(report.started_unix),
        git_sha: host::git_sha(repo_root),
        pack_run_dir: report.run_dir.clone(),
        pack_passed: report.passed,
        forced_cpu: m.forced_cpu,
        engine: m.engine.clone(),
        model: m.model.clone(),
        backend: m.backend.clone(),
        audio_ms: m.audio_ms,
        wall_ms: m.wall_ms,
        fixture_sha256: m.fixture_sha.clone(),
        meeting: Comparison::of(&nfr::stt_realtime(tier), m.meeting_rtf),
        diarization: Comparison::of(&nfr::NFR4_DIARIZATION_REALTIME_GPU, m.diarization_rtf),
        gpu_workload_proof: m
            .gpu_workload_proof
            .clone()
            .unwrap_or_else(|| "not run".into()),
        harness_cpu_pct: m.harness_cpu_pct,
        harness_warning: m.harness_warning.clone(),
        token_coverage: m
            .token_coverage
            .as_ref()
            .map(|t| serde_json::to_value(t).unwrap_or(Value::Null)),
    })
}

/// The report file the block lands in: the latest one for the host and
/// tier, written in place, so the file keeps the benchmark's own date
/// and commit and the block carries the pack's; none at all names
/// `just bench`.
fn target_file(dir: &Path, host: &str, tier: &str) -> Result<PathBuf, String> {
    let suffix = format!("-{host}-{tier}.json");
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(&suffix))
        .collect();
    candidates.sort();
    candidates.pop().ok_or_else(|| {
        format!(
            "no benchmark report for {host} on the {tier} tier under {}; run `just bench` first",
            dir.display()
        )
    })
}

/// Files the block into the checked-in report and re-renders the README.
pub fn record(repo_root: &Path, report: &Report) -> Result<PathBuf, String> {
    let block = block(repo_root, report)?;
    let dir = repo_root.join(REPORTS_DIR);
    // The file's tier is the run's: `cpu` under `--cpu`, the report's
    // tier otherwise, so the two rows of one desktop never merge.
    let tier = if block.forced_cpu {
        "cpu"
    } else {
        report.tier.as_str()
    };
    let to = target_file(&dir, &report.machine.hostname, tier)?;
    let text = std::fs::read_to_string(&to).map_err(|e| format!("{}: {e}", to.display()))?;
    let mut bench: bench::Report =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", to.display()))?;
    bench.meetings = Some(block);
    let json = serde_json::to_string_pretty(&bench).map_err(|e| e.to_string())? + "\n";
    std::fs::write(&to, json).map_err(|e| format!("{}: {e}", to.display()))?;
    let readme = bench::render::readme(&dir)?;
    std::fs::write(dir.join("README.md"), readme).map_err(|e| e.to_string())?;
    Ok(to)
}

/// The measurement lines of `<pack>-pack.md`.
pub fn markdown(m: &MeetingMeasurements, out: &mut String) {
    let verdict = |met: Option<bool>| match met {
        Some(true) => "met",
        Some(false) => "not met",
        None => "not measured",
    };
    out.push_str(&format!(
        "- Meeting throughput ({}, tier {}{}): {} against {}: {}. {} {} on {} transcribed {} ms of audio in {} ms through `transcripts.import`; fixture `{}`.\n",
        if m.meeting_rtf_nfr.is_empty() {
            "NFR-4/NFR-5"
        } else {
            m.meeting_rtf_nfr.as_str()
        },
        m.tier,
        if m.forced_cpu { ", --cpu" } else { "" },
        m.meeting_rtf
            .map(|r| format!("{r:.2}x realtime"))
            .unwrap_or_else(|| "-".into()),
        m.meeting_rtf_target
            .map(|t| format!(">= {t:.0}x"))
            .unwrap_or_else(|| "no target".into()),
        verdict(m.meeting_rtf_met),
        m.engine,
        m.model,
        m.backend,
        m.audio_ms,
        m.wall_ms,
        &m.fixture_sha[..m.fixture_sha.len().min(12)]
    ));
    out.push_str(&format!(
        "- Diarization throughput (NFR-4): {} against {}: {}.\n",
        m.diarization_rtf
            .map(|r| format!("{r:.2}x realtime"))
            .unwrap_or_else(|| "-".into()),
        m.diarization_rtf_target
            .map(|t| format!(">= {t:.0}x"))
            .unwrap_or_else(|| "no target".into()),
        verdict(m.diarization_rtf_met)
    ));
    out.push_str(&format!(
        "- Harness load during the import: {}{}.\n",
        m.harness_cpu_pct
            .map(|p| format!("{p:.1}% of a core"))
            .unwrap_or_else(|| "not measured".into()),
        m.harness_warning
            .as_deref()
            .map(|w| format!(" (warning: {w})"))
            .unwrap_or_default()
    ));
    out.push_str(&format!(
        "- GPU workload proof: {}.\n",
        m.gpu_workload_proof.as_deref().unwrap_or("not run")
    ));
    match &m.token_coverage {
        Some(t) => out.push_str(&format!(
            "- Token coverage (alpha fixture, {} capture): {} of {} tokens on their source{}; speakers after diarization: {}; names in the md export: {}.\n",
            t.capture,
            t.found,
            t.expected,
            if t.missing.is_empty() {
                String::new()
            } else {
                format!(" (missing: {})", t.missing.join(", "))
            },
            t.speakers.join(", "),
            if t.md_names_found { "both" } else { "missing" }
        )),
        None => out.push_str("- Token coverage: not measured.\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_block_lands_in_the_latest_report_in_place_and_never_under_a_new_date() {
        let dir = tempfile::tempdir().unwrap();
        assert!(target_file(dir.path(), "thor", "gpu").is_err());
        std::fs::write(dir.path().join("2026-09-01-thor-gpu.json"), "{}").unwrap();
        std::fs::write(dir.path().join("2026-09-03-thor-gpu.json"), "{}").unwrap();
        std::fs::write(dir.path().join("2026-09-04-thor-cpu.json"), "{}").unwrap();
        let to = target_file(dir.path(), "thor", "gpu").unwrap();
        assert!(to.ends_with("2026-09-03-thor-gpu.json"), "{}", to.display());
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            3,
            "no copy is made"
        );
    }

    #[test]
    fn the_measurements_read_back_from_the_evidence_and_render() {
        let dir = tempfile::tempdir().unwrap();
        let run = dir.path();
        std::fs::create_dir_all(run.join("meeting_throughput")).unwrap();
        std::fs::write(
            run.join("meeting_throughput/meeting-throughput.json"),
            serde_json::json!({
                "engine": "whisper", "model": "base.en", "backend": "cpu", "reason": "forced",
                "tier": "cpu", "tier_reason": "DETTIVO_FORCE_CPU", "audio_ms": 300000, "wall_ms": 30000,
                "job_wall_ms": 31000, "realtime_factor": 10.0, "nfr": "NFR-5", "target": 5.0, "met": true,
                "fixture_sha256": "abcdef", "harness_idle_cpu_pct": 0.5, "harness_cpu_pct": 12.0,
                "harness_warning": "over", "segments": 40, "words": 500, "chunks_total": 20,
                "forced_cpu": true, "load_1m": [1.0, 2.0]
            })
            .to_string(),
        )
        .unwrap();
        let steps = vec![
            StepResult {
                id: "meeting_throughput".into(),
                driver: "none".into(),
                outcome: StepOutcome::Pass,
                duration_ms: 1,
                evidence: Some("meeting_throughput".into()),
                reason: None,
                surface: None,
                scan: None,
            },
            StepResult {
                id: "gpu_workload_proof".into(),
                driver: "none".into(),
                outcome: StepOutcome::Skip,
                duration_ms: 1,
                evidence: None,
                reason: Some("cpu tier".into()),
                surface: None,
                scan: None,
            },
        ];
        let m = read(run, &steps).unwrap();
        assert_eq!(m.meeting_rtf, Some(10.0));
        assert_eq!(m.meeting_rtf_nfr, "NFR-5");
        assert_eq!(m.gpu_workload_proof.as_deref(), Some("skip: cpu tier"));
        assert!(m.diarization_rtf.is_none());
        let mut out = String::new();
        markdown(&m, &mut out);
        assert!(out.contains("10.00x realtime against >= 5x: met"), "{out}");
        assert!(out.contains("warning: over"), "{out}");
        assert!(
            out.contains("Diarization throughput (NFR-4): - against no target: not measured"),
            "{out}"
        );
        assert!(read(run, &[]).is_none());
    }
}
