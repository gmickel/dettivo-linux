//! Runs a pack for real: the preflight (binaries and their profile, a
//! daemon's `system.capabilities`, the doctor's checks), one step after
//! another through the scenario runner, a repository command or the WER
//! check under one run directory, the per-route scans after every
//! scenario step, then the report.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::report::{self, Blocker, Machine};
use super::wer::{self, Refusal};
use super::{Pack, RunOptions, Step, StepKind, StepOutcome, StepResult, gui, scan};
use crate::doctor;
use crate::evidence::{Evidence, Outcome};
use crate::profile::Profile;
use crate::runner;
use crate::scenarios::{self, daemon::DaemonHandle};

/// The `platform` block from a fresh daemon in a throwaway profile.
pub(crate) fn platform(repo_root: &Path, timeout: Duration) -> Result<Value, String> {
    let dettivod = scenarios::binary(repo_root, "dettivod")?;
    let mut profile = Profile::create("pack", None).map_err(|e| format!("profile: {e}"))?;
    // The real insertion chain, so the platform block names the backend
    // the machine's desktop gets rather than the profile's mock.
    let real_chain = BTreeMap::from([("DETTIVO_MOCK_INSERT".to_string(), "0".to_string())]);
    let daemon = DaemonHandle::spawn(&dettivod, &mut profile, "", &real_chain, timeout)?;
    let caps = daemon.call("system.capabilities", json!({}))?;
    Ok(caps["platform"].clone())
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn result(step: &Step, driver: &str, outcome: StepOutcome, reason: Option<String>) -> StepResult {
    StepResult {
        id: step.id.into(),
        driver: driver.into(),
        outcome,
        duration_ms: 0,
        evidence: None,
        reason,
        surface: step.surface.map(str::to_string),
        scan: None,
    }
}

/// The per-route scans over what a scenario step captured: a finding
/// turns a pass into a failure naming the route and the element, and a
/// failed step keeps its own reason with the findings appended.
fn scan_step(run_dir: &Path, result: &mut StepResult) {
    let Some(dir) = result.evidence.as_ref().map(|e| run_dir.join(e)) else {
        return;
    };
    if !dir.is_dir() {
        return;
    }
    match scan::scan_dir(&dir) {
        Ok(scan) => {
            let mut why = scan.failure();
            // A GUI step (one that belongs to a surface) proves its screens
            // through the trees it captured; a pass with none is not
            // inspected, which is not the same as clean.
            if why.is_none()
                && result.surface.is_some()
                && result.outcome == StepOutcome::Pass
                && scan.routes.is_empty()
            {
                why = Some(format!(
                    "no accessibility tree was captured under {}; the surface was not inspected",
                    dir.display()
                ));
            }
            if let Some(why) = why {
                result.outcome = StepOutcome::Fail;
                result.reason = Some(match result.reason.take() {
                    Some(own) => format!("{own}\n{why}"),
                    None => why,
                });
            }
            result.scan = Some(scan);
        }
        Err(e) => {
            result.outcome = StepOutcome::Fail;
            result.reason = Some(format!("scan {}: {e}", dir.display()));
        }
    }
}

fn scenario_step(
    step: &Step,
    driver: &str,
    run_dir: &Path,
    opts: &RunOptions,
    drive_blocker: Option<&str>,
) -> StepResult {
    let Some(scenario) = scenarios::by_id(step.id) else {
        return result(
            step,
            driver,
            StepOutcome::Fail,
            Some(format!("{} is not a scenario", step.id)),
        );
    };
    // A desktop piece the preflight found missing or dead skips every
    // step that drives a window, named up front, instead of letting a
    // driver fail against it mid-scenario.
    if let (true, Some(why)) = (scenario.needs_driver(), drive_blocker) {
        return result(
            step,
            driver,
            StepOutcome::Skip,
            Some(format!("preflight: {why}")),
        );
    }
    let options = runner::Options {
        driver: driver.to_string(),
        evidence_base: opts.evidence_base.clone(),
        models_dir: opts.models_dir.clone(),
        keep_profile: false,
        timeout: opts.timeout,
        run_dir: Some(run_dir.to_path_buf()),
        env: opts.meetings.scenario_env(),
    };
    let started = Instant::now();
    let evidence = format!("{}.{}", step.id, runner::evidence_driver_name(driver));
    let mut out = match runner::drive(&opts.repo_root, scenario.as_ref(), &options) {
        Ok(r) => StepResult {
            outcome: match r.outcome {
                Outcome::Pass => StepOutcome::Pass,
                Outcome::Skip => StepOutcome::Skip,
                Outcome::Fail => StepOutcome::Fail,
            },
            duration_ms: r.duration_ms.max(elapsed_ms(started)),
            evidence: Some(evidence),
            ..result(step, driver, StepOutcome::Fail, r.reason)
        },
        Err(e) => StepResult {
            duration_ms: elapsed_ms(started),
            evidence: Some(evidence),
            ..result(
                step,
                driver,
                StepOutcome::Fail,
                Some(format!("runner: {e}")),
            )
        },
    };
    scan_step(run_dir, &mut out);
    out
}

/// A repository command from `gui::command_for`, run from the root with
/// its output kept under `<run>/<id>/output.log`; exit 0 passes, any
/// other exit fails naming the code and the last lines.
fn command_step(step: &Step, run_dir: &Path, opts: &RunOptions) -> StepResult {
    let started = Instant::now();
    let Some((program, args)) = gui::command_for(step.id) else {
        return result(
            step,
            "none",
            StepOutcome::Fail,
            Some(format!("{} has no command", step.id)),
        );
    };
    let dir = run_dir.join(step.id);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return result(
            step,
            "none",
            StepOutcome::Fail,
            Some(format!("{}: {e}", dir.display())),
        );
    }
    // A repository script runs from the root; a bare tool name (`cargo`)
    // resolves on PATH.
    let program_path = if program.contains('/') {
        opts.repo_root.join(program)
    } else {
        PathBuf::from(program)
    };
    let output = Command::new(program_path)
        .args(&args)
        .current_dir(&opts.repo_root)
        .stdin(Stdio::null())
        .output();
    let mut out = result(step, "none", StepOutcome::Fail, None);
    out.evidence = Some(step.id.into());
    match output {
        Ok(o) => {
            let text = format!(
                "$ {program} {}\n{}{}",
                args.join(" "),
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            let _ = std::fs::write(dir.join("output.log"), &text);
            if o.status.success() {
                out.outcome = StepOutcome::Pass;
                out.reason = text
                    .lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .map(str::to_string);
            } else {
                let tail: Vec<&str> = text.lines().rev().take(5).collect::<Vec<_>>();
                out.reason = Some(format!(
                    "{program} exited {}: {}",
                    o.status.code().unwrap_or(-1),
                    tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
                ));
            }
        }
        Err(e) => out.reason = Some(format!("{program}: {e}")),
    }
    out.duration_ms = elapsed_ms(started);
    out
}

fn wer_step(step: &Step, run_dir: &Path, opts: &RunOptions) -> StepResult {
    let dir = run_dir.join(step.id);
    let started = Instant::now();
    let mut out = result(step, "none", StepOutcome::Fail, None);
    out.evidence = Some(step.id.into());
    if let Err(e) = std::fs::create_dir_all(&dir) {
        out.reason = Some(format!("{}: {e}", dir.display()));
        return out;
    }
    // CI has no GPU and the engine's CPU figure is the one its own test
    // pins; a Vulkan machine leaves the backend to the engine.
    let force_cpu =
        std::env::var_os("CI").is_some() || std::env::var_os("DETTIVO_FORCE_CPU").is_some();
    match wer::run(&opts.repo_root, opts.models_dir.as_deref(), force_cpu, &dir) {
        Ok(w) => {
            out.outcome = StepOutcome::Pass;
            out.reason = Some(format!(
                "WER {:.3} under {:.2} on {}",
                w.rate, w.threshold, w.backend
            ));
        }
        Err(Refusal::Skip(why)) => {
            out.outcome = StepOutcome::Skip;
            out.reason = Some(why);
        }
        Err(Refusal::Fail(why)) => out.reason = Some(why),
    }
    out.duration_ms = elapsed_ms(started);
    out
}

fn elapsed_ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Runs `pack` and writes the report; the exit code.
pub fn run(pack: &Pack, opts: &RunOptions, json: bool) -> u8 {
    let started_unix = unix_now();
    let started = Instant::now();
    let run_dir: PathBuf = match Evidence::new_run(&opts.evidence_base) {
        Ok(e) => e.run_dir.join(format!("pack-{}", pack.name)),
        Err(e) => {
            eprintln!("pack {}: evidence directory: {e}", pack.name);
            return 2;
        }
    };
    if let Err(e) = std::fs::create_dir_all(&run_dir) {
        eprintln!("pack {}: {}: {e}", pack.name, run_dir.display());
        return 2;
    }
    let platform = match platform(&opts.repo_root, opts.timeout.max(Duration::from_secs(10))) {
        Ok(p) => p,
        Err(why) => {
            eprintln!("pack {}: preflight: {why}", pack.name);
            return 2;
        }
    };
    let machine = Machine::from_platform(&platform);
    let binaries = report::binaries(&opts.repo_root);
    // The desktop's missing pieces are blockers the report names up
    // front, and every step that drives a window skips with that reason.
    let checks = doctor::checks();
    let drive_blocker = doctor::drive_ready(&checks).err();
    let preflight: Vec<Blocker> = checks
        .iter()
        .filter(|c| {
            !c.ok && ["x11 display", "session bus", "accessibility bus"].contains(&c.name.as_str())
        })
        .map(|c| Blocker {
            step: "preflight".into(),
            driver: "-".into(),
            reason: format!("{}: {}", c.name, c.detail),
        })
        .collect();
    if !json {
        println!(
            "pack {}: {} ({}), evidence {}",
            pack.name,
            machine.hostname,
            machine.compositor.as_deref().unwrap_or("no compositor"),
            run_dir.display()
        );
        for (name, info) in &binaries {
            println!("binary   {name:<12} {:<8} {}", info.profile, info.path);
        }
        for b in &preflight {
            println!("blocker  {}", b.reason);
        }
    }

    let mut exec = |step: &Step, driver: &str| -> StepResult {
        let result = match step.kind {
            StepKind::Scenario => {
                scenario_step(step, driver, &run_dir, opts, drive_blocker.as_deref())
            }
            StepKind::Command => command_step(step, &run_dir, opts),
            StepKind::WhisperWer => wer_step(step, &run_dir, opts),
            StepKind::ContractStrict => super::gate::contract_step(step, &run_dir, opts),
            StepKind::Packaging => super::gate::packaging_step(step, &run_dir, opts),
            StepKind::DiarizationDer
            | StepKind::MeetingThroughput
            | StepKind::DiarizationThroughput
            | StepKind::GpuWorkloadProof => super::meeting_steps::run(step, &run_dir, opts),
        };
        if !json {
            println!(
                "{:<8} {:<30} {:<6} {}ms{}",
                result.outcome.as_str(),
                result.id,
                result.driver,
                result.duration_ms,
                result
                    .reason
                    .as_deref()
                    .map(|x| format!("  {}", x.replace('\n', " ")))
                    .unwrap_or_default()
            );
        }
        result
    };
    let steps = super::run_steps(pack, &opts.driver, opts.continue_after_failure, &mut exec);
    if !json {
        for r in steps.iter().filter(|r| r.outcome == StepOutcome::NotRun) {
            println!(
                "{:<8} {:<30} {:<6} 0ms  {}",
                "not_run",
                r.id,
                r.driver,
                r.reason.as_deref().unwrap_or("")
            );
        }
    }
    let report = report::build(
        pack,
        machine,
        &run_dir,
        steps,
        preflight,
        binaries,
        (started_unix, elapsed_ms(started)),
    );
    match super::markdown::write(&report, &run_dir) {
        Ok((json_path, md_path)) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                );
            } else {
                for s in &report.surfaces {
                    println!(
                        "surface  {:<12} {:<7} {} routes, {} findings, a11y {} ({} of {})",
                        s.name,
                        if s.passed { "passed" } else { "FAILED" },
                        s.negative_text.routes,
                        s.negative_text.findings.len(),
                        s.a11y_coverage
                            .map(|c| format!("{c:.3}"))
                            .unwrap_or_else(|| "not inspected".into()),
                        s.a11y_named,
                        s.a11y_interactive
                    );
                }
                println!(
                    "pack {}: {} ({} blockers); report {} and {}",
                    pack.name,
                    if report.passed { "passed" } else { "FAILED" },
                    report.blockers.len(),
                    json_path.display(),
                    md_path.display()
                );
            }
        }
        Err(e) => {
            eprintln!("pack {}: report: {e}", pack.name);
            return 1;
        }
    }
    // `--record`: the meeting figures into the checked-in benchmark
    // report and its README (ADR 0039), after the pack's own report.
    if opts.meetings.record && pack.name == super::meetings::NAME {
        match super::meeting_measure::record(&opts.repo_root, &report) {
            Ok(path) => {
                if !json {
                    println!("recorded {}", path.display());
                }
            }
            Err(e) => {
                eprintln!("pack {}: --record: {e}", pack.name);
                return 1;
            }
        }
    }
    u8::from(!report.passed)
}
