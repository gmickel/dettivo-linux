//! The release gate's command steps (`release::STEP_IDS`): each runs a
//! whole command and embeds its output in the step's row; the checks
//! over checked-in files, the tag and the workflow runs are in
//! `release_checks`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use serde_json::{Value, json};

use super::release::{Context, HEADSET_RECEIPT, StepReport, blocker, elapsed_ms};
use super::{StepOutcome, gate};

pub(super) fn row(id: &str, command: String) -> StepReport {
    StepReport {
        id: id.to_string(),
        outcome: StepOutcome::Fail,
        duration_ms: 0,
        command,
        exit_code: None,
        reason: None,
        evidence: None,
        report: None,
        blockers: Vec::new(),
    }
}

/// The output of one command: exit code, stdout, stderr, with both
/// streams kept under `<run>/<step>/output.log`.
pub(super) struct Ran {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub(super) fn run_command(
    ctx: &Context<'_>,
    id: &str,
    program: &Path,
    args: &[String],
) -> Result<Ran, String> {
    let dir = ctx.run_dir.join(id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let output = Command::new(program)
        .args(args)
        .env(
            crate::scenarios::PROFILE_ENV,
            crate::scenarios::build_profile(),
        )
        .current_dir(&ctx.opts.repo_root)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{}: {e}", program.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let _ = std::fs::write(
        dir.join("output.log"),
        format!(
            "$ {} {}\n{stdout}{stderr}",
            program.display(),
            args.join(" ")
        ),
    );
    Ok(Ran {
        code: output.status.code(),
        stdout,
        stderr,
    })
}

fn last_line(text: &str) -> Option<String> {
    text.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(str::to_string)
}

pub(super) fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().rev().take(n).collect();
    lines.into_iter().rev().collect::<Vec<_>>().join(" | ")
}

/// The steps that are this binary's own verbs with `--json`: the command
/// runs, its stdout is parsed as the embedded report, exit 0 passes.
fn own_verb(ctx: &Context<'_>, id: &str, args: &[&str], skip_ok: bool) -> StepReport {
    let started = Instant::now();
    let mut args: Vec<String> = args.iter().map(|a| (*a).to_string()).collect();
    args.push("--json".into());
    args.push("--repo".into());
    args.push(ctx.opts.repo_root.display().to_string());
    let mut out = row(id, format!("dettivo-qa {}", args.join(" ")));
    out.evidence = Some(id.to_string());
    match run_command(ctx, id, &ctx.exe, &args) {
        Ok(ran) => {
            out.exit_code = ran.code;
            out.report = serde_json::from_str(&ran.stdout).ok();
            if ran.code == Some(0) {
                out.outcome = StepOutcome::Pass;
                out.reason = last_line(&ran.stderr).or_else(|| Some("exit 0".into()));
            } else if skip_ok && ran.code == Some(2) {
                out.outcome = StepOutcome::Skip;
                out.reason = Some(tail(&ran.stderr, 3));
            } else {
                out.reason = Some(format!(
                    "exit {}: {}",
                    ran.code.unwrap_or(-1),
                    tail(&format!("{}{}", ran.stdout, ran.stderr), 5)
                ));
            }
        }
        Err(e) => out.reason = Some(e),
    }
    out.duration_ms = elapsed_ms(started);
    out
}

/// A pack of scenarios through this binary: the pack's report is
/// embedded and its blockers join the gate's, each classified.
fn sub_pack(ctx: &Context<'_>, id: &str, name: &str) -> StepReport {
    // `--continue`: every row runs, so a failed row leaves the other
    // driver's rows and the later steps in the report.
    let mut args = vec![
        "pack",
        name,
        "--continue",
        "--driver",
        ctx.opts.driver.as_str(),
        "--out",
    ];
    let out_dir = ctx.opts.evidence_base.display().to_string();
    args.push(&out_dir);
    let timeout = ctx.opts.timeout.as_millis().to_string();
    args.push("--timeout-ms");
    args.push(&timeout);
    let engines = ctx
        .opts
        .meetings
        .engines_dir
        .as_ref()
        .map(|d| d.display().to_string());
    if name == "meetings" {
        if let Some(e) = &engines {
            args.push("--engines");
            args.push(e);
        }
        if ctx.opts.meetings.cpu {
            args.push("--cpu");
        }
        if ctx.opts.meetings.record {
            args.push("--record");
        }
    }
    let mut out = own_verb(ctx, id, &args, false);
    if let Some(report) = out.report.clone() {
        judge_pack(id, &report, &mut out);
    }
    out
}

