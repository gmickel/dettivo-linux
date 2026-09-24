//! Frame pacing evidence (fn-12 R5, fn-20 R4). `dettivo-qa osd-pacing`
//! runs the pill's listening animation for a number of seconds on the real
//! display and writes its summary and the render-thread trace; the drives
//! `osd_dictation` and `theme_switch` collect the same summary from every
//! Qt surface they launch through `DETTIVO_QA_PACING=<file>`, add the
//! scenario, and hold it to the NFR-12 budget on a real display. Under CI
//! (the `CI` variable set) there is no real refresh, so the numbers are
//! recorded and the presence of the report is the gate.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::evidence::Evidence;

/// What the run produced.
#[derive(Debug)]
pub struct PacingReport {
    /// The summary dettivo-osd printed.
    pub summary: Value,
    /// Where the evidence went.
    pub dir: PathBuf,
    /// The pass criteria.
    pub ok: bool,
}

/// Render cost budget per frame, in milliseconds (NFR-12).
pub const RENDER_BUDGET_MS: f64 = 1.0;

/// Dropped frames a drive may see per thousand delivered frames while the
/// surface animates (NFR-12, as measured on this desktop: the compositor's
/// frame callbacks on a busy session cost zero to two two-period gaps in
/// ten seconds at 240 Hz).
pub const DROP_BUDGET_PER_THOUSAND: f64 = 2.0;

/// The floor of the drop budget for short runs.
pub const DROP_BUDGET_FLOOR: i64 = 2;

/// The fastest refresh rate NFR-12 names (60, 120 and 144 Hz). A display
/// faster than this is judged at 144 Hz: a gap counts as a dropped frame
/// when it spans two 144 Hz periods (13.9 ms), so a 240 Hz desktop does
/// not fail on a 4 ms hitch that no rate NFR-12 names would notice.
pub const NFR_MAX_HZ: f64 = 144.0;

/// How long a theme switch may take to apply, in milliseconds (ADR 0010).
pub const THEME_APPLY_BUDGET_MS: f64 = 100.0;

/// Frames a surface must have rendered before its 99th-percentile render
/// cost is judged: a static window paints a handful of frames, and its
/// first paint (shaders, glyph atlas) is not a steady-state cost.
pub const RENDER_JUDGE_MIN_FRAMES: i64 = 100;

/// Whether the thresholds gate: on a real display, never under CI.
pub fn gate_active() -> bool {
    std::env::var_os("CI").is_none()
}

/// The dropped-frame budget for `frames` delivered frames.
pub fn drop_budget(frames: i64) -> i64 {
    ((f64::from(i32::try_from(frames).unwrap_or(i32::MAX)) * DROP_BUDGET_PER_THOUSAND / 1000.0)
        .ceil() as i64)
        .max(DROP_BUDGET_FLOOR)
}

/// Runs the animation for `seconds` and writes the evidence.
pub fn run(osd: &Path, evidence_base: &Path, seconds: u32) -> Result<PacingReport, String> {
    let evidence = Evidence::run_for_process(evidence_base).map_err(|e| e.to_string())?;
    let dir = evidence
        .scenario_dir("osd_pacing", "display")
        .map_err(|e| e.to_string())?;
    let profile = crate::profile::Profile::create("pacing", None).map_err(|e| e.to_string())?;
    let mut env = profile.env();
    env.extend([
        ("DETTIVO_E2E_OSD_STATE".into(), "listening".into()),
        ("DETTIVO_CONFIG".into(), "/nonexistent/config.toml".into()),
        ("QSG_RENDER_TIMING".into(), "1".into()),
        // Qt logs to the journal when stderr is not a terminal; the trace
        // has to land in the captured stream.
        ("QT_FORCE_STDERR_LOGGING".into(), "1".into()),
    ]);
    let output = crate::profile::command(osd, &env)
        .args(["--pacing", &seconds.to_string()])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("run {}: {e}", osd.display()))?;
    std::fs::write(dir.join("render-timing.log"), &output.stderr).map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|l| l.starts_with('{'))
        .ok_or_else(|| {
            format!(
                "dettivo-osd printed no summary (exit {:?}); see {}",
                output.status.code(),
                dir.join("render-timing.log").display()
            )
        })?;
    let summary: Value = serde_json::from_str(line).map_err(|e| format!("summary: {e}"))?;
    std::fs::write(
        dir.join("pacing.json"),
        serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let ok = verdict(&summary);
    Ok(PacingReport { summary, dir, ok })
}

