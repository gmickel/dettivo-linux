//! The release gate's `bench_report` step: the checked-in benchmark
//! report for each tier, read as the typed report the suite writes and
//! judged on its contents, so a file that does not parse, a failed step,
//! a missing meeting block or a measurement outside this history never
//! passes on its filename.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde_json::{Value, json};

use super::StepOutcome;
use super::release::{Context, StepReport, blocker, elapsed_ms};
use super::release_steps::row;
use crate::bench::report::{Report, Status};
use crate::nfr::Tier;

/// One tier's evidence as the step embeds it.
#[derive(Debug, PartialEq)]
pub struct Evidence {
    /// The report's `git_sha`.
    pub git_sha: String,
    /// The report's date.
    pub date: String,
    /// The host that measured it.
    pub hostname: String,
    /// `forced_cpu`.
    pub forced_cpu: bool,
    /// The steps as `{name, status, met}`.
    pub steps: Vec<Value>,
}

/// Judges one report's text for `tier`: `Ok` with what the report
/// establishes, `Err` naming the first fact it lacks. `in_history` says
/// whether a commit is an ancestor of the release commit.
pub fn judge(
    text: &str,
    tier: Tier,
    in_history: &dyn Fn(&str) -> bool,
) -> Result<Evidence, String> {
    let report: Report = serde_json::from_str(text).map_err(|e| format!("unreadable: {e}"))?;
    if report.schema_version != 1 {
        return Err(format!("schema_version {} is not 1", report.schema_version));
    }
    if report.tier != tier {
        return Err(format!(
            "the report is the {} tier's, not the {} tier's",
            report.tier.as_str(),
            tier.as_str()
        ));
    }
    if report.host.hostname.is_empty() {
        return Err("the report names no host".into());
    }
    if report.steps.is_empty() {
        return Err("the report has no steps".into());
    }
    if let Some(failed) = report.steps.iter().find(|s| s.status == Status::Failed) {
        return Err(format!(
            "step {} failed: {}",
            failed.name,
            failed.reason.as_deref().unwrap_or("no reason")
        ));
    }
    if let Some(unmeasured) = report.steps.iter().find(|s| {
        s.status == Status::Measured && s.comparison.as_ref().is_some_and(|c| c.value.is_none())
    }) {
        return Err(format!(
            "step {} is measured without a value",
            unmeasured.name
        ));
    }
    if !in_history(&report.git_sha) {
        return Err(format!(
            "measured at {} which is not in this history; run `just bench` on a commit of this history",
            short(&report.git_sha)
        ));
    }
    let Some(meetings) = &report.meetings else {
        return Err("no meetings block; run `just qa-pack-meetings` (`--cpu` for the CPU tier) with --record".into());
    };
    if !in_history(&meetings.git_sha) {
        return Err(format!(
            "the meetings block was recorded at {} which is not in this history",
            short(&meetings.git_sha)
        ));
    }
    if meetings.meeting.value.is_none() || meetings.diarization.value.is_none() {
        return Err("the meetings block carries no meeting or diarization factor".into());
    }
    Ok(Evidence {
        git_sha: report.git_sha.clone(),
        date: report.date.clone(),
        hostname: report.host.hostname.clone(),
        forced_cpu: report.forced_cpu,
        steps: report
            .steps
            .iter()
            .map(|s| {
                json!({
                    "name": s.name,
                    "status": s.status,
                    "met": s.comparison.as_ref().and_then(|c| c.met),
                })
            })
            .collect(),
    })
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(12)]
}

/// The newest report file per host for a tier, from the
/// `<date>-<host>-<tier>.json` names: the GPU tier's is this host's, the
/// CPU tier's are every host's, so a CPU-only machine's report counts
/// wherever the gate runs.
fn newest_per_host(dir: &Path, hostname: &str, tier: Tier) -> Vec<(String, PathBuf)> {
    let suffix = format!("-{}.json", tier.as_str());
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .filter_map(|e| e.file_name().to_str().map(str::to_string))
                .filter(|n| n.ends_with(&suffix) && n.len() > 11 && n.as_bytes()[10] == b'-')
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    for name in names.into_iter().rev() {
        let host = name[11..name.len() - suffix.len()].to_string();
        if host.is_empty() || (tier == Tier::Gpu && host != hostname) {
            continue;
        }
        if !out.iter().any(|(h, _)| *h == host) {
            out.push((host, dir.join(&name)));
        }
    }
    out
}

