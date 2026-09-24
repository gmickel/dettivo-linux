//! The visual report (fn-20 R1): `visual-report.json` and a Markdown
//! table with the render, the baseline and the diff image per entry, the
//! score against the threshold, the advisory score against the artboard,
//! and the outcome: `pass`, `fail`, `first-approval` (no baseline yet),
//! `style` (the negative style check named something) or `error`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One entry's line in the report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntryReport {
    /// The surface.
    pub surface: String,
    /// The state.
    pub state: String,
    /// The theme.
    pub theme: String,
    /// The scale.
    pub scale: u32,
    /// `pass`, `fail`, `first-approval`, `style` or `error`.
    pub outcome: String,
    /// The score against the baseline, when a comparison ran.
    pub score: Option<f64>,
    /// The threshold.
    pub threshold: f64,
    /// The baseline path, when there is one.
    pub baseline: Option<String>,
    /// `approved`, `artboard` or `missing`.
    pub baseline_kind: String,
    /// The render, relative to the report directory.
    pub render: Option<String>,
    /// The diff image, relative to the report directory.
    pub diff: Option<String>,
    /// The advisory score against the artboard crop, on every theme.
    pub artboard_score: Option<f64>,
    /// The largest per-pixel colour distance against the baseline, when the
    /// diff tool measured it (the sizes matched).
    #[serde(default)]
    pub colour_distance: Option<f64>,
    /// Why, when not a pass.
    pub reason: Option<String>,
    /// The style check's findings.
    pub style_findings: Vec<String>,
}

/// The whole run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Unix time the run started.
    pub started_unix: u64,
    /// Whether this was the canary run, where every entry must fail.
    pub canary: bool,
    /// Every entry, in matrix order.
    pub entries: Vec<EntryReport>,
    /// Entries that passed.
    pub passed: usize,
    /// Entries that failed, erred or were off style.
    pub failed: usize,
    /// Entries with no baseline yet.
    pub first_approval: usize,
}

impl Report {
    /// A report over `entries`.
    pub fn new(entries: Vec<EntryReport>, canary: bool) -> Self {
        let passed = entries.iter().filter(|e| e.outcome == "pass").count();
        let first_approval = entries
            .iter()
            .filter(|e| e.outcome == "first-approval")
            .count();
        let failed = entries.len() - passed - first_approval;
        Self {
            started_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            canary,
            entries,
            passed,
            failed,
            first_approval,
        }
    }

