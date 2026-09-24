//! The release gate's two non-scenario steps (ADR 0034): the contract
//! replay under `--strict` against a fresh daemon, and the packaging step
//! that reads `install-test.json` from the clean-machine install test so
//! the release report carries the same named steps as the gate in
//! `docs/RELEASING.md`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::{RunOptions, Step, StepOutcome, StepResult};
use crate::replay;

/// The environment variable naming the install test's report; without
/// it the report is read from `build/install-test.json` under the
/// repository, where `just install-test` writes it.
pub const REPORT_ENV: &str = "DETTIVO_INSTALL_TEST_REPORT";

fn elapsed_ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn result(
    step: &Step,
    outcome: StepOutcome,
    evidence: Option<&str>,
    reason: Option<String>,
) -> StepResult {
    StepResult {
        id: step.id.into(),
        driver: "none".into(),
        outcome,
        duration_ms: 0,
        evidence: evidence.map(str::to_string),
        reason,
        surface: step.surface.map(str::to_string),
        scan: None,
    }
}

/// The contract fixtures replayed against a daemon in a throwaway
/// profile with every pending method counted as a failure; the report
/// lands under `<run>/contract_strict/contract-replay.json`.
pub fn contract_step(step: &Step, run_dir: &Path, opts: &RunOptions) -> StepResult {
    let started = Instant::now();
    let dir = run_dir.join(step.id);
    let mut r = match run_contract(&dir, opts) {
        Ok(report) => {
            let (pass, fail, skip, pending) = report.counts();
            let admitted = report.admitted().len();
            if report.ok(true) {
                let reason = (admitted > 0).then(|| {
                    format!(
                        "{admitted} pending behind a false flag (admitted gaps: {})",
                        report
                            .admitted()
                            .iter()
                            .map(|r| r.method.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                });
                result(step, StepOutcome::Pass, Some(step.id), reason)
            } else {
                let promises = report.broken_promises();
                let broken = if promises.is_empty() {
                    String::new()
                } else {
                    format!("; pending behind a true flag: {}", promises.join(", "))
                };
                result(
                    step,
                    StepOutcome::Fail,
                    Some(step.id),
                    Some(format!(
                        "{fail} failed and {} pending of {} fixtures ({pass} passed, {skip} skipped, {admitted} pending admitted by a false flag){broken}; see {}",
                        pending - admitted,
                        pass + fail + skip + pending,
                        dir.join("contract-replay.json").display()
                    )),
                )
            }
        }
        Err(why) => result(step, StepOutcome::Fail, None, Some(why)),
    };
    r.duration_ms = elapsed_ms(started);
    r
}

fn run_contract(dir: &Path, opts: &RunOptions) -> Result<replay::Report, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let report = crate::contract::run(
        &opts.repo_root,
        opts.models_dir.clone(),
        opts.timeout.max(Duration::from_secs(10)),
    )?;
    let json = serde_json::to_string_pretty(&report).unwrap_or_default();
    std::fs::write(dir.join("contract-replay.json"), json)
        .map_err(|e| format!("contract-replay.json: {e}"))?;
    Ok(report)
}

/// The token the contract daemon's REST shim expects, the one the
/// `dettivo-qa contract` verb uses.
pub use crate::contract::REST_TOKEN;

/// Where the packaging step reads its report from.
pub fn report_path(repo_root: &Path) -> PathBuf {
    std::env::var_os(REPORT_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join("build/install-test.json"))
}

/// The install test's report as a step: pass when the receipt carries
/// every required check passed for this version, fail naming what it
/// lacks; a missing report is missing proof and fails too. The report
/// is copied to `<run>/packaging/install-test.json`.
pub fn packaging_step(step: &Step, run_dir: &Path, opts: &RunOptions) -> StepResult {
    let version = match super::release::workspace_version(&opts.repo_root) {
        Ok(v) => v,
        Err(e) => return result(step, StepOutcome::Fail, None, Some(e)),
    };
    judge_file(step, run_dir, &report_path(&opts.repo_root), &version)
}

fn judge_file(step: &Step, run_dir: &Path, path: &Path, version: &str) -> StepResult {
    let Ok(text) = std::fs::read_to_string(path) else {
        return result(
            step,
            StepOutcome::Fail,
            None,
            Some(format!(
                "{} is missing, so the clean install is unproven; run `just package-bin` then `just install-test` and set {REPORT_ENV}",
                path.display()
            )),
        );
    };
    let dir = run_dir.join(step.id);
    let copied = std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::write(dir.join("install-test.json"), &text))
        .is_ok();
    let evidence = copied.then_some(step.id);
    match judge(&text, version) {
        Ok(None) => result(step, StepOutcome::Pass, evidence, None),
        Ok(Some(why)) => result(step, StepOutcome::Fail, evidence, Some(why)),
        Err(why) => result(
            step,
            StepOutcome::Fail,
            evidence,
            Some(format!("{}: {why}", path.display())),
        ),
    }
}