/// The report that stands for a tier: each host's newest file judged on
/// its contents, a CPU-only host's clean report before this desktop's
/// forced one, and no other file standing in for a newest one that fails.
fn pick(
    dir: &Path,
    hostname: &str,
    tier: Tier,
    in_history: &dyn Fn(&str) -> bool,
) -> Result<(PathBuf, Evidence), String> {
    let mut rejected = Vec::new();
    let mut forced: Option<(PathBuf, Evidence)> = None;
    for (_, path) in newest_per_host(dir, hostname, tier) {
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        match judge(&text, tier, in_history) {
            Ok(ev) if tier == Tier::Cpu && ev.forced_cpu => {
                if forced.is_none() {
                    forced = Some((path, ev));
                }
            }
            Ok(ev) => return Ok((path, ev)),
            Err(why) => rejected.push(format!("{}: {why}", path.display())),
        }
    }
    if let (Some(found), true) = (forced, rejected.is_empty()) {
        return Ok(found);
    }
    Err(if rejected.is_empty() {
        format!(
            "no {} report under {}; run `just bench{}`",
            tier.as_str(),
            dir.display(),
            if tier == Tier::Cpu { "-cpu" } else { "" }
        )
    } else {
        rejected.join("; ")
    })
}

pub(super) fn bench_report(ctx: &Context<'_>) -> StepReport {
    let started = Instant::now();
    let root = &ctx.opts.repo_root;
    let dir = root.join("docs/reports/benchmarks");
    let mut out = row(
        "bench_report",
        format!(
            "{}/<date>-{}-gpu.json and <date>-<host>-cpu.json",
            dir.display(),
            ctx.hostname
        ),
    );
    let in_history = |sha: &str| {
        !sha.is_empty()
            && Command::new("git")
                .args(["merge-base", "--is-ancestor", sha, "HEAD"])
                .current_dir(root)
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
    };
    let mut rows = Vec::new();
    let mut failures = Vec::new();
    for tier in [Tier::Gpu, Tier::Cpu] {
        match pick(&dir, &ctx.hostname, tier, &in_history) {
            Ok((path, ev)) => {
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                if tier == Tier::Cpu && ev.forced_cpu {
                    out.blockers.push(blocker(
                        "bench_report",
                        &format!("{relative} is the CPU row measured on this desktop under DETTIVO_FORCE_CPU=1; the CPU-only VM has not run `just bench-cpu` and `just qa-pack-meetings-cpu`"),
                    ));
                }
                rows.push(json!({
                    "tier": tier.as_str(), "path": relative, "date": ev.date, "git_sha": ev.git_sha,
                    "host": ev.hostname, "forced_cpu": ev.forced_cpu, "steps": ev.steps,
                }));
            }
            Err(why) => failures.push(format!("{}: {why}", tier.as_str())),
        }
    }
    out.report = Some(json!({ "reports": rows }));
    if failures.is_empty() {
        out.outcome = StepOutcome::Pass;
        let named: Vec<String> = rows
            .iter()
            .map(|r| {
                format!(
                    "{} ({} on {}, {})",
                    r["path"].as_str().unwrap_or("?"),
                    r["date"].as_str().unwrap_or("?"),
                    r["host"].as_str().unwrap_or("?"),
                    short(r["git_sha"].as_str().unwrap_or(""))
                )
            })
            .collect();
        out.reason = Some(format!("{} reports: {}", rows.len(), named.join("; ")));
    } else {
        out.reason = Some(failures.join("; "));
    }
    out.duration_ms = elapsed_ms(started);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "8ba2f5481e5fcbae1f0efa647c1523ea511614fd";

    fn report(tier: &str, forced: bool, host: &str) -> Value {
        json!({
            "schema_version": 1, "generated_unix": 1, "date": "2026-09-05", "git_sha": SHA,
            "quick": false, "iterations": 10, "tier": tier, "tier_reason": "r", "forced_cpu": forced,
            "notes": [], "host": {"hostname": host, "cpu": "c", "memory_bytes": 1, "kernel": "k", "gpu": null, "gpu_reason": null},
            "engines": [], "models": [], "fixtures": [], "targets": [],
            "steps": [{"name": "startup", "status": "measured", "reason": null, "results": [], "duration_ms": 1,
                "comparison": {"nfr": "NFR-8", "metric": "m", "unit": "ms", "target": 300.0, "initial": 300.0, "direction": "at_most", "value": 181.0, "met": true}}],
            "meetings": {"recorded_unix": 2, "date": "2026-09-06", "git_sha": SHA, "pack_run_dir": "r", "pack_passed": true,
                "forced_cpu": forced, "engine": "whisper", "model": "m", "backend": "cpu", "audio_ms": 10, "wall_ms": 1, "fixture_sha256": "f",
                "meeting": {"nfr": "NFR-5", "metric": "m", "unit": "x", "target": 5.0, "initial": 5.0, "direction": "at_least", "value": 10.0, "met": true},
                "diarization": {"nfr": "NFR-4", "metric": "d", "unit": "x", "target": 4.0, "initial": 4.0, "direction": "at_least", "value": 10.0, "met": true},
                "gpu_workload_proof": "skip", "harness_cpu_pct": null, "harness_warning": null, "token_coverage": null}
        })
    }

    fn all(_: &str) -> bool {
        true
    }

    #[test]
    fn a_report_passes_on_its_contents_and_never_on_its_name() {
        let ok = judge(&report("gpu", false, "thor").to_string(), Tier::Gpu, &all).unwrap();
        assert_eq!((ok.hostname.as_str(), ok.forced_cpu), ("thor", false));
        assert_eq!(ok.steps[0]["met"], Value::Bool(true));
        assert!(
            judge("{", Tier::Gpu, &all)
                .unwrap_err()
                .starts_with("unreadable")
        );
        assert!(
            judge("{}", Tier::Gpu, &all)
                .unwrap_err()
                .starts_with("unreadable")
        );
        let wrong_tier = judge(&report("cpu", true, "thor").to_string(), Tier::Gpu, &all);
        assert!(wrong_tier.unwrap_err().contains("cpu tier's, not the gpu"));
        let mut empty = report("gpu", false, "thor");
        empty["steps"] = json!([]);
        assert_eq!(
            judge(&empty.to_string(), Tier::Gpu, &all).unwrap_err(),
            "the report has no steps"
        );
        let mut failed = report("gpu", false, "thor");
        failed["steps"][0]["status"] = json!("failed");
        failed["steps"][0]["reason"] = json!("the daemon died");
        assert_eq!(
            judge(&failed.to_string(), Tier::Gpu, &all).unwrap_err(),
            "step startup failed: the daemon died"
        );
        let mut absent = report("gpu", false, "thor");
        absent["steps"][0]["comparison"]["value"] = Value::Null;
        assert!(
            judge(&absent.to_string(), Tier::Gpu, &all)
                .unwrap_err()
                .contains("without a value")
        );
        let mut no_meetings = report("gpu", false, "thor");
        no_meetings.as_object_mut().unwrap().remove("meetings");
        assert!(
            judge(&no_meetings.to_string(), Tier::Gpu, &all)
                .unwrap_err()
                .starts_with("no meetings block")
        );
        let squashed = judge(
            &report("gpu", false, "thor").to_string(),
            Tier::Gpu,
            &|_| false,
        );
        assert!(
            squashed.unwrap_err().contains("not in this history"),
            "history is enforced"
        );
        let mut old_block = report("gpu", false, "thor");
        old_block["meetings"]["git_sha"] = json!("0000000000000000000000000000000000000000");
        let only_report_in_history = |s: &str| s == SHA;
        assert!(
            judge(&old_block.to_string(), Tier::Gpu, &only_report_in_history)
                .unwrap_err()
                .contains("meetings block was recorded")
        );
    }

    #[test]
    fn a_cpu_only_host_clears_the_forced_desktop_row_and_a_corrupt_newest_file_never_stands_in() {
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str, text: String| std::fs::write(dir.path().join(name), text).unwrap();
        write(
            "2026-09-05-thor-cpu.json",
            report("cpu", true, "thor").to_string(),
        );
        let (path, ev) = pick(dir.path(), "thor", Tier::Cpu, &all).unwrap();
        assert!(path.ends_with("2026-09-05-thor-cpu.json") && ev.forced_cpu);
        write(
            "2026-09-01-vm-cpu.json",
            report("cpu", false, "vm").to_string(),
        );
        let (path, ev) = pick(dir.path(), "thor", Tier::Cpu, &all).unwrap();
        assert!(
            path.ends_with("2026-09-01-vm-cpu.json"),
            "{}",
            path.display()
        );
        assert!(!ev.forced_cpu && ev.hostname == "vm");
        write(
            "2026-09-05-thor-gpu.json",
            report("gpu", false, "thor").to_string(),
        );
        assert!(pick(dir.path(), "thor", Tier::Gpu, &all).is_ok());
        write("2026-09-06-thor-gpu.json", "not json".into());
        let why = pick(dir.path(), "thor", Tier::Gpu, &all).unwrap_err();
        assert!(
            why.contains("2026-09-06-thor-gpu.json: unreadable"),
            "the corrupt newest file fails the tier: {why}"
        );
        assert!(
            pick(dir.path(), "other", Tier::Gpu, &all)
                .unwrap_err()
                .contains("no gpu report")
        );
        write("2026-09-07-vm-cpu.json", "{}".into());
        let why = pick(dir.path(), "thor", Tier::Cpu, &all).unwrap_err();
        assert!(
            why.contains("vm-cpu.json: unreadable"),
            "a corrupt newest VM file fails the tier rather than the desktop row standing in: {why}"
        );
    }
}
