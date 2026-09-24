//! The measurements block of a pack report (ADR 0017, ADR 0029): what
//! the dictation steps wrote under the run directory (the insertion
//! matrix rows, the first-insert runs, the WER result) aggregated
//! against the NFR targets, and the tier the engine's backend puts the
//! run in. `report::build` calls `measure` and `tier`; a GUI pack has
//! none of this evidence and records an empty block.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::StepResult;
use super::wer::WerResult;
use crate::nfr;
use crate::scenarios::first_insert_timing::{Split, Timing};
use crate::scenarios::support::MatrixRow;
use crate::stats::reliability;

/// NFR-3: insertion reliability across the target matrix.
pub const NFR3_TARGET: f64 = 0.98;

/// A matrix target that failed, with what ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailedTarget {
    /// The target.
    pub target: String,
    /// The backend that ran, when one did.
    pub backend: Option<String>,
    /// Why.
    pub reason: String,
}

/// The measurements block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Measurements {
    /// p50 over the warm runs.
    pub first_insert_p50_ms: Option<u64>,
    /// p95 over the warm runs.
    pub first_insert_p95_ms: Option<u64>,
    /// The excluded cold run.
    pub first_insert_cold_ms: Option<u64>,
    /// Warm runs that completed.
    pub first_insert_warm_runs: usize,
    /// p50 of each part of the path.
    pub first_insert_split_p50: Option<Split>,
    /// The first-insert NFR the tier is held to (`NFR-1` on the GPU,
    /// `NFR-2` on the CPU), from `crate::nfr`.
    pub first_insert_nfr: String,
    /// The calibrated first-insert target for the tier (ADR 0029).
    pub nfr1_target_ms: Option<u64>,
    /// p50 at or under the target; `None` without a figure.
    pub nfr1_met: Option<bool>,
    /// Passes over attempted targets.
    pub insertion_reliability: Option<f64>,
    /// Attempted targets.
    pub insertion_attempted: usize,
    /// Passed targets.
    pub insertion_passed: usize,
    /// Targets excluded from the figure, with why.
    pub insertion_skipped: Vec<String>,
    /// Targets that failed, with backend and reason.
    pub insertion_failed: Vec<FailedTarget>,
    /// The NFR-3 target.
    pub nfr3_target: f64,
    /// Reliability at or over the target; `None` without a figure.
    pub nfr3_met: Option<bool>,
    /// The matrix rows as the scenario wrote them.
    pub matrix: Vec<MatrixRow>,
    /// The WER result.
    pub wer: Option<WerResult>,
    /// The meetings pack's figures (ADR 0039); absent for every other pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meetings: Option<super::meeting_measure::MeetingMeasurements>,
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

/// Aggregates the evidence the steps wrote under `run_dir`.
pub fn measure(
    run_dir: &Path,
    steps: &[StepResult],
) -> (Measurements, Option<String>, Option<Timing>) {
    let mut m = Measurements {
        nfr3_target: NFR3_TARGET,
        ..Measurements::default()
    };
    if let Some(dir) = evidence_dir(run_dir, steps, "insertion_matrix") {
        let rows: Vec<MatrixRow> =
            read_json(&dir.join("insertion-matrix.json")).unwrap_or_default();
        for row in &rows {
            match row.outcome.as_str() {
                "skipped" => m.insertion_skipped.push(format!(
                    "{}: {}",
                    row.target,
                    row.reason.as_deref().unwrap_or("skipped")
                )),
                "pass" => {
                    m.insertion_attempted += 1;
                    m.insertion_passed += 1;
                }
                _ => {
                    m.insertion_attempted += 1;
                    m.insertion_failed.push(FailedTarget {
                        target: row.target.clone(),
                        backend: row.backend.clone(),
                        reason: row.reason.clone().unwrap_or_else(|| "failed".into()),
                    });
                }
            }
        }
        m.insertion_reliability = reliability(m.insertion_passed, m.insertion_attempted);
        m.nfr3_met = m.insertion_reliability.map(|r| r >= NFR3_TARGET);
        m.matrix = rows;
    }
    let timing: Option<Timing> = evidence_dir(run_dir, steps, "first_insert_timing")
        .and_then(|dir| read_json(&dir.join("first-insert-timing.json")));
    if let Some(t) = &timing {
        m.first_insert_p50_ms = t.p50_ms;
        m.first_insert_p95_ms = t.p95_ms;
        m.first_insert_cold_ms = t.runs.iter().find(|r| r.cold).map(|r| r.total_ms);
        m.first_insert_warm_runs = t.warm_completed;
        m.first_insert_split_p50 = Some(t.split_p50.clone());
    }
    m.wer = evidence_dir(run_dir, steps, "whisper_wer")
        .and_then(|dir| read_json(&dir.join("wer.json")));
    m.meetings = super::meeting_measure::read(run_dir, steps);
    let engine_backend = m
        .wer
        .as_ref()
        .map(|w| w.backend.clone())
        .or_else(|| timing.as_ref().and_then(|t| t.engine_backend.clone()))
        .or_else(|| {
            m.meetings
                .as_ref()
                .filter(|x| !x.backend.is_empty())
                .map(|x| x.backend.clone())
        });
    (m, engine_backend, timing)
}

/// The tier the engine's backend puts the run in and the first-insert
/// target it is held to: NFR-1 on the GPU, NFR-2 on the CPU, both
/// calibrated in `crate::nfr` (ADR 0029).
pub fn tier(engine_backend: Option<&str>) -> (nfr::Tier, nfr::Target) {
    let tier = nfr::Tier::from_backend(engine_backend);
    (tier, nfr::first_insert(tier))
}