/// The receipt `scripts/packaging/install-test.sh` writes.
#[derive(Debug, Deserialize)]
pub struct Receipt {
    /// 1.
    pub schema_version: u32,
    /// The package file the test installed.
    pub package: String,
    /// `Cargo.toml`'s version at the time.
    pub version: String,
    /// `prefix` or `system`.
    pub mode: String,
    /// True when `--session` ran the desktop checks.
    #[serde(default)]
    pub session: bool,
    /// The script's own verdict.
    pub passed: bool,
    /// One row per check.
    pub steps: Vec<ReceiptStep>,
}

/// One check of the install test.
#[derive(Debug, Deserialize)]
pub struct ReceiptStep {
    /// The check's name.
    pub step: String,
    /// `pass`, `skip` or `fail`.
    pub status: String,
    /// The command's exit code.
    pub exit_code: i64,
    /// What the check saw.
    #[serde(default)]
    pub detail: String,
}

/// The checks a receipt proves a clean install with: the file list, the
/// desktop entry and the units, the CLI's version and completions, the
/// Home render, every engine's help, the CPU-fallback smoke, the natural
/// backend and one dictation through the installed daemon.
pub const REQUIRED_CHECKS: &[&str] = &[
    "files",
    "desktop",
    "units",
    "version",
    "completions_bash",
    "completions_zsh",
    "completions_fish",
    "render",
    "engine_help_whisper",
    "engine_help_parakeet",
    "engine_help_llm",
    "engine_help_diarize",
    "cpu_smoke_whisper",
    "cpu_smoke_parakeet",
    "cpu_smoke_llm",
    "natural_backend",
    "dictation",
];

