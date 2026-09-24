//! Packs (ADR 0017, ADR 0037): a named, ordered list of steps that runs
//! into one evidence run directory and one report. A step is a scenario
//! from the drive pack, a repository command (a lint, a Qt Quick test),
//! or the Whisper word-error-rate check; each declares the driver it
//! pins, whether a skip is allowed, the surface it belongs to and the
//! measurement it contributes. Scenarios stay unchanged; the pack
//! composes them. `dictation` holds the dictation and release packs,
//! `gui` the four per-surface GUI packs and their composite, `execute`
//! runs a pack for real, `scan` walks every captured tree for developer
//! text and unnamed controls, `report` and `surfaces` aggregate the
//! results, `measure` aggregates the dictation evidence against the
//! NFR targets, `markdown` renders `<pack>-pack.md` and writes both files,
//! `wer` is the engine check, `gate` holds the strict contract replay
//! and the packaging step (ADR 0034), `meetings` the meeting lane end to
//! end with its throughput steps (ADR 0039: `meeting_steps`,
//! `meeting_throughput`, `gpu_proof`, `meeting_measure`), and `release`
//! is the release gate over whole commands (ADR 0041: `release_steps`,
//! `release_report`).

pub mod binaries;
pub mod dictation;
pub mod execute;
pub mod gate;
pub mod gpu_proof;
pub mod gui;
pub mod markdown;
pub mod measure;
pub mod meeting_measure;
pub mod meeting_steps;
pub mod meeting_throughput;
pub mod meetings;
pub mod release;
pub mod release_bench;
pub mod release_blockers;
pub mod release_checks;
pub mod release_report;
pub mod release_steps;
pub mod report;
pub mod scan;
pub mod surfaces;
pub mod wer;

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// What a step runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// A scenario from `scenarios::by_id`, by id.
    Scenario,
    /// A repository command (`gui::command_for`), run from the root.
    Command,
    /// The Whisper WER fixture through the engine binary.
    WhisperWer,
    /// The contract fixtures replayed under `--strict` against a fresh
    /// daemon (`gate::contract_step`).
    ContractStrict,
    /// The clean-machine install test's report (`gate::packaging_step`).
    Packaging,
    /// The diarization pipeline against the two-speaker fixture
    /// (`meeting_steps::diarization_der`).
    DiarizationDer,
    /// A five-minute meeting import through the daemon, timed
    /// (`meeting_throughput`).
    MeetingThroughput,
    /// The diarization engine's CLI mode over the same system track
    /// (`meeting_steps::diarization_throughput`).
    DiarizationThroughput,
    /// The engine pid seen on the GPU during the throughput step
    /// (`gpu_proof`).
    GpuWorkloadProof,
}

/// What the pack expects of a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// The step must run and pass; a skip is a hard failure.
    Pass,
    /// A skip with a reason is allowed and recorded as a blocker.
    SkipAllowed,
}

/// One step of a pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    /// The scenario id, the command name, or `whisper_wer`.
    pub id: &'static str,
    /// What runs.
    pub kind: StepKind,
    /// A pinned driver; `None` takes the pack run's `--driver`.
    pub driver: Option<&'static str>,
    /// Pass, or skip allowed.
    pub expect: Expect,
    /// The surface the step proves (`onboarding`, `settings`, `history`,
    /// `omarchy`); `None` for a pack without surfaces.
    pub surface: Option<&'static str>,
    /// The measurement the step feeds, for the listing.
    pub contributes: &'static str,
}

/// A named pack.
#[derive(Debug, Clone)]
pub struct Pack {
    /// The name `dettivo-qa pack <name>` takes.
    pub name: &'static str,
    /// One line for the listing.
    pub summary: &'static str,
    /// The steps, in run order.
    pub steps: Vec<Step>,
}

impl Pack {
    /// The surfaces the steps name, in first-seen order.
    pub fn surfaces(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for s in self.steps.iter().filter_map(|s| s.surface) {
            if !out.contains(&s) {
                out.push(s);
            }
        }
        out
    }
}

/// A scenario step on the run's driver.
pub const fn scenario(id: &'static str, expect: Expect, contributes: &'static str) -> Step {
    Step {
        id,
        kind: StepKind::Scenario,
        driver: None,
        expect,
        surface: None,
        contributes,
    }
}

/// Every scenario pack: the dictation slice, the GUI packs (the four
/// surfaces and their composite), then the meeting lane; the release
/// gate (`release`) runs whole commands and is dispatched by name.
pub fn packs() -> Vec<Pack> {
    let mut all = dictation::packs();
    all.extend(gui::packs());
    all.push(meetings::pack());
    all
}

/// A pack by name; the error names every pack.
pub fn by_name(name: &str) -> Result<Pack, String> {
    let all = packs();
    all.iter().find(|p| p.name == name).cloned().ok_or_else(|| {
        format!(
            "unknown pack {name:?}; the packs are: {}",
            all.iter().map(|p| p.name).collect::<Vec<_>>().join(", ")
        )
    })
}

