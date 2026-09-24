//! The human rendering of a bench report and the README table under
//! `docs/reports/benchmarks/`: the latest report per host and tier, every
//! step's figure beside its target, met or not.

use std::path::Path;

use super::{Report, Status};

fn figure(value: Option<f64>, unit: &str) -> String {
    match value {
        None => "-".into(),
        Some(v) if unit == "bytes" => format!("{:.1} MiB", v / 1024.0 / 1024.0),
        Some(v) if unit == "ms" => format!("{v:.0} ms"),
        Some(v) => format!("{v:.2} {unit}"),
    }
}

fn verdict(met: Option<bool>) -> &'static str {
    match met {
        Some(true) => "met",
        Some(false) => "not met",
        None => "not compared",
    }
}

/// The human report.
pub fn human(r: &Report) -> String {
    let mut out = format!(
        "bench: {} tier {} ({}) on {} ({}, {}), {} iterations{}, commit {}\n",
        r.host.hostname,
        r.tier.as_str(),
        r.tier_reason,
        r.host.cpu,
        r.host
            .gpu
            .as_ref()
            .map(|g| g.name.clone())
            .unwrap_or_else(|| "no Vulkan device".into()),
        format_args!("{} GiB", r.host.memory_bytes >> 30),
        r.iterations,
        if r.quick { " (quick)" } else { "" },
        &r.git_sha[..r.git_sha.len().min(12)]
    );
    for s in &r.steps {
        let line = match &s.comparison {
            Some(c) => format!(
                "{} {} {} against {} {}: {}",
                c.nfr,
                c.metric,
                figure(c.value, &c.unit),
                figure(Some(c.target), &c.unit),
                if c.direction == crate::nfr::Direction::AtMost {
                    "at most"
                } else {
                    "at least"
                },
                verdict(c.met)
            ),
            None => "recorded".into(),
        };
        out.push_str(&format!(
            "  {:<9} {:<15} {}{}\n",
            match s.status {
                Status::Measured => "measured",
                Status::Skipped => "skipped",
                Status::NotAvailable => "n/a",
                Status::Failed => "FAILED",
            },
            s.name,
            line,
            s.reason
                .as_deref()
                .map(|x| format!("  ({x})"))
                .unwrap_or_default()
        ));
    }
    for note in &r.notes {
        out.push_str(&format!("  note: {note}\n"));
    }
    out
}

/// The load average at the start and the end of the run, as a sentence.
fn load_line(r: &Report) -> String {
    match (r.load_average.start, r.load_average.end) {
        (Some(s), Some(e)) => format!(
            " Load average {:.1} at the start and {:.1} at the end of the run (1 min).",
            s[0], e[0]
        ),
        _ => String::new(),
    }
}

