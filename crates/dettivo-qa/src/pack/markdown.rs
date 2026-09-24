//! The human rendering of a pack report (`<pack>-pack.md`) and the
//! writer for both files.

use std::path::{Path, PathBuf};

use super::report::Report;
use crate::scenarios::first_insert_timing::WARM_RUNS;

fn ms(v: Option<u64>) -> String {
    v.map(|n| format!("{n} ms")).unwrap_or_else(|| "-".into())
}

/// A table cell: pipes escaped, line breaks flattened, so a reason never
/// breaks the row.
fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\n', '\r'], " ")
}

fn measurements(r: &Report, out: &mut String) {
    let m = &r.measurements;
    out.push_str("\n## Measurements\n\n");
    // The meetings pack has its own figures and none of the dictation
    // slice's.
    if let Some(meetings) = &m.meetings {
        super::meeting_measure::markdown(meetings, out);
        return;
    }
    out.push_str(&format!(
        "- Time to first insert ({}, tier {}): p50 {}, p95 {} over {} of {WARM_RUNS} warm runs; the cold run ({}) warmed the engine and is excluded. Split at p50: capture {}, transcribe {}, insert {}. Target {}: {}.\n",
        if m.first_insert_nfr.is_empty() {
            "NFR-1"
        } else {
            m.first_insert_nfr.as_str()
        },
        r.tier,
        ms(m.first_insert_p50_ms),
        ms(m.first_insert_p95_ms),
        m.first_insert_warm_runs,
        ms(m.first_insert_cold_ms),
        ms(m.first_insert_split_p50.as_ref().map(|s| s.capture_ms)),
        ms(m.first_insert_split_p50.as_ref().map(|s| s.transcribe_ms)),
        ms(m.first_insert_split_p50.as_ref().map(|s| s.insert_ms)),
        m.nfr1_target_ms
            .map(|t| format!("p50 <= {t} ms"))
            .unwrap_or_else(|| "none set for this tier".into()),
        match m.nfr1_met {
            Some(true) => "met",
            Some(false) => "not met",
            None => "not compared",
        }
    ));
    out.push_str(&format!(
        "- Insertion reliability (NFR-3): {} ({} of {} attempted targets) against {:.2}: {}. Skipped and excluded: {}.\n",
        m.insertion_reliability
            .map(|x| format!("{x:.3}"))
            .unwrap_or_else(|| "-".into()),
        m.insertion_passed,
        m.insertion_attempted,
        m.nfr3_target,
        match m.nfr3_met {
            Some(true) => "met",
            Some(false) => "not met",
            None => "not measured",
        },
        if m.insertion_skipped.is_empty() {
            "none".to_string()
        } else {
            m.insertion_skipped.join("; ")
        }
    ));
    for f in &m.insertion_failed {
        out.push_str(&format!(
            "  - failed: {} ({}): {}\n",
            f.target,
            f.backend.as_deref().unwrap_or("no backend"),
            f.reason
        ));
    }
    match &m.wer {
        Some(w) => out.push_str(&format!(
            "- Whisper WER: {:.3} against {:.2} on {} ({}, {} ms audio, {} ms wall).\n",
            w.rate, w.threshold, w.backend, w.model, w.audio_ms, w.elapsed_ms
        )),
        None => out.push_str("- Whisper WER: not measured.\n"),
    }
    if !m.matrix.is_empty() {
        out.push_str(
            "\n| Target | Outcome | Backend | Latency | Reason |\n|---|---|---|---|---|\n",
        );
        for row in &m.matrix {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                row.target,
                row.outcome,
                row.backend.as_deref().unwrap_or("-"),
                ms(row.latency_ms),
                cell(row.reason.as_deref().unwrap_or(""))
            ));
        }
    }
}

/// One section per surface: the steps, the scans and the coverage.
fn surfaces(r: &Report, out: &mut String) {
    if r.surfaces.is_empty() {
        return;
    }
    out.push_str("\n## Surfaces\n\n| Surface | Outcome | Steps | Routes scanned | Developer text | Accessibility coverage |\n|---|---|---|---|---|---|\n");
    for s in &r.surfaces {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            s.name,
            if s.passed { "passed" } else { "FAILED" },
            s.steps.len(),
            s.negative_text.routes,
            s.negative_text.findings.len(),
            match s.a11y_coverage {
                Some(c) => format!("{c:.3} ({} of {} named)", s.a11y_named, s.a11y_interactive),
                None => "not inspected".into(),
            }
        ));
    }
    for s in &r.surfaces {
        if s.negative_text.findings.is_empty() && s.a11y_offenders.is_empty() {
            continue;
        }
        out.push_str(&format!("\n### {}\n\n", s.name));
        for f in &s.negative_text.findings {
            out.push_str(&format!(
                "- `{}` ({}), route `{}`: {} in {}\n",
                f.step,
                f.driver,
                f.route,
                f.class,
                cell(&format!("{:?}", f.name))
            ));
        }
        for o in &s.a11y_offenders {
            out.push_str(&format!(
                "- `{}` ({}), route `{}`: unnamed {} #{}\n",
                o.step, o.driver, o.route, o.role, o.index
            ));
        }
    }
}