/// The outcome of one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepOutcome {
    /// Passed.
    Pass,
    /// Skipped with a reason.
    Skip,
    /// Failed.
    Fail,
    /// Never started: an earlier step failed hard and `--continue` was off.
    NotRun,
}

impl StepOutcome {
    /// The lowercase word the reports use.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Skip => "skip",
            Self::Fail => "fail",
            Self::NotRun => "not_run",
        }
    }
}

/// One step's row in the report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepResult {
    /// The step id.
    pub id: String,
    /// The driver it ran on (`none` for a step that needs none).
    pub driver: String,
    /// The outcome.
    pub outcome: StepOutcome,
    /// Wall time.
    pub duration_ms: u64,
    /// The evidence directory relative to the pack's run directory.
    pub evidence: Option<String>,
    /// Why, when not a pass.
    pub reason: Option<String>,
    /// The surface the step proves, when the pack has surfaces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,
    /// The scans over the trees the step captured, when it captured any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan: Option<scan::StepScan>,
}

impl StepResult {
    /// A step that never started.
    pub fn not_run(step: &Step, driver: &str, because: &str) -> Self {
        Self {
            id: step.id.to_string(),
            driver: driver.to_string(),
            outcome: StepOutcome::NotRun,
            duration_ms: 0,
            evidence: None,
            reason: Some(format!("not run: {because}")),
            surface: step.surface.map(str::to_string),
            scan: None,
        }
    }

    /// True when the result fails the pack: a failure, or a skip or
    /// not-run on a step the pack expects to pass.
    pub fn hard_failure(&self, expect: Expect) -> bool {
        match self.outcome {
            StepOutcome::Fail => true,
            StepOutcome::Pass => false,
            StepOutcome::Skip | StepOutcome::NotRun => expect == Expect::Pass,
        }
    }
}

/// How a pack run is configured.
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// The repository root.
    pub repo_root: PathBuf,
    /// The driver for steps that pin none.
    pub driver: String,
    /// Keep going after a hard failure.
    pub continue_after_failure: bool,
    /// The evidence base directory.
    pub evidence_base: PathBuf,
    /// The real model directory, when it exists.
    pub models_dir: Option<PathBuf>,
    /// Window and label wait.
    pub timeout: Duration,
    /// The meetings pack's own switches (`--cpu`, `--model`, `--engines`,
    /// `--record`); the defaults for every other pack.
    pub meetings: meetings::Options,
    /// The release gate's own switches (`--skip`, `--record`).
    pub release: release::Options,
}

/// The driver a step runs on; `none` for a step that drives no window.
pub fn driver_for(step: &Step, default: &str) -> String {
    match step.kind {
        StepKind::Scenario => step.driver.unwrap_or(default).to_string(),
        StepKind::Command
        | StepKind::WhisperWer
        | StepKind::ContractStrict
        | StepKind::Packaging
        | StepKind::DiarizationDer
        | StepKind::MeetingThroughput
        | StepKind::DiarizationThroughput
        | StepKind::GpuWorkloadProof => "none".to_string(),
    }
}

/// Runs the steps in order through `exec`, stopping after a hard failure
/// unless `continue_after_failure`; the steps after a stop are `not_run`
/// naming the step that failed. Returns one result per step.
pub fn run_steps(
    pack: &Pack,
    default_driver: &str,
    continue_after_failure: bool,
    exec: &mut dyn FnMut(&Step, &str) -> StepResult,
) -> Vec<StepResult> {
    let mut results = Vec::with_capacity(pack.steps.len());
    let mut stopped_by: Option<String> = None;
    for step in &pack.steps {
        let driver = driver_for(step, default_driver);
        if let Some(failed) = &stopped_by {
            results.push(StepResult::not_run(
                step,
                &driver,
                &format!("{failed} failed"),
            ));
            continue;
        }
        let mut result = exec(step, &driver);
        result.surface = step.surface.map(str::to_string);
        if result.hard_failure(step.expect) && !continue_after_failure {
            stopped_by = Some(step.id.to_string());
        }
        results.push(result);
    }
    results
}

/// True when the pack passed: no hard failure among the results.
pub fn passed(pack: &Pack, results: &[StepResult]) -> bool {
    pack.steps
        .iter()
        .zip(results)
        .all(|(step, r)| !r.hard_failure(step.expect))
}