/// Zero dropped frames and the render cost within budget at the 99th
/// percentile (a single scheduler hiccup on the render thread is noted
/// as the maximum, not counted as a failure).
pub fn verdict(summary: &Value) -> bool {
    summary["dropped_frames"].as_i64() == Some(0)
        && summary["render_p99_ms"]
            .as_f64()
            .is_some_and(|ms| ms < RENDER_BUDGET_MS)
        && summary["frames"].as_i64().unwrap_or(0) > 0
}

/// The one-paragraph report.
pub fn human(report: &PacingReport) -> String {
    let s = &report.summary;
    let mut out = format!(
        "osd-pacing: {} frames in {} s at {} Hz (expected {}), {} dropped, render p99 {:.3} ms, max {:.3} ms, mean {:.3} ms -> {}\n",
        s["frames"],
        s["seconds"],
        s["refresh_hz"],
        s["expected_frames"],
        s["dropped_frames"],
        s["render_p99_ms"].as_f64().unwrap_or(0.0),
        s["render_max_ms"].as_f64().unwrap_or(0.0),
        s["render_mean_ms"].as_f64().unwrap_or(0.0),
        if report.ok { "pass" } else { "FAIL" }
    );
    if let Some(drops) = s["dropped"].as_array() {
        for d in drops {
            out.push_str(&format!(
                "  dropped at {:.1} ms: interval {:.1} ms ({} missed)\n",
                d["at_ms"].as_f64().unwrap_or(0.0),
                d["interval_ms"].as_f64().unwrap_or(0.0),
                d["missed"]
            ));
        }
    }
    out.push_str(&format!("  evidence: {}\n", report.dir.display()));
    out
}

/// When a surface was animating, in the collector's milliseconds: a swap
/// gap outside those stretches is an idle window, not a dropped frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Animating<'a> {
    /// A static surface: gaps are idle time, dropped frames are not judged.
    Never,
    /// The whole run.
    Always,
    /// These stretches only (`start_ms`, `end_ms`).
    During(&'a [(f64, f64)]),
}

impl Animating<'_> {
    fn covers(&self, at_ms: f64) -> bool {
        self.covers_span(at_ms, at_ms)
    }

    /// Whether one stretch holds the whole interval `from_ms..=at_ms`.
    fn covers_span(&self, from_ms: f64, at_ms: f64) -> bool {
        match self {
            Self::Never => false,
            Self::Always => true,
            Self::During(windows) => windows.iter().any(|(s, e)| from_ms >= *s && at_ms <= *e),
        }
    }
}

/// The collector's clock for a wall-clock moment: `unix_ms` relative to
/// the summary's `started_unix_ms`.
pub fn collector_ms(summary: &Value, unix_ms: f64) -> f64 {
    unix_ms - summary["started_unix_ms"].as_f64().unwrap_or(0.0)
}

/// What a drive concluded from one surface's pacing summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DriveVerdict {
    /// The scenario.
    pub scenario: String,
    /// The surface (the binary's application name).
    pub surface: String,
    /// Frames delivered while animating.
    pub frames: i64,
    /// Frames dropped while animating.
    pub dropped: i64,
    /// The drop budget for this many frames.
    pub drop_budget: i64,
    /// The render cost at the 99th percentile.
    pub render_p99_ms: f64,
    /// How long the theme switch took to show, when the drive switched.
    pub theme_apply_ms: Option<f64>,
    /// Whether the thresholds gated (a real display) or only recorded.
    pub gated: bool,
    /// The verdict.
    pub ok: bool,
    /// Why not, one line per breach, dropped frames with their timestamps.
    pub reasons: Vec<String>,
}

