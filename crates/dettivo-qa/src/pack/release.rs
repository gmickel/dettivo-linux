//! The release gate as one script (QA-9, ADR 0017 over whole commands):
//! `dettivo-qa pack release` runs every named step below in order, embeds
//! each step's own report, and ends with the blockers list and the
//! `external_blockers` list that makes "only external blockers remain"
//! a claim the report proves or refutes. `release_steps` runs each step,
//! `release_report` renders the Markdown and files the record under
//! `docs/reports/release-gate/<version>.json` on `--record`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::report::{BinaryInfo, Machine};
use super::{RunOptions, StepOutcome};
use crate::evidence::Evidence;

pub use super::release_blockers::{blocker, external_need};

/// The steps, in run order.
pub const STEP_IDS: &[&str] = &[
    "version_match",
    "ci_green",
    "contract_strict",
    "mcp_harness",
    "pack_dictation",
    "pack_gui",
    "pack_meetings",
    "bench_report",
    "install_test",
    "evidence_map",
    "docs_build",
    "notice_lint",
    "real_headset",
];

/// The pack's name.
pub const NAME: &str = "release";
/// Where `--record` files the run, relative to the repository root.
pub const RECORD_DIR: &str = "docs/reports/release-gate";
/// The receipt a person writes after one real-microphone dictation.
pub const HEADSET_RECEIPT: &str = "docs/reports/release-gate/real-headset.json";

/// What a step could not do, and whether the cause lies outside this
/// machine and this repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blocker {
    /// The step (`<step>/<inner step>` for a pack's own blocker).
    pub step: String,
    /// Why.
    pub reason: String,
    /// True when the cause is external: the CPU-only VM, a tool that is
    /// not installed, a workflow GitHub has disabled, a person.
    pub external: bool,
    /// What would lift it, for an external blocker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs: Option<String>,
}

/// One step's row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepReport {
    /// The step id.
    pub id: String,
    /// The outcome.
    pub outcome: StepOutcome,
    /// Wall time.
    pub duration_ms: u64,
    /// The command that ran, for a person to repeat.
    pub command: String,
    /// The command's exit code, when one ran.
    pub exit_code: Option<i32>,
    /// Why, when not a pass; the last line otherwise.
    pub reason: Option<String>,
    /// The evidence directory relative to the run directory.
    pub evidence: Option<String>,
    /// The step's own report, embedded.
    pub report: Option<Value>,
    /// The blockers the step raised.
    #[serde(default)]
    pub blockers: Vec<Blocker>,
}

/// The whole report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// 1.
    pub schema_version: u32,
    /// `release`.
    pub pack: String,
    /// `Cargo.toml`'s version.
    pub version: String,
    /// The commit.
    pub git_sha: String,
    /// The machine.
    pub machine: Machine,
    /// The binaries the packs drove.
    pub binaries: std::collections::BTreeMap<String, BinaryInfo>,
    /// The run directory.
    pub run_dir: String,
    /// Unix seconds.
    pub started_unix: u64,
    /// Wall time.
    pub duration_ms: u64,
    /// True when every step passed or skipped for an external reason and
    /// no blocker is unexplained.
    pub passed: bool,
    /// The steps `--skip` left out.
    pub skipped: Vec<String>,
    /// One row per step.
    pub steps: Vec<StepReport>,
    /// Every blocker.
    pub blockers: Vec<Blocker>,
    /// The blockers whose cause is outside this machine and repository.
    pub external_blockers: Vec<Blocker>,
    /// The blockers that are not external: each one is fixed, never shipped.
    pub unexplained_blockers: Vec<Blocker>,
}

/// The gate's own switches.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Steps to leave out, recorded as skipped.
    pub skip: Vec<String>,
    /// File the run under `docs/reports/release-gate/<version>.json`.
    pub record: bool,
}

/// The facts every step reads.
pub struct Context<'a> {
    /// The run options.
    pub opts: &'a RunOptions,
    /// The release options.
    pub release: &'a Options,
    /// The run directory.
    pub run_dir: PathBuf,
    /// The version from `Cargo.toml`.
    pub version: String,
    /// The commit.
    pub git_sha: String,
    /// The host name.
    pub hostname: String,
    /// This binary.
    pub exe: PathBuf,
}

pub(crate) fn elapsed_ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The version in the workspace `Cargo.toml`.
pub fn workspace_version(root: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(root.join("Cargo.toml")).map_err(|e| e.to_string())?;
    text.lines()
        .find_map(|l| l.trim().strip_prefix("version = "))
        .map(|v| v.trim().trim_matches('"').to_string())
        .ok_or_else(|| "Cargo.toml has no `version = ` line".to_string())
}