/// A pack's report decides its step: the pack's blockers join the gate's,
/// each classified by its own reason, and a failed row fails the step
/// whichever driver it ran on (the report's `passed` is false then, or
/// when a must-pass row skipped or never ran).
pub(super) fn judge_pack(id: &str, report: &Value, out: &mut StepReport) {
    if let Some(run_dir) = report["run_dir"].as_str() {
        out.evidence = Some(run_dir.to_string());
    }
    if let Some(blockers) = report["blockers"].as_array() {
        for b in blockers {
            let step = b["step"].as_str().unwrap_or("?");
            let driver = b["driver"].as_str().unwrap_or("-");
            let reason = b["reason"].as_str().unwrap_or("skipped");
            out.blockers.push(blocker(
                &format!("{id}/{step}"),
                &format!("{reason} (driver {driver})"),
            ));
        }
    }
    let failed: Vec<String> = report["steps"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter(|r| r["outcome"] == "fail")
                .map(|r| {
                    format!(
                        "{} ({}): {}",
                        r["id"].as_str().unwrap_or("?"),
                        r["driver"].as_str().unwrap_or("-"),
                        r["reason"].as_str().unwrap_or("failed")
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    if report["passed"] == Value::Bool(true) && failed.is_empty() {
        out.outcome = StepOutcome::Pass;
    } else {
        out.outcome = StepOutcome::Fail;
        out.reason = Some(if failed.is_empty() {
            "the pack failed: a must-pass row skipped or never ran; the blockers name it"
                .to_string()
        } else {
            format!("the pack failed: {}", failed.join("; "))
        });
    }
}

fn from_gate(
    mut result: super::StepResult,
    command: &str,
    run_dir: &Path,
    file: &str,
) -> StepReport {
    let mut out = row(&result.id, command.to_string());
    out.outcome = result.outcome;
    out.duration_ms = result.duration_ms;
    out.reason = result.reason.take();
    out.evidence = result.evidence.clone();
    if let Some(e) = &result.evidence {
        out.report = std::fs::read_to_string(run_dir.join(e).join(file))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok());
    }
    if out.outcome == StepOutcome::Skip {
        out.blockers.push(blocker(
            &result.id,
            out.reason.as_deref().unwrap_or("skipped"),
        ));
    }
    out
}

fn install_test(ctx: &Context<'_>) -> StepReport {
    let step = super::Step {
        id: "install_test",
        kind: super::StepKind::Packaging,
        driver: None,
        expect: super::Expect::Pass,
        surface: None,
        contributes: "install-test.json",
    };
    let result = gate::packaging_step(&step, &ctx.run_dir, ctx.opts);
    let mut out = from_gate(
        result,
        &format!(
            "install-test.json from {}",
            gate::report_path(&ctx.opts.repo_root).display()
        ),
        &ctx.run_dir,
        "install-test.json",
    );
    if out.outcome == StepOutcome::Pass && out.reason.is_none() {
        let steps = out
            .report
            .as_ref()
            .and_then(|r| r["steps"].as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        out.reason = Some(format!(
            "every one of {steps} checks passed in {}",
            gate::report_path(&ctx.opts.repo_root).display()
        ));
    }
    let session = out
        .report
        .as_ref()
        .and_then(|r| r["steps"].as_array())
        .is_some_and(|steps| {
            steps.iter().any(|s| {
                s["step"]
                    .as_str()
                    .is_some_and(|n| n.starts_with("session_"))
            })
        });
    if !session && out.outcome == StepOutcome::Pass {
        out.blockers.push(blocker(
            "install_test",
            "the report is a prefix install on this desktop; `scripts/packaging/install-test.sh --session` on a fresh CPU-only VM has not been run for this version",
        ));
    }
    out
}

fn real_headset(ctx: &Context<'_>) -> StepReport {
    let mut out = own_verb(ctx, "real_headset", &["audio-check"], false);
    out.command = format!("dettivo-qa audio-check --json; {HEADSET_RECEIPT}");
    let receipt_path = ctx.opts.repo_root.join(HEADSET_RECEIPT);
    let receipt: Option<Value> = std::fs::read_to_string(&receipt_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let rig = out.report.take();
    match receipt {
        Some(r) if r["version"] == Value::String(ctx.version.clone()) => {
            out.report = Some(json!({ "rig": rig, "receipt": r }));
        }
        Some(r) => {
            out.report = Some(json!({ "rig": rig, "receipt": r }));
            if out.outcome == StepOutcome::Pass {
                out.outcome = StepOutcome::Skip;
            }
            out.blockers.push(blocker(
                "real_headset",
                &format!("{HEADSET_RECEIPT} records version {} rather than {}; a person dictates once through the real microphone on this desktop and records the receipt again", r["version"], ctx.version),
            ));
        }
        None => {
            out.report = Some(json!({ "rig": rig, "receipt": Value::Null }));
            if out.outcome == StepOutcome::Pass {
                out.outcome = StepOutcome::Skip;
            }
            out.blockers.push(blocker(
                "real_headset",
                &format!("{HEADSET_RECEIPT} is missing: the virtual rig round trip ran, and a person dictates once through the real microphone on this desktop and records the receipt (version, date, device, the words, the backend)"),
            ));
        }
    }
    out
}

fn script(ctx: &Context<'_>, id: &str, program: &str, args: &[&str]) -> StepReport {
    let started = Instant::now();
    let args: Vec<String> = args.iter().map(|a| (*a).to_string()).collect();
    let mut out = row(id, format!("{program} {}", args.join(" ")));
    out.evidence = Some(id.to_string());
    let path = if program.contains('/') {
        ctx.opts.repo_root.join(program)
    } else {
        PathBuf::from(program)
    };
    match run_command(ctx, id, &path, &args) {
        Ok(ran) => {
            out.exit_code = ran.code;
            let text = format!("{}{}", ran.stdout, ran.stderr);
            out.report =
                Some(json!({ "exit_code": ran.code, "lines": text.lines().collect::<Vec<_>>() }));
            if ran.code == Some(0) {
                out.outcome = StepOutcome::Pass;
                out.reason = last_line(&text);
            } else {
                out.reason = Some(format!(
                    "exit {}: {}",
                    ran.code.unwrap_or(-1),
                    tail(&text, 5)
                ));
            }
        }
        Err(e) => out.reason = Some(e),
    }
    out.duration_ms = elapsed_ms(started);
    out
}

fn contract_mcp(ctx: &Context<'_>) -> StepReport {
    let mut out = row(
        "mcp_harness",
        "MCP section of contract_strict/contract-replay.json".into(),
    );
    out.evidence = Some("contract_strict".into());
    let path = ctx.run_dir.join("contract_strict/contract-replay.json");
    let report = std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|text| {
            serde_json::from_str::<crate::replay::Report>(&text).map_err(|e| e.to_string())
        });
    match report {
        Ok(report) => match crate::contract::validate_required(&report) {
            Ok(()) => {
                let pass = report
                    .mcp
                    .iter()
                    .all(|r| r.verdict != dettivo_mcp::harness::Verdict::Fail);
                out.outcome = if pass {
                    StepOutcome::Pass
                } else {
                    StepOutcome::Fail
                };
                out.exit_code = Some(i32::from(!pass));
                out.reason = Some("MCP results reused from the shared contract run".into());
                out.report = serde_json::to_value(&report.mcp).ok();
            }
            Err(why) => out.reason = Some(why),
        },
        Err(why) => out.reason = Some(format!("{}: {why}", path.display())),
    }
    out
}

/// Runs one step by id.
pub fn run_step(id: &str, ctx: &Context<'_>) -> StepReport {
    match id {
        "version_match" => super::release_checks::version_match(ctx),
        "ci_green" => super::release_checks::ci_green(ctx),
        "contract_strict" => {
            let step = super::Step {
                id: "contract_strict",
                kind: super::StepKind::ContractStrict,
                driver: None,
                expect: super::Expect::Pass,
                surface: None,
                contributes: "contract replay",
            };
            let result = gate::contract_step(&step, &ctx.run_dir, ctx.opts);
            from_gate(
                result,
                "dettivo-qa contract --strict",
                &ctx.run_dir,
                "contract-replay.json",
            )
        }
        "mcp_harness" => contract_mcp(ctx),
        "pack_dictation" => sub_pack(ctx, id, "dictation"),
        "pack_gui" => sub_pack(ctx, id, "gui"),
        "pack_meetings" => sub_pack(ctx, id, "meetings"),
        "bench_report" => super::release_bench::bench_report(ctx),
        "install_test" => install_test(ctx),
        "evidence_map" => own_verb(ctx, id, &["evidence-map"], false),
        "docs_build" => script(ctx, id, "scripts/check-docs.sh", &[]),
        "notice_lint" => script(
            ctx,
            id,
            "cargo",
            &["run", "-q", "-p", "xtask", "--", "lint-notice"],
        ),
        "real_headset" => real_headset(ctx),
        other => {
            let mut out = row(other, String::new());
            out.reason = Some(format!("{other} is not a step"));
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_row_that_fails_on_one_driver_fails_the_step_whatever_the_other_did() {
        let report = json!({
            "run_dir": "/r/pack-meetings",
            "passed": false,
            "blockers": [{"step": "rig", "driver": "none", "reason": "no PipeWire on this machine"}],
            "steps": [
                {"id": "meetings_seeded", "driver": "atspi", "outcome": "pass"},
                {"id": "meetings_seeded", "driver": "cua", "outcome": "fail", "reason": "the renamed speaker: not found"},
            ],
        });
        let mut out = row("pack_meetings", String::new());
        out.exit_code = Some(1);
        judge_pack("pack_meetings", &report, &mut out);
        assert_eq!(out.outcome, StepOutcome::Fail);
        assert_eq!(
            out.reason.as_deref(),
            Some("the pack failed: meetings_seeded (cua): the renamed speaker: not found")
        );
        assert_eq!(out.evidence.as_deref(), Some("/r/pack-meetings"));
        assert_eq!(
            out.blockers.len(),
            1,
            "no blocker is invented for the failed row"
        );
        assert_eq!(out.blockers[0].step, "pack_meetings/rig");
        assert!(!out.blockers[0].external);
        let clean = json!({"passed": true, "blockers": [], "steps": [
            {"id": "meetings_seeded", "driver": "atspi", "outcome": "pass"},
            {"id": "meetings_seeded", "driver": "cua", "outcome": "skip", "reason": "missing: cua-driver"},
        ]});
        let mut out = row("pack_meetings", String::new());
        judge_pack("pack_meetings", &clean, &mut out);
        assert_eq!(out.outcome, StepOutcome::Pass);
        let skipped = json!({"passed": false, "blockers": [], "steps": []});
        let mut out = row("pack_gui", String::new());
        judge_pack("pack_gui", &skipped, &mut out);
        assert_eq!(out.outcome, StepOutcome::Fail);
        assert!(out.reason.as_deref().unwrap().contains("must-pass row"));
    }
}
