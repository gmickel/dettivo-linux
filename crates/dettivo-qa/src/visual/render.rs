//! Rendering and diffing one matrix entry (fn-20 R1, R5): the surface's
//! binary draws the state headlessly on the offscreen platform at the
//! entry's scale and theme, runs the negative style check over its tree
//! (exit 3 with `style-check:` lines when a control or a text is off the
//! Dettivo style), and `dettivo-visual-diff` scores the render against the
//! baseline, leaving the diff image beside it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use super::matrix::{Entry, theme_env};
use crate::scenarios::binary;

/// The exit code a host uses for a style finding (qt/host/render.h).
pub const STYLE_FAILURE_EXIT: i32 = 3;

/// Why a render did not produce a clean image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// The style check named these findings; the image was still written.
    Style(Vec<String>),
    /// The binary is missing, exited with an error, or wrote no image.
    Process(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Style(findings) => write!(f, "style check: {}", findings.join("; ")),
            Self::Process(why) => write!(f, "{why}"),
        }
    }
}

/// What the diff tool answered (`compare --json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Score {
    /// The overall correlation.
    pub score: f64,
    /// The contrast-structure correlation.
    pub contrast: f64,
    /// The saturation correlation.
    pub saturation: f64,
    /// The column shift that scored best.
    pub shift: i32,
    /// The threshold used.
    pub threshold: f64,
    /// Whether the two images have the same size (a diff tool that
    /// predates the strict facts reports none).
    #[serde(default)]
    pub size_match: Option<bool>,
    /// The largest per-pixel colour distance between matching cells.
    #[serde(default)]
    pub colour_distance: Option<f64>,
    /// Whether the score reached the threshold and, under a strict
    /// comparison, the size matched and the colour stayed within tolerance.
    pub pass: bool,
}

/// The environment a render runs in: the QA switches, the platform, the
/// scale, the theme and the state's own variables.
pub fn render_env(
    repo: &Path,
    entry: &Entry,
    canary: Option<&str>,
) -> Result<BTreeMap<String, String>, String> {
    let mut env = theme_env(repo, &entry.theme)?;
    env.insert("QT_QPA_PLATFORM".into(), "offscreen".into());
    env.insert("QT_SCALE_FACTOR".into(), entry.scale.to_string());
    env.insert("DETTIVO_REDUCED_MOTION".into(), "1".into());
    env.insert("DETTIVO_QA_MODE".into(), "1".into());
    env.insert("DETTIVO_QA_ALLOW_RELEASE".into(), "1".into());
    env.insert("DETTIVO_CONFIG".into(), "/nonexistent/config.toml".into());
    if let Some(axis) = canary {
        env.insert("DETTIVO_QA_CANARY".into(), axis.into());
    }
    for (k, v) in &entry.env {
        env.insert(k.clone(), v.clone());
    }
    Ok(env)
}

