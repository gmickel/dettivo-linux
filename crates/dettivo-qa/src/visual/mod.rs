//! `dettivo-qa visual` (fn-20, ADR 0021): every manifest surface rendered
//! at every scale and theme, diffed against its baseline, reported with
//! diff images, exit 1 on an unapproved difference; `approve` turns a
//! render into a baseline with its provenance; `--canary` renders with a
//! deliberate token regression and passes only when every diff fails.

pub mod approve;
pub mod cli;
pub mod manifest;
pub mod matrix;
pub mod render;
pub mod report;

use std::path::{Path, PathBuf};

use manifest::Manifest;
use matrix::{Baseline, Entry, Filter, expand};
use render::{RenderError, compare, diff_tool, render};
use report::{EntryReport, Report};

/// The default output directory, relative to the working directory.
pub const DEFAULT_OUT: &str = "qa-evidence/visual";

/// The surface the canary renders when no filter names one: the sheet,
/// where a type or spacing change moves every row.
pub const CANARY_SURFACE: &str = "design-system";

/// The token regressions a canary run can ask the theme for
/// (`DETTIVO_QA_CANARY`): `all` moves the accent and raises the type and
/// spacing; `colour` moves the accent alone, which only the strict
/// comparison against an approved render can see; `type` raises the font
/// sizes; `spacing` widens the spacing units.
pub const CANARY_AXES: &[&str] = &["all", "colour", "type", "spacing"];

/// Options for a run.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Which entries.
    pub filter: Filter,
    /// Where renders, diffs and the report go.
    pub out: PathBuf,
    /// The canary run.
    pub canary: bool,
    /// Which regression the canary renders with (`all` when empty).
    pub canary_axis: String,
}

impl RunOptions {
    /// The `DETTIVO_QA_CANARY` value a canary render gets.
    pub fn canary_value(&self) -> Option<&str> {
        self.canary.then_some({
            if self.canary_axis.is_empty() {
                "all"
            } else {
                self.canary_axis.as_str()
            }
        })
    }
}

/// Renders, diffs and reports every entry the options keep.
pub fn run(repo: &Path, opts: &RunOptions) -> Result<Report, String> {
    let manifest = Manifest::load(repo)?;
    let mut filter = opts.filter.clone();
    if opts.canary && filter.surface.is_none() {
        filter.surface = Some(CANARY_SURFACE.into());
    }
    let canary = opts.canary_value();
    if let Some(axis) = canary.filter(|a| !CANARY_AXES.contains(a)) {
        return Err(format!(
            "canary axis {axis:?} is not one of {}",
            CANARY_AXES.join(", ")
        ));
    }
    let mut entries = expand(&manifest, repo, &filter);
    // A colour-only regression is invisible to the structure score by
    // design (the artboard crops are drawn in another theme), so that
    // canary is judged on the approved renders alone.
    if canary == Some("colour") {
        entries.retain(Entry::strict);
    }
    if entries.is_empty() {
        return Err(format!("no manifest entry matches {filter:?}"));
    }
    let tool = diff_tool(repo)?;
    std::fs::create_dir_all(&opts.out).map_err(|e| format!("{}: {e}", opts.out.display()))?;
    let mut lines = Vec::with_capacity(entries.len());
    for entry in &entries {
        lines.push(run_entry(repo, &tool, entry, &opts.out, canary));
    }
    let report = Report::new(lines, opts.canary);
    report.write(&opts.out)?;
    Ok(report)
}

