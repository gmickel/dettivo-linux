//! The release gate's checks over what is already recorded: the version
//! against the changelog and the tag (`version_match`) and the workflow
//! runs for the commit (`ci_green`); the benchmark reports are judged in
//! `release_bench`.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use serde_json::{Value, json};

use super::StepOutcome;
use super::release::{Blocker, Context, StepReport, elapsed_ms};
use super::release_steps::{row, run_command, tail};

pub(super) fn version_match(ctx: &Context<'_>) -> StepReport {
    let started = Instant::now();
    let root = &ctx.opts.repo_root;
    let mut out = row(
        "version_match",
        format!("git tag -l v{}; CHANGELOG.md", ctx.version),
    );
    let changelog = std::fs::read_to_string(root.join("CHANGELOG.md")).unwrap_or_default();
    let section = format!("## [{}]", ctx.version);
    let has_section = changelog
        .lines()
        .any(|l| l.trim_start().starts_with(&section));
    let tag = format!("v{}", ctx.version);
    let tag_sha = Command::new("git")
        .args(["rev-list", "-n", "1", &tag])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());
    let tag_state = match &tag_sha {
        Some(sha) if *sha == ctx.git_sha => format!("{tag} points at this commit"),
        Some(sha) => format!(
            "{tag} points at {} rather than this commit",
            &sha[..sha.len().min(12)]
        ),
        None => format!("{tag} is not created yet; the release workflow tags after this gate"),
    };
    let tag_ok = tag_sha.as_deref().is_none_or(|s| s == ctx.git_sha);
    out.report = Some(json!({
        "version": ctx.version,
        "changelog_section": has_section,
        "tag": tag,
        "tag_sha": tag_sha,
        "git_sha": ctx.git_sha,
    }));
    if has_section && tag_ok {
        out.outcome = StepOutcome::Pass;
        out.reason = Some(format!(
            "Cargo.toml {}, CHANGELOG.md has {section}, {tag_state}",
            ctx.version
        ));
    } else {
        out.reason = Some(format!(
            "{}{}",
            if has_section {
                String::new()
            } else {
                format!("CHANGELOG.md has no `{section}` section; ")
            },
            tag_state
        ));
    }
    out.duration_ms = elapsed_ms(started);
    out
}