/// Renders `entry` to `png`, with the process's stderr in `log`.
pub fn render(
    repo: &Path,
    entry: &Entry,
    png: &Path,
    log: &Path,
    canary: Option<&str>,
) -> Result<(), RenderError> {
    let program = binary(repo, &entry.binary).map_err(RenderError::Process)?;
    let profile = crate::profile::Profile::create("render", None)
        .map_err(|e| RenderError::Process(e.to_string()))?;
    let mut env = profile.env();
    env.extend(render_env(repo, entry, canary).map_err(RenderError::Process)?);
    if let Some(parent) = png.parent() {
        std::fs::create_dir_all(parent).map_err(|e| RenderError::Process(e.to_string()))?;
    }
    let output = crate::profile::command(&program, &env)
        .arg("--render")
        .arg(png)
        .args(&entry.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .output()
        .map_err(|e| RenderError::Process(format!("run {}: {e}", program.display())))?;
    let _ = std::fs::write(log, &output.stderr);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.code() == Some(STYLE_FAILURE_EXIT) {
        return Err(RenderError::Style(style_findings(&stderr)));
    }
    if !output.status.success() {
        return Err(RenderError::Process(format!(
            "{} exited {:?} (see {})",
            entry.binary,
            output.status.code(),
            log.display()
        )));
    }
    if !png.is_file() {
        return Err(RenderError::Process(format!(
            "{} exited 0 but wrote no image at {}",
            entry.binary,
            png.display()
        )));
    }
    if let Some(rect) = entry.crop {
        cut(repo, png, rect, entry.scale).map_err(RenderError::Process)?;
    }
    Ok(())
}

/// Cuts `rect` (a 1x rectangle, scaled by `scale`) out of the render in
/// place with the diff tool, so a region entry compares the region alone.
fn cut(repo: &Path, png: &Path, rect: [u32; 4], scale: u32) -> Result<(), String> {
    let tool = diff_tool(repo)?;
    let whole = png.with_extension("window.png");
    std::fs::rename(png, &whole).map_err(|e| format!("{}: {e}", png.display()))?;
    let status = Command::new(&tool)
        .arg("crop")
        .arg(&whole)
        .args(rect.iter().map(|v| (v * scale).to_string()))
        .arg(png)
        .env("QT_QPA_PLATFORM", "offscreen")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .map_err(|e| format!("run {}: {e}", tool.display()))?;
    if !status.success() || !png.is_file() {
        return Err(format!(
            "could not cut {:?} out of {}",
            rect,
            whole.display()
        ));
    }
    Ok(())
}

/// The `style-check:` lines of a host's stderr.
pub fn style_findings(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter_map(|l| l.strip_prefix("style-check: "))
        .map(str::to_string)
        .collect()
}

/// The diff tool, built with the Qt tree.
pub fn diff_tool(repo: &Path) -> Result<PathBuf, String> {
    let tool = repo.join("build/qt/tools/visual-diff/dettivo-visual-diff");
    tool.is_file()
        .then_some(tool)
        .ok_or_else(|| "dettivo-visual-diff is not built (cmake --build build/qt)".to_string())
}

/// Scores `candidate` against `baseline`, writing the diff image when a
/// path is given.
pub fn compare(
    tool: &Path,
    baseline: &Path,
    candidate: &Path,
    threshold: f64,
    diff: Option<&Path>,
) -> Result<Score, String> {
    compare_with(tool, baseline, candidate, threshold, diff, None)
}

/// `compare`, strict when `colour_tolerance` is given: the baseline is an
/// approved render of the same theme and scale, so the size must match and
/// every pixel's colour must stay within the tolerance.
pub fn compare_with(
    tool: &Path,
    baseline: &Path,
    candidate: &Path,
    threshold: f64,
    diff: Option<&Path>,
    colour_tolerance: Option<f64>,
) -> Result<Score, String> {
    let mut cmd = Command::new(tool);
    cmd.arg("compare")
        .arg(baseline)
        .arg(candidate)
        .arg("--threshold")
        .arg(format!("{threshold:.3}"))
        .arg("--json")
        .env("QT_QPA_PLATFORM", "offscreen")
        .stdin(Stdio::null());
    if let Some(diff) = diff {
        cmd.arg("--diff").arg(diff);
    }
    if let Some(tolerance) = colour_tolerance {
        cmd.arg("--strict")
            .arg("--colour-tolerance")
            .arg(format!("{tolerance:.3}"));
    }
    let output = cmd
        .output()
        .map_err(|e| format!("run {}: {e}", tool.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().find(|l| l.starts_with('{')).ok_or_else(|| {
        format!(
            "dettivo-visual-diff answered no JSON (exit {:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    })?;
    serde_json::from_str(line).map_err(|e| format!("dettivo-visual-diff: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::visual::matrix::Baseline;

    fn entry(theme: &str) -> Entry {
        Entry {
            surface: "osd".into(),
            state: "listening".into(),
            theme: theme.into(),
            scale: 2,
            binary: "dettivo-osd".into(),
            env: BTreeMap::from([("DETTIVO_E2E_OSD_STATE".to_string(), "listening".to_string())]),
            args: Vec::new(),
            threshold_milli: 550,
            colour_tolerance_milli: 60,
            baseline: Baseline::Missing(PathBuf::from("/x")),
            artboard_crop: None,
            crop: None,
        }
    }

    #[test]
    fn the_render_environment_carries_scale_theme_state_and_the_canary() {
        let repo = tempfile::tempdir().unwrap();
        let env = render_env(repo.path(), &entry("builtin-dark"), None).unwrap();
        assert_eq!(env["QT_SCALE_FACTOR"], "2");
        assert_eq!(env["QT_QPA_PLATFORM"], "offscreen");
        assert_eq!(env["DETTIVO_COLOR_SCHEME"], "dark");
        assert_eq!(env["DETTIVO_E2E_OSD_STATE"], "listening");
        assert!(!env.contains_key("DETTIVO_QA_CANARY"));
        let canary = render_env(repo.path(), &entry("builtin-light"), Some("colour")).unwrap();
        assert_eq!(canary["DETTIVO_QA_CANARY"], "colour");
        let missing = render_env(repo.path(), &entry("nord"), None).unwrap_err();
        assert!(missing.contains("nord"), "{missing}");
    }

    #[test]
    fn style_findings_are_the_prefixed_lines() {
        let stderr = "dettivo-sheet: host window\nstyle-check: Button \"x\": control from a, not the Dettivo style\nstyle-check: QQuickText \"\": font family \"Sans\", the theme's is \"monospace\"\n";
        let findings = style_findings(stderr);
        assert_eq!(findings.len(), 2);
        assert!(findings[0].starts_with("Button \"x\""));
        assert!(style_findings("nothing\n").is_empty());
        assert_eq!(
            RenderError::Style(findings).to_string(),
            "style check: Button \"x\": control from a, not the Dettivo style; QQuickText \"\": font family \"Sans\", the theme's is \"monospace\""
        );
    }
}