    /// A normal run passes when every entry passed; the canary run passes
    /// when every compared entry failed its diff, so a diff that stopped
    /// seeing differences is caught.
    pub fn ok(&self) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        if self.canary {
            self.entries.iter().all(|e| e.outcome == "fail")
        } else {
            self.failed == 0 && self.first_approval == 0
        }
    }

    /// Writes `visual-report.json` and `visual-report.md` into `dir`.
    pub fn write(&self, dir: &Path) -> Result<(PathBuf, PathBuf), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let json = dir.join("visual-report.json");
        std::fs::write(
            &json,
            serde_json::to_string_pretty(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("{}: {e}", json.display()))?;
        let md = dir.join("visual-report.md");
        std::fs::write(&md, self.markdown()).map_err(|e| format!("{}: {e}", md.display()))?;
        Ok((json, md))
    }

    /// The Markdown table.
    pub fn markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(if self.canary {
            "# Visual regression: canary run\n\nEvery entry renders with a deliberate token change and must fail its diff.\n\n"
        } else {
            "# Visual regression\n\n"
        });
        out.push_str(&format!(
            "{} entries: {} passed, {} failed, {} awaiting first approval. Verdict: **{}**.\n\n",
            self.entries.len(),
            self.passed,
            self.failed,
            self.first_approval,
            if self.ok() { "pass" } else { "FAIL" }
        ));
        out.push_str("| Surface | State | Theme | Scale | Outcome | Score | Threshold | Artboard | Baseline | Render | Diff | Reason |\n");
        out.push_str("|---|---|---|---|---|---|---|---|---|---|---|---|\n");
        for e in &self.entries {
            let link = |p: &Option<String>| {
                p.as_deref()
                    .map(|p| format!("[{}]({p})", p.rsplit('/').next().unwrap_or(p)))
                    .unwrap_or_default()
            };
            let score = |s: Option<f64>| s.map(|s| format!("{s:.3}")).unwrap_or_default();
            let mut reason = e.reason.clone().unwrap_or_default();
            if !e.style_findings.is_empty() {
                reason = e.style_findings.join("; ");
            }
            out.push_str(&format!(
                "| {} | {} | {} | {}x | {} | {} | {:.2} | {} | {} | {} | {} | {} |\n",
                e.surface,
                e.state,
                e.theme,
                e.scale,
                e.outcome,
                score(e.score),
                e.threshold,
                score(e.artboard_score),
                e.baseline
                    .as_deref()
                    .map(|b| format!("{} ({})", b, e.baseline_kind))
                    .unwrap_or_else(|| e.baseline_kind.clone()),
                link(&e.render),
                link(&e.diff),
                reason.replace('|', "\\|")
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(outcome: &str, score: Option<f64>) -> EntryReport {
        EntryReport {
            surface: "osd".into(),
            state: "listening".into(),
            theme: "black-gold".into(),
            scale: 2,
            outcome: outcome.into(),
            score,
            threshold: 0.55,
            baseline: Some("docs/design/studio/baselines/osd/listening.png".into()),
            baseline_kind: "artboard".into(),
            render: Some("osd/listening.black-gold.2x.png".into()),
            diff: Some("osd/listening.black-gold.2x.diff.png".into()),
            artboard_score: score,
            colour_distance: None,
            reason: (outcome != "pass").then(|| format!("{outcome} reason")),
            style_findings: Vec::new(),
        }
    }

    #[test]
    fn the_report_counts_outcomes_and_never_passes_a_first_approval_silently() {
        let report = Report::new(
            vec![
                entry("pass", Some(0.7)),
                entry("first-approval", None),
                entry("fail", Some(0.4)),
            ],
            false,
        );
        assert_eq!(
            (report.passed, report.failed, report.first_approval),
            (1, 1, 1)
        );
        assert!(!report.ok());
        assert!(Report::new(vec![entry("pass", Some(0.7))], false).ok());
        assert!(!Report::new(vec![entry("first-approval", None)], false).ok());
        assert!(!Report::new(Vec::new(), false).ok());
    }

    #[test]
    fn the_canary_passes_only_when_every_entry_fails() {
        assert!(Report::new(vec![entry("fail", Some(0.3))], true).ok());
        assert!(
            !Report::new(
                vec![entry("fail", Some(0.3)), entry("pass", Some(0.9))],
                true
            )
            .ok()
        );
        assert!(!Report::new(vec![entry("error", None)], true).ok());
    }

    #[test]
    fn the_files_land_with_the_table_and_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let mut styled = entry("style", None);
        styled.style_findings = vec!["Button \"x\": control from a | b".into()];
        let report = Report::new(vec![entry("pass", Some(0.712)), styled], false);
        let (json, md) = report.write(dir.path()).unwrap();
        let back: Report = serde_json::from_str(&std::fs::read_to_string(json).unwrap()).unwrap();
        assert_eq!(back, report);
        let text = std::fs::read_to_string(md).unwrap();
        assert!(
            text.contains("| osd | listening | black-gold | 2x | pass | 0.712 | 0.55 | 0.712 |")
        );
        assert!(
            text.contains(
                "[listening.black-gold.2x.diff.png](osd/listening.black-gold.2x.diff.png)"
            )
        );
        assert!(text.contains("control from a \\| b"));
        assert!(text.contains("Verdict: **FAIL**"));
    }
}