fn git_sha(root: &Path) -> String {
    crate::bench::host::git_sha(root)
}

/// Runs the gate; the exit code (0 passed, 1 a failed step or an
/// unexplained blocker, 2 a preflight refusal, which a binary set that
/// is not the release build of this commit is).
pub fn run(opts: &RunOptions, release: &Options, json: bool) -> u8 {
    let started_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let started = Instant::now();
    for s in &release.skip {
        if !STEP_IDS.contains(&s.as_str()) {
            eprintln!(
                "pack release: --skip {s:?} is not a step; the steps are {}",
                STEP_IDS.join(", ")
            );
            return 2;
        }
    }
    let run_dir = match Evidence::new_run(&opts.evidence_base) {
        Ok(e) => e.run_dir.join(format!("pack-{NAME}")),
        Err(e) => {
            eprintln!("pack release: evidence directory: {e}");
            return 2;
        }
    };
    if let Err(e) = std::fs::create_dir_all(&run_dir) {
        eprintln!("pack release: {}: {e}", run_dir.display());
        return 2;
    }
    let version = match workspace_version(&opts.repo_root) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pack release: {e}");
            return 2;
        }
    };
    let machine = match super::execute::platform(
        &opts.repo_root,
        opts.timeout.max(Duration::from_secs(10)),
    ) {
        Ok(p) => Machine::from_platform(&p),
        Err(why) => {
            eprintln!("pack release: preflight: {why}");
            return 2;
        }
    };
    // The gate drives the release build of this commit and nothing else:
    // the profile is pinned for this process and every verb it spawns,
    // and a debug or stale binary set is a refusal, never a report.
    crate::scenarios::pin_release_profile();
    let binaries = super::report::binaries(&opts.repo_root);
    let sha = git_sha(&opts.repo_root);
    if let Err(why) = super::report::check_release_binaries(&binaries, &sha) {
        eprintln!("pack release: preflight: {why}");
        return 2;
    }
    let ctx = Context {
        opts,
        release,
        run_dir: run_dir.clone(),
        version: version.clone(),
        git_sha: sha,
        hostname: machine.hostname.clone(),
        exe: std::env::current_exe().unwrap_or_else(|_| PathBuf::from("dettivo-qa")),
    };
    if !json {
        println!(
            "pack release: version {version} at {} on {}, evidence {}",
            &ctx.git_sha[..ctx.git_sha.len().min(12)],
            machine.hostname,
            run_dir.display()
        );
    }
    let mut steps = Vec::new();
    for id in STEP_IDS {
        let row = if release.skip.iter().any(|s| s == id) {
            StepReport {
                id: (*id).to_string(),
                outcome: StepOutcome::Skip,
                duration_ms: 0,
                command: String::new(),
                exit_code: None,
                reason: Some("left out by --skip".into()),
                evidence: None,
                report: None,
                blockers: vec![Blocker {
                    step: (*id).to_string(),
                    reason: "left out by --skip".into(),
                    external: false,
                    needs: None,
                }],
            }
        } else {
            super::release_steps::run_step(id, &ctx)
        };
        if !json {
            println!(
                "{:<8} {:<18} {}ms  {}",
                row.outcome.as_str(),
                row.id,
                row.duration_ms,
                row.reason.as_deref().unwrap_or("").replace('\n', " ")
            );
            for b in &row.blockers {
                println!(
                    "         {} blocker  {}: {}",
                    if b.external {
                        "external"
                    } else {
                        "UNEXPLAINED"
                    },
                    b.step,
                    b.reason.replace('\n', " ")
                );
            }
        }
        steps.push(row);
    }
    let blockers: Vec<Blocker> = steps
        .iter()
        .flat_map(|s| s.blockers.iter().cloned())
        .collect();
    let external_blockers: Vec<Blocker> = blockers.iter().filter(|b| b.external).cloned().collect();
    let unexplained_blockers: Vec<Blocker> =
        blockers.iter().filter(|b| !b.external).cloned().collect();
    let passed =
        steps.iter().all(|s| s.outcome != StepOutcome::Fail) && unexplained_blockers.is_empty();
    let report = Report {
        schema_version: 1,
        pack: NAME.into(),
        version,
        git_sha: ctx.git_sha.clone(),
        machine,
        binaries,
        run_dir: run_dir.display().to_string(),
        started_unix,
        duration_ms: elapsed_ms(started),
        passed,
        skipped: release.skip.clone(),
        steps,
        blockers,
        external_blockers,
        unexplained_blockers,
    };
    match super::release_report::write(&report, &run_dir) {
        Ok((json_path, md_path)) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                );
            } else {
                println!(
                    "pack release: {} ({} blockers, {} external, {} unexplained); report {} and {}",
                    if report.passed { "passed" } else { "FAILED" },
                    report.blockers.len(),
                    report.external_blockers.len(),
                    report.unexplained_blockers.len(),
                    json_path.display(),
                    md_path.display()
                );
            }
        }
        Err(e) => {
            eprintln!("pack release: report: {e}");
            return 1;
        }
    }
    if release.record {
        match super::release_report::record(&report, &opts.repo_root) {
            Ok(path) => {
                if !json {
                    println!("recorded {}", path.display());
                }
            }
            Err(e) => {
                eprintln!("pack release: --record: {e}");
                return 1;
            }
        }
    }
    u8::from(!report.passed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_release_pack_names_every_step_and_the_report_names_the_external_blockers() {
        assert_eq!(STEP_IDS.len(), 13);
        assert_eq!(STEP_IDS[0], "version_match");
        assert_eq!(STEP_IDS[12], "real_headset");
        let vm = blocker(
            "bench_report",
            "the cpu row was measured on this desktop under DETTIVO_FORCE_CPU=1; a CPU-only VM has not run it",
        );
        assert!(vm.external);
        assert!(vm.needs.as_deref().unwrap().contains("virtual machine"));
        let cua = blocker("pack_gui/first_run_fresh", "missing: cua-driver");
        assert!(cua.external);
        let cua_failed = blocker(
            "pack_meetings/meetings_seeded",
            "the renamed speaker: not found (driver cua)",
        );
        assert!(
            !cua_failed.external,
            "a failure on either driver is a failure"
        );
        let live = blocker("pack_dictation/hotkeys_hyprland", "hyprctl is not on PATH");
        assert!(live.external);
        let ours = blocker(
            "pack_gui/history_seeded",
            "the accessibility bus does not answer",
        );
        assert!(!ours.external && ours.needs.is_none());
        let unproven = blocker(
            "install_test",
            "build/install-test.json is missing, so the clean install is unproven; run `just package-bin` then `just install-test` (or install-test.sh --session on the VM)",
        );
        assert!(!unproven.external, "a missing receipt is missing proof");
        let steps = vec![
            StepReport {
                id: "evidence_map".into(),
                outcome: StepOutcome::Pass,
                duration_ms: 1,
                command: "dettivo-qa evidence-map".into(),
                exit_code: Some(0),
                reason: None,
                evidence: Some("evidence_map".into()),
                report: Some(serde_json::json!({"ok": true})),
                blockers: vec![],
            },
            StepReport {
                id: "bench_report".into(),
                outcome: StepOutcome::Pass,
                duration_ms: 1,
                command: String::new(),
                exit_code: None,
                reason: None,
                evidence: None,
                report: None,
                blockers: vec![vm.clone(), ours.clone()],
            },
        ];
        let blockers: Vec<Blocker> = steps.iter().flat_map(|s| s.blockers.clone()).collect();
        let report = Report {
            schema_version: 1,
            pack: NAME.into(),
            version: "0.1.0".into(),
            git_sha: "abc".into(),
            machine: Machine::default(),
            binaries: Default::default(),
            run_dir: "/r".into(),
            started_unix: 1,
            duration_ms: 2,
            passed: false,
            skipped: vec![],
            steps,
            external_blockers: blockers.iter().filter(|b| b.external).cloned().collect(),
            unexplained_blockers: blockers.iter().filter(|b| !b.external).cloned().collect(),
            blockers,
        };
        assert_eq!(report.external_blockers.len(), 1);
        assert_eq!(report.unexplained_blockers.len(), 1);
        let md = super::super::release_report::markdown(&report);
        assert!(md.contains("| `evidence_map` | pass |"), "{md}");
        assert!(md.contains("## External blockers"), "{md}");
        assert!(md.contains("a CPU-only virtual machine"), "{md}");
        assert!(md.contains("## Unexplained blockers"), "{md}");
        let back: Report = serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
        assert_eq!(back, report);
        assert!(
            workspace_version(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../..")
                    .as_path()
            )
            .is_ok()
        );
    }
}