pub(super) fn ci_green(ctx: &Context<'_>) -> StepReport {
    let started = Instant::now();
    let args: Vec<String> = [
        "run",
        "list",
        "--commit",
        ctx.git_sha.as_str(),
        "--json",
        "name,status,conclusion,workflowName,url",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    let mut out = row("ci_green", format!("gh {}", args.join(" ")));
    out.evidence = Some("ci_green".into());
    match run_command(ctx, "ci_green", Path::new("gh"), &args) {
        Ok(ran) => {
            out.exit_code = ran.code;
            let runs: Vec<Value> = serde_json::from_str(&ran.stdout).unwrap_or_default();
            let enabled = (ran.code != Some(0) || runs.is_empty())
                .then(|| actions_enabled(ctx))
                .flatten();
            let workflow = ((ran.code != Some(0) || runs.is_empty()) && enabled != Some(false))
                .then(|| ci_workflow(ctx))
                .flatten();
            out.report =
                Some(json!({ "runs": runs, "actions_enabled": enabled, "ci_workflow": workflow }));
            let (outcome, reason, blocker) = judge_runs(
                &ctx.git_sha,
                ran.code,
                &tail(&ran.stderr, 2),
                &runs,
                enabled,
                workflow.as_ref(),
            );
            out.outcome = outcome;
            out.reason = Some(reason);
            out.blockers.extend(blocker);
        }
        Err(e) => {
            out.outcome = StepOutcome::Skip;
            out.blockers.push(Blocker {
                step: "ci_green".into(),
                reason: e.clone(),
                external: true,
                needs: Some("the gh command installed and signed in".into()),
            });
            out.reason = Some(e);
        }
    }
    out.duration_ms = elapsed_ms(started);
    out
}

/// Whether GitHub Actions is enabled for the repository, from the API;
/// `None` when the question could not be answered.
fn actions_enabled(ctx: &Context<'_>) -> Option<bool> {
    let args: Vec<String> = [
        "api",
        "repos/{owner}/{repo}/actions/permissions",
        "--jq",
        ".enabled",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    let ran = run_command(ctx, "ci_green/permissions", Path::new("gh"), &args).ok()?;
    match (ran.code, ran.stdout.trim()) {
        (Some(0), "true") => Some(true),
        (Some(0), "false") => Some(false),
        _ => None,
    }
}

/// The primary CI workflow's API state; an unavailable or malformed
/// response supplies no evidence that CI is disabled.
fn ci_workflow(ctx: &Context<'_>) -> Option<Value> {
    let args: Vec<String> = ["api", "repos/{owner}/{repo}/actions/workflows/ci.yml"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let ran = run_command(ctx, "ci_green/workflow", Path::new("gh"), &args).ok()?;
    if ran.code != Some(0) {
        return None;
    }
    let workflow: Value = serde_json::from_str(&ran.stdout).ok()?;
    Some(json!({"id": workflow["id"], "path": workflow["path"], "state": workflow["state"]}))
}

/// The `ci_green` verdict: pass when every run concluded success, fail
/// when one did not, and without a run for the commit a blocker that is
/// external only when the API confirms the workflows are disabled;
/// otherwise the missing evidence is unexplained.
pub(super) fn judge_runs(
    sha: &str,
    code: Option<i32>,
    stderr: &str,
    runs: &[Value],
    enabled: Option<bool>,
    workflow: Option<&Value>,
) -> (StepOutcome, String, Option<Blocker>) {
    let short = &sha[..sha.len().min(12)];
    let disabled = |why: String| {
        (
            StepOutcome::Skip,
            why.clone(),
            Some(Blocker {
                step: "ci_green".into(),
                reason: why,
                external: true,
                needs: Some(
                    "the GitHub Actions workflows enabled and a run for this commit".into(),
                ),
            }),
        )
    };
    let unexplained = |why: String| {
        (
            StepOutcome::Skip,
            why.clone(),
            Some(Blocker {
                step: "ci_green".into(),
                reason: why,
                external: false,
                needs: None,
            }),
        )
    };
    let bad: Vec<String> = runs
        .iter()
        .filter(|r| r["conclusion"] != "success")
        .map(|r| {
            format!(
                "{} {}/{}",
                r["workflowName"].as_str().unwrap_or("?"),
                r["status"].as_str().unwrap_or("?"),
                r["conclusion"].as_str().unwrap_or("none")
            )
        })
        .collect();
    if !bad.is_empty() {
        return (
            StepOutcome::Fail,
            format!("not green: {}", bad.join("; ")),
            None,
        );
    }
    let confirmed_disabled = if enabled == Some(false) {
        Some("GitHub Actions is disabled for the repository".to_string())
    } else {
        workflow
            .filter(|w| w["path"] == ".github/workflows/ci.yml")
            .and_then(|w| w["state"].as_str())
            .filter(|state| {
                matches!(
                    *state,
                    "disabled_manually" | "disabled_inactivity" | "disabled_fork"
                )
            })
            .map(|state| format!("CI workflow .github/workflows/ci.yml is {state}"))
    };
    if code != Some(0) {
        let why = format!("gh exited {}: {stderr}", code.unwrap_or(-1));
        return match confirmed_disabled {
            Some(state) => disabled(format!("{why}; the API confirms {state}")),
            None => unexplained(format!("{why}; no CI evidence for {short}")),
        };
    }
    if runs.is_empty() {
        return match confirmed_disabled {
            Some(state) => disabled(format!(
                "no workflow run exists for {short}: the API confirms {state}"
            )),
            None => unexplained(format!(
                "no workflow run exists for {short}; the API did not confirm that CI is disabled"
            )),
        };
    }
    (
        StepOutcome::Pass,
        format!("{} runs concluded success", runs.len()),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_ci_evidence_is_external_only_when_the_api_confirms_actions_are_disabled() {
        let sha = "2582ff95409a9eca69389092e5537450440bc61b";
        let (o, why, b) = judge_runs(sha, Some(1), "HTTP 404: Not Found", &[], None, None);
        assert_eq!(o, StepOutcome::Skip);
        assert!(why.contains("no CI evidence"), "{why}");
        assert!(
            !b.unwrap().external,
            "a 404 alone does not prove the workflows are disabled"
        );
        let (_, why, b) = judge_runs(sha, Some(1), "HTTP 404: Not Found", &[], Some(false), None);
        assert!(why.contains("confirms"), "{why}");
        assert!(b.unwrap().external);
        let (_, _, b) = judge_runs(sha, Some(0), "", &[], Some(true), None);
        assert!(!b.unwrap().external);
        let (_, _, b) = judge_runs(sha, Some(0), "", &[], Some(false), None);
        assert!(b.unwrap().external);
        let green = [json!({"workflowName": "ci", "status": "completed", "conclusion": "success"})];
        assert_eq!(
            judge_runs(sha, Some(0), "", &green, None, None).0,
            StepOutcome::Pass
        );
        let red = [json!({"workflowName": "ci", "status": "completed", "conclusion": "failure"})];
        let (o, why, b) = judge_runs(sha, Some(0), "", &red, None, None);
        assert_eq!(o, StepOutcome::Fail);
        assert!(why.contains("ci completed/failure"), "{why}");
        assert!(b.is_none());
    }
    #[test]
    fn disabled_primary_workflow_is_recorded_without_excusing_failed_runs() {
        let sha = "2582ff95409a9eca69389092e5537450440bc61b";
        let workflow = json!({"path": ".github/workflows/ci.yml", "state": "disabled_manually"});
        let (outcome, reason, blocker) =
            judge_runs(sha, Some(0), "", &[], Some(true), Some(&workflow));
        assert_eq!(outcome, StepOutcome::Skip);
        assert!(
            blocker.unwrap().external,
            "a confirmed disabled CI workflow is a named external blocker"
        );
        assert!(
            reason.contains("ci.yml") && reason.contains("disabled_manually"),
            "{reason}"
        );
        for unconfirmed in [
            json!({"path": ".github/workflows/ci.yml", "state": "active"}),
            json!({"path": ".github/workflows/other.yml", "state": "disabled_manually"}),
            json!({"path": ".github/workflows/ci.yml", "state": "unknown"}),
            json!({"state": "disabled_manually"}),
        ] {
            let (_, _, blocker) = judge_runs(sha, Some(0), "", &[], Some(true), Some(&unconfirmed));
            assert!(!blocker.unwrap().external);
        }
        let failed =
            [json!({"workflowName": "ci", "status": "completed", "conclusion": "failure"})];
        for code in [Some(0), Some(1)] {
            let (outcome, _, blocker) =
                judge_runs(sha, code, "", &failed, Some(true), Some(&workflow));
            assert_eq!(outcome, StepOutcome::Fail);
            assert!(blocker.is_none());
        }
    }
}