/// Judges a surface's summary: dropped frames within budget over the
/// stretches the surface animated, render cost within budget once enough
/// frames were rendered, the theme switch within its budget when one was
/// made. Without a gate the numbers are recorded and `ok` is true.
pub fn judge(
    summary: &Value,
    scenario: &str,
    theme_apply_ms: Option<f64>,
    gated: bool,
    animating: Animating<'_>,
) -> DriveVerdict {
    let all_frames = summary["frames"].as_i64().unwrap_or(0);
    let hz = summary["refresh_hz"].as_f64().unwrap_or(60.0);
    let judged_hz = hz.clamp(1.0, NFR_MAX_HZ);
    let period = 1000.0 / judged_hz;
    let swaps: Vec<f64> = summary["swaps_ms"]
        .as_array()
        .map(|s| s.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    let frames = match animating {
        Animating::Never => 0,
        Animating::Always => all_frames,
        Animating::During(_) => {
            i64::try_from(swaps.iter().filter(|ms| animating.covers(**ms)).count())
                .unwrap_or(i64::MAX)
        }
    };
    // Every swap interval of two judged periods or more whose whole span
    // lies inside one animating stretch: the first frame after an idle gap
    // ends an interval that began before the animation did.
    let drops: Vec<String> = swaps
        .windows(2)
        .filter_map(|pair| {
            let interval = pair[1] - pair[0];
            let missed = (interval / period).floor() as i64 - 1;
            (missed >= 1 && animating.covers_span(pair[0], pair[1]))
                .then(|| format!("{:.1} ms ({missed} missed)", pair[1]))
        })
        .collect();
    let dropped = i64::try_from(drops.len()).unwrap_or(i64::MAX);
    let budget = drop_budget(frames);
    let p99 = summary["render_p99_ms"].as_f64().unwrap_or(0.0);
    let mut reasons = Vec::new();
    if animating != Animating::Never && dropped > budget {
        reasons.push(format!(
            "{dropped} dropped frames (two periods or more at {judged_hz:.0} Hz) over the budget of {budget} for {frames} frames while animating: at {}",
            drops.join(", ")
        ));
    }
    if all_frames >= RENDER_JUDGE_MIN_FRAMES && p99 >= RENDER_BUDGET_MS {
        reasons.push(format!(
            "render p99 {p99:.3} ms over the {RENDER_BUDGET_MS} ms budget across {all_frames} frames"
        ));
    }
    if let Some(ms) = theme_apply_ms {
        if ms > THEME_APPLY_BUDGET_MS {
            reasons.push(format!(
                "theme applied after {ms:.1} ms, over the {THEME_APPLY_BUDGET_MS} ms budget"
            ));
        }
    }
    DriveVerdict {
        scenario: scenario.into(),
        surface: summary["surface"].as_str().unwrap_or("").into(),
        frames,
        dropped,
        drop_budget: budget,
        render_p99_ms: p99,
        theme_apply_ms,
        gated,
        ok: !gated || reasons.is_empty(),
        reasons,
    }
}

/// Reads the summary a surface wrote to `file`, adds the scenario, writes
/// it into the evidence directory as `pacing-<surface>.json` and judges
/// it. A missing or unreadable file is the error, by path.
pub fn collect(
    file: &Path,
    scenario: &str,
    evidence_dir: &Path,
    theme_apply_ms: Option<f64>,
    gated: bool,
    animating: Animating<'_>,
) -> Result<(DriveVerdict, String), String> {
    let text = std::fs::read_to_string(file)
        .map_err(|e| format!("pacing summary {}: {e}", file.display()))?;
    let mut summary: Value = serde_json::from_str(&text)
        .map_err(|e| format!("pacing summary {}: {e}", file.display()))?;
    summary["scenario"] = Value::String(scenario.into());
    let verdict = judge(&summary, scenario, theme_apply_ms, gated, animating);
    summary["verdict"] = serde_json::to_value(&verdict).map_err(|e| e.to_string())?;
    let surface = summary["surface"].as_str().unwrap_or("surface").to_string();
    let name = format!("pacing-{surface}.json");
    std::fs::write(
        evidence_dir.join(&name),
        serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok((verdict, name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_verdict_needs_zero_drops_and_the_render_budget() {
        assert!(verdict(
            &json!({"dropped_frames": 0, "render_p99_ms": 0.4, "frames": 600})
        ));
        assert!(!verdict(
            &json!({"dropped_frames": 1, "render_p99_ms": 0.4, "frames": 600})
        ));
        assert!(!verdict(
            &json!({"dropped_frames": 0, "render_p99_ms": 1.2, "frames": 600})
        ));
        assert!(!verdict(
            &json!({"dropped_frames": 0, "render_p99_ms": 0.4, "frames": 0})
        ));
        let report = PacingReport {
            summary: json!({"frames": 10, "seconds": 1, "refresh_hz": 60.0, "expected_frames": 60,
                "dropped_frames": 1, "render_p99_ms": 0.3, "render_max_ms": 0.9, "render_mean_ms": 0.3,
                "dropped": [{"at_ms": 116.7, "interval_ms": 50.0, "missed": 2}]}),
            dir: PathBuf::from("/tmp/x"),
            ok: false,
        };
        let text = human(&report);
        assert!(text.contains("FAIL") && text.contains("dropped at 116.7 ms"));
    }

    #[test]
    fn the_drive_budget_scales_with_frames_and_names_every_breach() {
        assert_eq!(drop_budget(0), 2);
        assert_eq!(drop_budget(2400), 5);
        assert_eq!(drop_budget(12_000), 24);
        // 240 Hz swaps every 4.17 ms; six gaps of two 144 Hz periods or
        // more (one of them three), and a 6 ms hitch that only 240 Hz
        // would call a drop.
        let mut swaps = Vec::new();
        let mut t = 0.0;
        for i in 0..2400 {
            swaps.push(t);
            t += match i {
                100 => 14.0,
                200 => 21.0,
                300 | 400 | 500 | 600 => 14.0,
                700 => 6.0,
                _ => 1000.0 / 240.0,
            };
        }
        let summary = json!({"surface": "dettivo-osd", "frames": 2400, "refresh_hz": 240.0,
            "swaps_ms": swaps, "render_p99_ms": 0.04});
        let v = judge(
            &summary,
            "theme_switch",
            Some(140.0),
            true,
            Animating::Always,
        );
        assert!(!v.ok);
        assert_eq!(v.reasons.len(), 2);
        assert!(
            v.reasons[0]
                .contains("6 dropped frames (two periods or more at 144 Hz) over the budget of 5"),
            "{}",
            v.reasons[0]
        );
        assert!(v.reasons[0].contains("(2 missed)"));
        assert!(v.reasons[1].contains("140.0 ms"));
        assert_eq!(v.surface, "dettivo-osd");
        // Recorded only: the same numbers pass, the reasons stay.
        let recorded = judge(
            &summary,
            "theme_switch",
            Some(140.0),
            false,
            Animating::Always,
        );
        assert!(recorded.ok && !recorded.gated && recorded.reasons.len() == 2);
        let fine = judge(
            &json!({"surface": "dettivo-app", "frames": 60, "refresh_hz": 60.0, "render_p99_ms": 0.2,
                "swaps_ms": [0.0, 16.7, 50.0, 66.7]}),
            "theme_switch",
            Some(30.0),
            true,
            Animating::Always,
        );
        assert!(fine.ok && fine.reasons.is_empty());
        // A handful of frames: the first paint is not a steady-state cost.
        let few = judge(
            &json!({"surface": "dettivo-app", "frames": 3, "dropped_frames": 0, "render_p99_ms": 1.3}),
            "theme_switch",
            Some(30.0),
            true,
            Animating::Always,
        );
        assert!(few.ok, "{:?}", few.reasons);
        let many = judge(
            &json!({"surface": "dettivo-osd", "frames": 2400, "render_p99_ms": 1.3}),
            "osd_dictation",
            None,
            true,
            Animating::Always,
        );
        assert!(!many.ok && many.reasons[0].contains("render p99 1.300 ms"));
    }

    #[test]
    fn only_the_animating_stretches_are_judged() {
        // 60 Hz swaps while listening (0-160 ms and 400-560 ms), one late
        // frame inside the first window, one long idle gap between them.
        let mut swaps: Vec<f64> = (0..=10).map(|i| f64::from(i) * 16.0).collect();
        swaps[5] = 100.0; // 36 ms after the previous swap: two periods
        swaps.extend((0..=10).map(|i| 400.0 + f64::from(i) * 16.0));
        let summary = json!({"surface": "dettivo-osd", "started_unix_ms": 5000.0, "refresh_hz": 60.0,
            "frames": swaps.len(), "swaps_ms": swaps, "render_p99_ms": 0.1});
        let windows = [(0.0, 160.0), (400.0, 560.0)];
        let v = judge(
            &summary,
            "osd_dictation",
            None,
            true,
            Animating::During(&windows),
        );
        assert_eq!((v.frames, v.dropped), (22, 1));
        assert!(v.ok, "{:?}", v.reasons);
        assert_eq!(collector_ms(&summary, 5100.0), 100.0);
        let whole = judge(&summary, "osd_dictation", None, true, Animating::Always);
        assert_eq!(whole.dropped, 2);
        let still = judge(&summary, "osd_dictation", None, true, Animating::Never);
        assert_eq!(still.dropped, 0);
    }

    #[test]
    fn collect_adds_the_scenario_and_names_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("raw.json");
        let missing = collect(
            &file,
            "osd_dictation",
            dir.path(),
            None,
            true,
            Animating::Always,
        )
        .unwrap_err();
        assert!(missing.contains("raw.json"), "{missing}");
        std::fs::write(
            &file,
            json!({"surface": "dettivo-osd", "frames": 10, "refresh_hz": 60.0, "render_p99_ms": 0.1,
                "swaps_ms": [0.0, 16.7, 33.3]})
                .to_string(),
        )
        .unwrap();
        let (verdict, name) = collect(
            &file,
            "osd_dictation",
            dir.path(),
            None,
            true,
            Animating::Always,
        )
        .unwrap();
        assert!(verdict.ok);
        assert_eq!(name, "pacing-dettivo-osd.json");
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.path().join(name)).unwrap()).unwrap();
        assert_eq!(written["scenario"], "osd_dictation");
        assert_eq!(written["verdict"]["ok"], true);
    }
}