/// The README for a directory of reports: the latest per host and tier.
pub fn readme(dir: &Path) -> Result<String, String> {
    let mut reports: Vec<Report> = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        // Accuracy evaluations share this directory. Match the suite filename
        // contract used by the release gate, then parse every candidate strictly.
        .filter(|e| {
            e.file_name().to_str().is_some_and(|name| {
                ["-gpu.json", "-cpu.json"].iter().any(|suffix| {
                    name.ends_with(suffix)
                        && name.len() > 11 + suffix.len()
                        && name.as_bytes()[10] == b'-'
                })
            })
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let text = std::fs::read_to_string(entry.path()).map_err(|e| e.to_string())?;
        let report: Report =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", entry.path().display()))?;
        reports.push(report);
    }
    let mut latest: Vec<&Report> = Vec::new();
    for r in &reports {
        match latest
            .iter_mut()
            .find(|l| l.host.hostname == r.host.hostname && l.tier == r.tier)
        {
            Some(slot) if slot.generated_unix < r.generated_unix => *slot = r,
            Some(_) => {}
            None => latest.push(r),
        }
    }
    let mut out = String::from(
        "# Benchmark reports\n\nThe numbers Dettivo for Linux promises, measured on real machines: `dettivo-qa bench` (`just bench`) runs the NFR steps from the fixtures the tests use and files one report per host and tier here, every figure beside its calibrated target ([ADR 0029](../../adr/0029-nfr-calibration-and-the-benchmark-suite.md), the suite in [docs/qa.md](../../qa.md)). This table is rendered from the latest report per host and tier; `just bench` rewrites it.\n\n",
    );
    for r in &latest {
        out.push_str(&format!(
            "## {} on the {} tier ({})\n\n{} with {}, {} GiB, commit `{}`{}; {}.{}\n\n| Step | NFR | Metric | Measured | Target | Verdict |\n|---|---|---|---|---|---|\n",
            r.host.hostname,
            r.tier.as_str(),
            r.date,
            r.host.cpu,
            r.host
                .gpu
                .as_ref()
                .map(|g| format!("{} ({} {})", g.name, g.driver, g.driver_info))
                .unwrap_or_else(|| "no Vulkan device".into()),
            r.host.memory_bytes >> 30,
            &r.git_sha[..r.git_sha.len().min(12)],
            if r.quick { ", quick run" } else { "" },
            r.tier_reason,
            load_line(r)
        ));
        for s in &r.steps {
            let (nfr, metric, measured, target, met) = match &s.comparison {
                Some(c) => (
                    c.nfr.clone(),
                    c.metric.clone(),
                    figure(c.value, &c.unit),
                    format!(
                        "{} {}",
                        if c.direction == crate::nfr::Direction::AtMost {
                            "<="
                        } else {
                            ">="
                        },
                        figure(Some(c.target), &c.unit)
                    ),
                    verdict(c.met).to_string(),
                ),
                None => (
                    "-".into(),
                    "-".into(),
                    "-".into(),
                    "-".into(),
                    "recorded".into(),
                ),
            };
            let status = match s.status {
                Status::Measured => met,
                Status::Skipped => format!("skipped: {}", s.reason.as_deref().unwrap_or("")),
                Status::NotAvailable => "not available".into(),
                Status::Failed => format!("failed: {}", s.reason.as_deref().unwrap_or("")),
            };
            out.push_str(&format!(
                "| `{}` | {} | {} | {} | {} | {} |\n",
                s.name,
                nfr,
                metric,
                measured,
                target,
                status.replace('|', "\\|").replace('\n', " ")
            ));
            // One line per engine row beneath the headline, so the engine
            // the tier's target is not stated for is visible too.
            let Some(c) = &s.comparison else { continue };
            if s.results.len() < 2 {
                continue;
            }
            for row in &s.results {
                let (Some(provider), Some(value)) =
                    (row["provider"].as_str(), row["figure"].as_f64())
                else {
                    continue;
                };
                out.push_str(&format!(
                    "| `{}` · {} {} | {} | {} | {} | {} | {} |\n",
                    s.name,
                    provider,
                    row["model"].as_str().unwrap_or(""),
                    c.nfr,
                    c.metric,
                    figure(Some(value), &c.unit),
                    target,
                    verdict(row["met"].as_bool())
                ));
            }
        }
        if let Some(m) = &r.meetings {
            for row in m.rows() {
                out.push_str(&row);
                out.push('\n');
            }
        }
        out.push('\n');
        for note in &r.notes {
            out.push_str(&format!("- {note}\n"));
        }
        if let Some(m) = &r.meetings {
            out.push_str(&format!("- {}\n", m.note()));
        }
        out.push('\n');
    }
    if latest.is_empty() {
        out.push_str("No report has been filed yet.\n");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::{Comparison, Step, host};
    use crate::nfr;

    fn report(unix: u64, tier: nfr::Tier, p50: f64) -> Report {
        Report {
            schema_version: 1,
            generated_unix: unix,
            date: crate::bench::date_of(unix),
            git_sha: "abcdef1234567890".into(),
            quick: false,
            iterations: 10,
            tier,
            tier_reason: "a Vulkan device".into(),
            forced_cpu: false,
            notes: vec!["a note".into()],
            load_average: Default::default(),
            host: host::Host {
                hostname: "thor".into(),
                cpu: "Ryzen".into(),
                memory_bytes: 64 << 30,
                kernel: "7.1".into(),
                gpu: None,
                gpu_reason: Some("no vulkaninfo".into()),
            },
            engines: Vec::new(),
            models: Vec::new(),
            fixtures: Vec::new(),
            steps: vec![
                Step {
                    name: "first_insert".into(),
                    status: Status::Measured,
                    reason: None,
                    comparison: Some(Comparison::of(&nfr::first_insert(tier), Some(p50))),
                    results: vec![
                        serde_json::json!({
                            "provider": "whisper", "model": "large-v3-turbo",
                            "figure": p50, "met": nfr::first_insert(tier).met(p50)
                        }),
                        serde_json::json!({
                            "provider": "parakeet", "model": "parakeet-v3",
                            "figure": 400.0, "met": true
                        }),
                    ],
                    duration_ms: 1,
                },
                Step::without("llm_rewrite", Status::NotAvailable, "later".into()),
            ],
            targets: nfr::all(),
            meetings: None,
        }
    }

    #[test]
    fn the_readme_ignores_accuracy_reports_but_rejects_malformed_suite_reports() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("diarization-accuracy-2026-09-09.json"),
            r#"{"schema_version":1,"accuracy":0.98}"#,
        )
        .unwrap();
        let suite = dir.path().join("2026-09-14-thor-gpu.json");
        std::fs::write(
            &suite,
            serde_json::to_string(&report(2_000, nfr::Tier::Gpu, 700.0)).unwrap(),
        )
        .unwrap();
        let md = readme(dir.path()).unwrap();
        assert!(md.contains("700 ms"), "{md}");
        std::fs::write(&suite, "{}").unwrap();
        let error = readme(dir.path()).unwrap_err();
        assert!(error.contains("2026-09-14-thor-gpu.json"), "{error}");
    }

    #[test]
    fn the_readme_keeps_the_latest_report_per_host_and_tier() {
        let dir = tempfile::tempdir().unwrap();
        for (name, r) in [
            (
                "2026-09-01-thor-gpu.json",
                report(1_000, nfr::Tier::Gpu, 900.0),
            ),
            (
                "2026-09-02-thor-gpu.json",
                report(2_000, nfr::Tier::Gpu, 700.0),
            ),
            (
                "2026-09-02-thor-cpu.json",
                report(2_000, nfr::Tier::Cpu, 3000.0),
            ),
        ] {
            std::fs::write(dir.path().join(name), serde_json::to_string(&r).unwrap()).unwrap();
        }
        let md = readme(dir.path()).unwrap();
        assert!(md.contains("## thor on the gpu tier (1970-01-01)"), "{md}");
        assert!(
            md.contains(
                "| `first_insert` | NFR-1 | first_insert_p50 | 700 ms | <= 1000 ms | met |"
            ),
            "{md}"
        );
        assert!(!md.contains("900 ms"), "{md}");
        assert!(
            md.contains(
                "| `first_insert` | NFR-2 | first_insert_p50 | 3000 ms | <= 1500 ms | not met |"
            ),
            "{md}"
        );
        assert!(
            md.contains(
                "| `first_insert` · parakeet parakeet-v3 | NFR-2 | first_insert_p50 | 400 ms | <= 1500 ms | met |"
            ),
            "{md}"
        );
        assert!(
            md.contains("| `llm_rewrite` | - | - | - | - | not available |"),
            "{md}"
        );
        let text = human(&report(3, nfr::Tier::Gpu, 700.0));
        assert!(
            text.contains("NFR-1 first_insert_p50 700 ms against 1000 ms at most: met"),
            "{text}"
        );
        assert!(text.contains("n/a       llm_rewrite"), "{text}");
    }
}