fn relative(out: &Path, path: &Path) -> String {
    path.strip_prefix(out)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn run_entry(
    repo: &Path,
    tool: &Path,
    entry: &Entry,
    out: &Path,
    canary: Option<&str>,
) -> EntryReport {
    let dir = out.join(&entry.surface);
    let png = dir.join(format!("{}.png", entry.stem()));
    let log = dir.join(format!("{}.log", entry.stem()));
    let diff = dir.join(format!("{}.diff.png", entry.stem()));
    let mut line = EntryReport {
        surface: entry.surface.clone(),
        state: entry.state.clone(),
        theme: entry.theme.clone(),
        scale: entry.scale,
        outcome: "error".into(),
        score: None,
        threshold: entry.threshold(),
        baseline: entry.baseline.path().map(|p| relative(repo, p)),
        baseline_kind: entry.baseline.kind().into(),
        render: None,
        diff: None,
        artboard_score: None,
        colour_distance: None,
        reason: None,
        style_findings: Vec::new(),
    };
    match render(repo, entry, &png, &log, canary) {
        Ok(()) => line.render = Some(relative(out, &png)),
        Err(RenderError::Style(findings)) => {
            line.render = png.is_file().then(|| relative(out, &png));
            line.outcome = "style".into();
            line.reason = Some("the negative style check named an item".into());
            line.style_findings = findings;
            return line;
        }
        Err(RenderError::Process(why)) => {
            line.reason = Some(why);
            return line;
        }
    }
    if let Some(crop) = entry.artboard_crop.as_deref().filter(|c| c.is_file()) {
        if let Ok(score) = compare(tool, crop, &png, entry.threshold(), None) {
            line.artboard_score = Some(score.score);
        }
    }
    let Some(baseline) = entry.baseline.path() else {
        line.outcome = "first-approval".into();
        line.reason = Some(match &entry.baseline {
            Baseline::Missing(path) => format!(
                "no baseline yet; `dettivo-qa visual approve {} --state {} --theme {} --scale {}` writes {}",
                entry.surface,
                entry.state,
                entry.theme,
                entry.scale,
                relative(repo, path)
            ),
            _ => "no baseline".into(),
        });
        return line;
    };
    if !baseline.is_file() {
        line.reason = Some(format!(
            "baseline crop missing: {}",
            relative(repo, baseline)
        ));
        return line;
    }
    // An approved render of this theme and scale is held to its size and
    // colour as well as its structure; an artboard crop, drawn in the
    // artboard's theme, only to its structure.
    let strict = entry.strict().then(|| entry.colour_tolerance());
    match render::compare_with(tool, baseline, &png, entry.threshold(), Some(&diff), strict) {
        Ok(score) => {
            line.score = Some(score.score);
            line.colour_distance = score.colour_distance;
            line.diff = diff.is_file().then(|| relative(out, &diff));
            if score.pass {
                line.outcome = "pass".into();
            } else {
                line.outcome = "fail".into();
                line.reason = Some(failure_reason(&score, strict));
            }
        }
        Err(why) => line.reason = Some(why),
    }
    line
}

/// Why a comparison failed: the structure score under its threshold, or,
/// under a strict comparison, the size or the colour.
fn failure_reason(score: &render::Score, colour_tolerance: Option<f64>) -> String {
    let mut parts = Vec::new();
    if score.score < score.threshold {
        parts.push(format!(
            "score {:.3} under the threshold {:.2} (structure {:.3}, colour {:.3}, shift {})",
            score.score, score.threshold, score.contrast, score.saturation, score.shift
        ));
    }
    if let Some(tolerance) = colour_tolerance {
        if score.size_match == Some(false) {
            parts.push("the render is not the approved render's size".into());
        }
        if let Some(distance) = score.colour_distance.filter(|d| *d > tolerance) {
            parts.push(format!(
                "colour distance {distance:.3} over the tolerance {tolerance:.2} against the approved render"
            ));
        }
    }
    if parts.is_empty() {
        parts.push("the diff tool reported a failure without a reason".into());
    }
    parts.join("; ")
}

/// The one-line-per-entry human output.
pub fn human(report: &Report, out: &Path) -> String {
    let mut text = String::new();
    for e in &report.entries {
        let score = e
            .score
            .map(|s| format!("{s:.3}"))
            .unwrap_or_else(|| "-".into());
        let artboard = e
            .artboard_score
            .map(|s| format!(" artboard {s:.3}"))
            .unwrap_or_default();
        let reason = e
            .reason
            .as_deref()
            .map(|r| format!("  {r}"))
            .unwrap_or_default();
        let findings = if e.style_findings.is_empty() {
            String::new()
        } else {
            format!("  {}", e.style_findings.join("; "))
        };
        text.push_str(&format!(
            "{:<15} {:<14} {:<12} {:<17} {}x  {} (threshold {:.2}{artboard}){reason}{findings}\n",
            e.outcome, e.surface, e.state, e.theme, e.scale, score, e.threshold
        ));
    }
    text.push_str(&format!(
        "visual{}: {} entries, {} passed, {} failed, {} awaiting first approval -> {} (report under {})\n",
        if report.canary { " canary" } else { "" },
        report.entries.len(),
        report.passed,
        report.failed,
        report.first_approval,
        if report.ok() { "pass" } else { "FAIL" },
        out.display()
    ));
    text
}

/// The `approve` verb's human output.
pub fn human_approved(approved: &[approve::Approved]) -> String {
    let mut text = String::new();
    for a in approved {
        text.push_str(&format!(
            "approved {}/{} {} {}x -> {}{}{}\n",
            a.entry.surface,
            a.entry.state,
            a.entry.theme,
            a.entry.scale,
            a.path.display(),
            a.artboard_score
                .map(|s| format!(" (artboard score {s:.3})"))
                .unwrap_or_default(),
            if a.replaced { " (replaced)" } else { "" }
        ));
    }
    text.push_str(&format!(
        "visual approve: {} baseline(s) written, recorded in {}\n",
        approved.len(),
        approve::LOG_PATH
    ));
    text
}