/// `None` when the receipt proves the install of `version`: every required
/// check present once and passed with exit 0, the package named for the
/// version, `passed` true. Otherwise what it lacks, every defect named.
pub fn judge(text: &str, version: &str) -> Result<Option<String>, String> {
    let receipt: Receipt = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let mut why = Vec::new();
    if receipt.schema_version != 1 {
        why.push(format!(
            "schema_version {} is not 1",
            receipt.schema_version
        ));
    }
    if receipt.version != version {
        why.push(format!(
            "the receipt is for version {} rather than {version}",
            receipt.version
        ));
    }
    if !receipt.package.contains(version) {
        why.push(format!(
            "the package {:?} is not a {version} package",
            receipt.package
        ));
    }
    if !receipt.passed {
        why.push("the receipt says passed=false".into());
    }
    for name in REQUIRED_CHECKS {
        let rows: Vec<&ReceiptStep> = receipt.steps.iter().filter(|s| s.step == *name).collect();
        match rows.as_slice() {
            [] => why.push(format!("{name}: no row")),
            [row] => match (row.status.as_str(), row.exit_code) {
                ("pass", 0) => {}
                ("pass", code) => why.push(format!("{name}: passed with exit {code}")),
                ("skip", _) => why.push(format!("{name}: skipped, {}", row.detail)),
                ("fail", code) => why.push(format!("{name} (exit {code}): {}", row.detail)),
                (other, _) => why.push(format!("{name}: status {other:?}")),
            },
            _ => why.push(format!("{name}: {} rows", rows.len())),
        }
    }
    for row in &receipt.steps {
        if !REQUIRED_CHECKS.contains(&row.step.as_str()) && row.status == "fail" {
            why.push(format!(
                "{} (exit {}): {}",
                row.step, row.exit_code, row.detail
            ));
        }
    }
    Ok((!why.is_empty()).then(|| why.join("; ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(version: &str, rows: &[(&str, &str, i64)]) -> String {
        let steps: Vec<String> = rows
            .iter()
            .map(|(step, status, code)| {
                format!(
                    r#"{{"step": "{step}", "status": "{status}", "exit_code": {code}, "detail": "d"}}"#
                )
            })
            .collect();
        let passed = rows.iter().all(|(_, s, _)| *s != "fail");
        format!(
            r#"{{"schema_version": 1, "package": "dettivo-bin-{version}-1-x86_64.pkg.tar.zst", "version": "{version}", "mode": "prefix", "session": false, "passed": {passed}, "steps": [{}]}}"#,
            steps.join(", ")
        )
    }

    fn complete() -> Vec<(&'static str, &'static str, i64)> {
        REQUIRED_CHECKS.iter().map(|c| (*c, "pass", 0)).collect()
    }

    fn why(rows: &[(&str, &str, i64)]) -> String {
        judge(&receipt("0.1.0", rows), "0.1.0").unwrap().unwrap()
    }

    #[test]
    fn a_complete_receipt_passes_and_a_failing_one_names_the_checks() {
        assert_eq!(
            judge(&receipt("0.1.0", &complete()), "0.1.0").unwrap(),
            None
        );
        let mut rows = complete();
        rows[2] = ("units", "fail", 1);
        rows[7] = ("render", "fail", 2);
        let why = why(&rows);
        assert!(why.contains("units (exit 1): d"), "{why}");
        assert!(why.contains("render (exit 2): d"), "{why}");
        assert!(why.contains("passed=false"), "{why}");
        assert!(judge("not json", "0.1.0").is_err());
        assert!(judge(r#"{"passed": true}"#, "0.1.0").is_err());
    }

    #[test]
    fn a_receipt_without_evidence_is_refused() {
        let empty = r#"{"schema_version": 1, "package": "dettivo-bin-0.1.0-1-x86_64.pkg.tar.zst", "version": "0.1.0", "mode": "prefix", "passed": true, "steps": []}"#;
        let w = judge(empty, "0.1.0").unwrap().unwrap();
        assert!(
            w.contains("files: no row") && w.contains("dictation: no row"),
            "{w}"
        );
        let mut rows = complete();
        rows.retain(|(c, _, _)| *c != "dictation");
        assert_eq!(why(&rows), "dictation: no row");
        let mut rows = complete();
        rows[0] = ("files", "unknown", 0);
        assert!(why(&rows).contains("files: status \"unknown\""));
        let mut rows = complete();
        rows[0] = ("files", "pass", 3);
        assert!(why(&rows).contains("files: passed with exit 3"));
        let mut rows = complete();
        rows[5] = ("completions_zsh", "skip", 0);
        assert!(why(&rows).contains("completions_zsh: skipped, d"));
        let mut rows = complete();
        rows.push(("files", "pass", 0));
        assert!(why(&rows).contains("files: 2 rows"));
        let w = judge(&receipt("0.0.9", &complete()), "0.1.0")
            .unwrap()
            .unwrap();
        assert!(w.contains("version 0.0.9 rather than 0.1.0"), "{w}");
        assert!(w.contains("not a 0.1.0 package"), "{w}");
        let mut rows = complete();
        rows.push(("session_osd", "fail", 1));
        assert!(why(&rows).contains("session_osd (exit 1)"));
    }

    #[test]
    fn a_missing_report_is_missing_proof() {
        let dir = tempfile::tempdir().unwrap();
        let step = Step {
            id: "install_test",
            kind: super::super::StepKind::Packaging,
            driver: None,
            expect: super::super::Expect::Pass,
            surface: None,
            contributes: "install-test.json",
        };
        let r = judge_file(&step, dir.path(), &dir.path().join("none.json"), "0.1.0");
        assert_eq!(r.outcome, StepOutcome::Fail);
        assert!(
            r.reason.as_deref().unwrap().contains("unproven"),
            "{:?}",
            r.reason
        );
    }

    #[test]
    fn the_report_path_honours_the_environment() {
        let default = report_path(Path::new("/repo"));
        assert!(default.ends_with("build/install-test.json"));
    }
}