/// `dettivo-qa pack <name>`: `list` prints the packs and their steps;
/// otherwise the pack runs, narrowed to one `surface` when asked. Exit 0
/// when every step passed or was skipped for an allowed reason, 1 on a
/// hard failure, 2 on a preflight refusal (an unknown pack or surface, a
/// missing binary, an unwritable evidence directory).
pub fn command(name: &str, surface: Option<&str>, opts: &RunOptions, json: bool) -> u8 {
    if name == "list" {
        println!(
            "{:<16} the release gate: every named step below as a whole command, each report embedded, the external blockers named (docs/RELEASING.md)",
            release::NAME
        );
        for step in release::STEP_IDS {
            println!("    {step:<30} -      must pass    -           its own report");
        }
        for pack in packs() {
            println!("{:<16} {}", pack.name, pack.summary);
            for step in &pack.steps {
                println!(
                    "    {:<30} {:<6} {:<12} {:<11} {}",
                    step.id,
                    step.driver.unwrap_or("-"),
                    match step.expect {
                        Expect::Pass => "must pass",
                        Expect::SkipAllowed => "skip allowed",
                    },
                    step.surface.unwrap_or("-"),
                    step.contributes
                );
            }
        }
        return 0;
    }
    if name == release::NAME {
        return release::run(opts, &opts.release, json);
    }
    let pack = match by_name(name)
        .and_then(|p| gui::narrow(p, surface))
        .and_then(|p| meetings::preflight(&p, opts).map(|()| p))
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("pack: {e}");
            return 2;
        }
    };
    execute::run(&pack, opts, json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(step: &Step, driver: &str, outcome: StepOutcome) -> StepResult {
        StepResult {
            id: step.id.into(),
            driver: driver.into(),
            outcome,
            duration_ms: 1,
            evidence: Some(format!("{}.{driver}", step.id)),
            reason: None,
            surface: None,
            scan: None,
        }
    }

    #[test]
    fn the_dictation_pack_runs_its_steps_in_order_and_names_unknown_packs() {
        let pack = by_name("dictation").unwrap();
        let ids: Vec<&str> = pack.steps.iter().map(|s| s.id).collect();
        assert_eq!(
            ids,
            [
                "hotkeys_hyprland",
                "osd_dictation",
                "osd_dictation",
                "insertion_matrix",
                "never_into_self",
                "history_roundtrip",
                "first_insert_timing",
                "whisper_wer"
            ]
        );
        for step in pack.steps.iter().filter(|s| s.kind == StepKind::Scenario) {
            assert!(
                crate::scenarios::by_id(step.id).is_some(),
                "{} is not a scenario",
                step.id
            );
        }
        assert_eq!(driver_for(&pack.steps[2], "atspi"), "cua");
        assert_eq!(driver_for(&pack.steps[3], "cua"), "cua");
        assert_eq!(driver_for(&pack.steps[7], "cua"), "none");
        let err = by_name("nope").unwrap_err();
        assert!(err.contains("nope") && err.contains("dictation"), "{err}");
        assert!(err.contains("gui-onboarding"), "{err}");
        assert!(pack.surfaces().is_empty());
    }

    #[test]
    fn a_hard_failure_stops_the_pack_and_marks_the_rest_not_run() {
        let pack = by_name("dictation").unwrap();
        let mut ran = Vec::new();
        let results = run_steps(&pack, "atspi", false, &mut |step, driver| {
            ran.push(step.id);
            let outcome = if step.id == "insertion_matrix" {
                StepOutcome::Fail
            } else {
                StepOutcome::Pass
            };
            result(step, driver, outcome)
        });
        assert_eq!(ran.len(), 4);
        assert_eq!(results.len(), pack.steps.len());
        assert_eq!(results[3].outcome, StepOutcome::Fail);
        for r in &results[4..] {
            assert_eq!(r.outcome, StepOutcome::NotRun);
            assert_eq!(
                r.reason.as_deref(),
                Some("not run: insertion_matrix failed")
            );
        }
        assert!(!passed(&pack, &results));
    }

    #[test]
    fn continue_runs_every_step_and_allowed_skips_pass() {
        let pack = by_name("dictation").unwrap();
        let mut ran = 0;
        let results = run_steps(&pack, "atspi", true, &mut |step, driver| {
            ran += 1;
            let outcome = match step.id {
                "insertion_matrix" => StepOutcome::Fail,
                "hotkeys_hyprland" => StepOutcome::Skip,
                _ => StepOutcome::Pass,
            };
            result(step, driver, outcome)
        });
        assert_eq!(ran, pack.steps.len());
        assert!(results.iter().all(|r| r.outcome != StepOutcome::NotRun));
        assert!(!passed(&pack, &results));

        let results = run_steps(&pack, "atspi", false, &mut |step, driver| {
            let outcome = if step.expect == Expect::SkipAllowed {
                StepOutcome::Skip
            } else {
                StepOutcome::Pass
            };
            result(step, driver, outcome)
        });
        assert!(passed(&pack, &results));
    }

    #[test]
    fn a_skip_on_a_must_pass_step_is_a_hard_failure() {
        let pack = by_name("dictation").unwrap();
        let results = run_steps(&pack, "atspi", false, &mut |step, driver| {
            let outcome = if step.id == "history_roundtrip" {
                StepOutcome::Skip
            } else {
                StepOutcome::Pass
            };
            result(step, driver, outcome)
        });
        assert!(!passed(&pack, &results));
        assert_eq!(results[6].outcome, StepOutcome::NotRun);
    }

    #[test]
    fn a_step_result_from_an_older_report_still_reads() {
        let old = r#"{"id": "x", "driver": "atspi", "outcome": "pass", "duration_ms": 1, "evidence": null, "reason": null}"#;
        let r: StepResult = serde_json::from_str(old).unwrap();
        assert_eq!(r.surface, None);
        assert!(r.scan.is_none());
    }
}