/// Renders the pack report as the Markdown document beside the JSON.
pub fn markdown(r: &Report) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# Pack `{}`: {}\n\n",
        r.pack,
        if r.passed { "passed" } else { "FAILED" }
    ));
    out.push_str(&format!(
        "Machine `{}` ({}, {}, {} session, gpu {}, insertion backend {}), tier **{}**{}. Run directory `{}`, {} ms.\n\n",
        r.machine.hostname,
        r.machine.os,
        r.machine.compositor.as_deref().unwrap_or("no compositor"),
        r.machine.session,
        r.machine.gpu.as_deref().unwrap_or("none"),
        r.machine.insertion_backend,
        r.tier,
        if r.ci { " under CI (NFR figures recorded, never gated)" } else { "" },
        r.run_dir,
        r.duration_ms
    ));
    if !r.binaries.is_empty() {
        let list: Vec<String> = r
            .binaries
            .iter()
            .map(|(name, b)| format!("`{name}` {}", b.profile))
            .collect();
        out.push_str(&format!(
            "Binaries: {}; QA mode allowed in release builds: {}.\n\n",
            list.join(", "),
            if r.qa_allow_release { "yes" } else { "no" }
        ));
    }
    out.push_str("## Steps\n\n| Step | Driver | Outcome | Duration | Evidence | Reason |\n|---|---|---|---|---|---|\n");
    for s in &r.steps {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} ms | {} | {} |\n",
            s.id,
            s.driver,
            s.outcome.as_str(),
            s.duration_ms,
            s.evidence
                .as_deref()
                .map(|e| format!("`{e}/`"))
                .unwrap_or_else(|| "-".into()),
            cell(s.reason.as_deref().unwrap_or(""))
        ));
    }
    surfaces(r, &mut out);
    measurements(r, &mut out);
    out.push_str("\n## Blockers\n\n");
    if r.blockers.is_empty() {
        out.push_str("None: every step ran.\n");
    } else {
        for b in &r.blockers {
            out.push_str(&format!(
                "- `{}` ({}): {}\n",
                b.step,
                b.driver,
                b.reason.replace('\n', " ")
            ));
        }
    }
    out
}

/// Writes `<pack>-pack.json` and `<pack>-pack.md` into `dir`.
pub fn write(report: &Report, dir: &Path) -> std::io::Result<(PathBuf, PathBuf)> {
    let json = dir.join(format!("{}-pack.json", report.pack));
    let md = dir.join(format!("{}-pack.md", report.pack));
    std::fs::write(
        &json,
        serde_json::to_string_pretty(report).map_err(std::io::Error::other)?,
    )?;
    std::fs::write(&md, markdown(report))?;
    Ok((json, md))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a11y_tree::{Coverage, Offender};
    use crate::pack::report::{self, Machine};
    use crate::pack::scan::{RouteScan, StepScan, TextFinding};
    use crate::pack::{StepOutcome, StepResult, by_name, run_steps};

    #[test]
    fn a_gui_report_renders_its_surfaces_with_the_findings_located() {
        let gui = by_name("gui").unwrap();
        let results = run_steps(&gui, "atspi", true, &mut |step, driver| {
            let planted = step.id == "settings_roundtrip.models" && driver == "atspi";
            StepResult {
                id: step.id.into(),
                driver: driver.into(),
                outcome: if planted {
                    StepOutcome::Fail
                } else {
                    StepOutcome::Pass
                },
                duration_ms: 3,
                evidence: Some(format!("{}.{driver}", step.id)),
                reason: planted.then(|| "developer text on the models route".to_string()),
                surface: None,
                scan: planted.then(|| StepScan {
                    routes: vec![RouteScan {
                        route: "models".into(),
                        negative_text: vec![TextFinding {
                            class: "internal identifier".into(),
                            name: "parity_gap: speech.model".into(),
                        }],
                        a11y: Coverage {
                            interactive: 4,
                            named: 3,
                            coverage: 0.75,
                            offenders: vec![Offender {
                                role: "push button".into(),
                                id: "/e/9".into(),
                                index: 9,
                                bounds: Some((1, 2, 3, 4)),
                            }],
                        },
                    }],
                }),
            }
        });
        let dir = tempfile::tempdir().unwrap();
        let report = report::build(
            &gui,
            Machine::default(),
            dir.path(),
            results,
            Vec::new(),
            Default::default(),
            (7, 900),
        );
        assert!(!report.passed);
        assert_eq!(report.surfaces.len(), 4);
        let settings = &report.surfaces[1];
        assert_eq!(settings.name, "settings");
        assert!(!settings.passed && report.surfaces[0].passed);
        assert_eq!(settings.negative_text.findings.len(), 1);
        assert!((settings.a11y_coverage.unwrap() - 0.75).abs() < 1e-9);
        let (json, md) = write(&report, dir.path()).unwrap();
        assert!(json.ends_with("gui-pack.json") && md.ends_with("gui-pack.md"));
        let text = std::fs::read_to_string(&md).unwrap();
        assert!(text.contains("## Surfaces"), "{text}");
        assert!(
            text.contains("| settings | FAILED | 11 | 1 | 1 | 0.750 (3 of 4 named) |"),
            "{text}"
        );
        assert!(
            text.contains("- `settings_roundtrip.models` (atspi), route `models`: internal identifier in \"parity_gap: speech.model\""),
            "{text}"
        );
        assert!(
            text.contains(
                "- `settings_roundtrip.models` (atspi), route `models`: unnamed push button #9"
            ),
            "{text}"
        );
        assert!(
            text.contains("| onboarding | passed | 8 | 0 | 0 | not inspected |"),
            "{text}"
        );
        let back: report::Report =
            serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
        assert_eq!(back.surfaces[1].a11y_offenders[0].index, 9);
        assert!(back.qa_allow_release);
    }
}
