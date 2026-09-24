//! The pack report: `<pack>-pack.json` for the release gate and the
//! flow-next QA pass, `<pack>-pack.md` for a person. Every step is named
//! with its outcome, duration, evidence directory and reason; the
//! measurements aggregate what the steps wrote (the insertion matrix rows,
//! the first-insert runs, the WER result) against the NFR targets; the
//! blockers list names every step that could not run and why; a GUI
//! pack adds one block per surface with the scans (`surfaces`), and
//! every pack records the profile of the binaries it drove.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::surfaces::SurfaceReport;

pub use super::binaries::{BinaryInfo, binaries, check_release_binaries};
use super::{Pack, StepOutcome, StepResult};

pub use super::measure::{FailedTarget, Measurements, NFR3_TARGET, measure, tier};

/// The machine the pack ran on, from `system.capabilities.platform`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Machine {
    /// The host name.
    pub hostname: String,
    /// `linux`.
    pub os: String,
    /// The compositor, when known.
    pub compositor: Option<String>,
    /// `wayland` or `x11`.
    pub session: String,
    /// `vulkan` when an ICD is installed.
    pub gpu: Option<String>,
    /// The insertion chain's choice.
    pub insertion_backend: String,
}

impl Machine {
    /// From the `platform` block.
    pub fn from_platform(platform: &Value) -> Self {
        let text = |k: &str| platform[k].as_str().unwrap_or("").to_string();
        Self {
            hostname: std::fs::read_to_string("/etc/hostname")
                .map(|h| h.trim().to_string())
                .ok()
                .filter(|h| !h.is_empty())
                .or_else(|| std::env::var("HOSTNAME").ok())
                .unwrap_or_else(|| "unknown".into()),
            os: text("os"),
            compositor: platform["compositor"].as_str().map(str::to_string),
            session: text("session"),
            gpu: platform["gpu"].as_str().map(str::to_string),
            insertion_backend: text("insertion_backend"),
        }
    }
}

/// A step that could not run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blocker {
    /// The step, or `preflight`.
    pub step: String,
    /// The driver.
    pub driver: String,
    /// Why.
    pub reason: String,
}

/// The whole report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// The pack name.
    pub pack: String,
    /// The machine.
    pub machine: Machine,
    /// `gpu` when the Whisper engine ran on Vulkan, `cpu` otherwise.
    pub tier: String,
    /// The backend the engine reported, when a step saw it.
    pub engine_backend: Option<String>,
    /// True under CI, where the NFR figures are recorded and never gated.
    pub ci: bool,
    /// Whether the NFR comparison gates the exit code (it never does; the
    /// release spec decides).
    pub nfr_gated: bool,
    /// The pack's run directory.
    pub run_dir: String,
    /// Unix time the run started.
    pub started_unix: u64,
    /// Wall time.
    pub duration_ms: u64,
    /// True when no step failed hard.
    pub passed: bool,
    /// One row per step, in order.
    pub steps: Vec<StepResult>,
    /// The measurements.
    pub measurements: Measurements,
    /// Every step that could not run, and why.
    pub blockers: Vec<Blocker>,
    /// One block per surface (a GUI pack); empty otherwise.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surfaces: Vec<SurfaceReport>,
    /// The binaries the pack drove, with their build profile.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub binaries: BTreeMap<String, BinaryInfo>,
    /// True: every scenario's profile sets `DETTIVO_QA_ALLOW_RELEASE=1`,
    /// so a release binary accepts QA mode and the scans run against it.
    #[serde(default)]
    pub qa_allow_release: bool,
}

/// Builds the report from the step results and the evidence under
/// `run_dir`; the last pair is the start time (Unix seconds) and the
/// wall time in milliseconds.
pub fn build(
    pack: &Pack,
    machine: Machine,
    run_dir: &Path,
    steps: Vec<StepResult>,
    preflight_blockers: Vec<Blocker>,
    binaries: BTreeMap<String, BinaryInfo>,
    (started_unix, duration_ms): (u64, u64),
) -> Report {
    let (mut measurements, engine_backend, _) = measure(run_dir, &steps);
    let (tier, target) = tier(engine_backend.as_deref());
    measurements.first_insert_nfr = target.nfr.to_string();
    measurements.nfr1_target_ms = Some(target.calibrated as u64);
    measurements.nfr1_met = measurements
        .first_insert_p50_ms
        .map(|p50| target.met(p50 as f64));
    let mut blockers = preflight_blockers;
    blockers.extend(
        steps
            .iter()
            .filter(|s| matches!(s.outcome, StepOutcome::Skip | StepOutcome::NotRun))
            .map(|s| Blocker {
                step: s.id.clone(),
                driver: s.driver.clone(),
                reason: s.reason.clone().unwrap_or_else(|| "skipped".into()),
            }),
    );
    Report {
        pack: pack.name.to_string(),
        machine,
        tier: tier.as_str().to_string(),
        engine_backend,
        ci: std::env::var_os("CI").is_some(),
        nfr_gated: false,
        run_dir: run_dir.display().to_string(),
        started_unix,
        duration_ms,
        passed: super::passed(pack, &steps),
        surfaces: super::surfaces::build(pack, &steps),
        steps,
        measurements,
        blockers,
        binaries,
        qa_allow_release: true,
    }
}

#[cfg(test)]
mod tests {
    use super::super::markdown::{markdown, write};
    use super::*;
    use crate::nfr;
    use crate::pack::wer::WerResult;
    use crate::scenarios::first_insert_timing::{Run, Timing};
    use crate::scenarios::support::MatrixRow;

    fn step(id: &str, driver: &str, outcome: StepOutcome, reason: Option<&str>) -> StepResult {
        StepResult {
            id: id.into(),
            driver: driver.into(),
            outcome,
            duration_ms: 10,
            evidence: Some(format!("{id}.{driver}")),
            reason: reason.map(str::to_string),
            surface: None,
            scan: None,
        }
    }

    fn run(n: u64, cold: bool) -> Run {
        Run {
            job_id: format!("job_dict_{n}"),
            cold,
            total_ms: n * 100,
            capture_ms: 10,
            transcribe_ms: n * 80,
            insert_ms: 5,
            outcome: "inserted".into(),
            backend: Some("xdotool".into()),
        }
    }

    #[test]
    fn the_report_renders_from_a_fixture_result() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path();
        let matrix = run_dir.join("insertion_matrix.atspi");
        std::fs::create_dir_all(&matrix).unwrap();
        let rows = vec![
            MatrixRow {
                target: "dettivo-insert-target".into(),
                outcome: "pass".into(),
                backend: Some("xdotool".into()),
                latency_ms: Some(40),
                reason: None,
            },
            MatrixRow {
                target: "foot".into(),
                outcome: "fail".into(),
                backend: Some("virtual_keyboard".into()),
                latency_ms: Some(90),
                reason: Some("read back 3 characters".into()),
            },
            MatrixRow::skipped("chromium", "chromium is not installed".into()),
        ];
        std::fs::write(
            matrix.join("insertion-matrix.json"),
            serde_json::to_string(&rows).unwrap(),
        )
        .unwrap();
        let timing_dir = run_dir.join("first_insert_timing.atspi");
        std::fs::create_dir_all(&timing_dir).unwrap();
        let mut runs = vec![run(60, true)];
        runs.extend((1..=10).map(|n| run(n, false)));
        let timing = Timing::summarise(10_000, runs, Some("vulkan".into()), Value::Null);
        std::fs::write(
            timing_dir.join("first-insert-timing.json"),
            serde_json::to_string(&timing).unwrap(),
        )
        .unwrap();
        let wer_dir = run_dir.join("whisper_wer");
        std::fs::create_dir_all(&wer_dir).unwrap();
        std::fs::write(
            wer_dir.join("wer.json"),
            serde_json::to_string(&WerResult {
                rate: 0.0,
                threshold: 0.15,
                backend: "vulkan".into(),
                model: "tiny.en".into(),
                reference_words: 22,
                hypothesis_words: 22,
                audio_ms: 11_000,
                elapsed_ms: 900,
                forced_cpu: false,
            })
            .unwrap(),
        )
        .unwrap();
        let pack = super::super::by_name("dictation").unwrap();
        let steps = vec![
            step(
                "hotkeys_hyprland",
                "atspi",
                StepOutcome::Skip,
                Some("hyprctl is not on PATH"),
            ),
            step("osd_dictation", "atspi", StepOutcome::Pass, None),
            step(
                "osd_dictation",
                "cua",
                StepOutcome::Skip,
                Some("missing: cua-driver"),
            ),
            step(
                "insertion_matrix",
                "atspi",
                StepOutcome::Fail,
                Some("foot: read back 3 characters"),
            ),
            step("never_into_self", "atspi", StepOutcome::Pass, None),
            step("history_roundtrip", "none", StepOutcome::Pass, None),
            step("first_insert_timing", "atspi", StepOutcome::Pass, None),
            StepResult {
                evidence: Some("whisper_wer".into()),
                ..step("whisper_wer", "none", StepOutcome::Pass, None)
            },
        ];
        let preflight = vec![Blocker {
            step: "preflight".into(),
            driver: "-".into(),
            reason: "accessibility bus: org.a11y.Bus does not answer".into(),
        }];
        let binaries = BTreeMap::from([(
            "dettivod".to_string(),
            BinaryInfo {
                path: "/r/target/release/dettivod".into(),
                profile: "release".into(),
                sha256: "ab".into(),
                revision: None,
            },
        )]);
        let report = build(
            &pack,
            Machine::default(),
            run_dir,
            steps,
            preflight,
            binaries,
            (1, 5_000),
        );
        let m = &report.measurements;
        assert_eq!(report.tier, "gpu");
        assert!(report.surfaces.is_empty());
        assert!(report.qa_allow_release);
        assert_eq!(report.binaries["dettivod"].profile, "release");
        assert_eq!(m.first_insert_nfr, "NFR-1");
        assert_eq!(
            m.nfr1_target_ms,
            Some(nfr::NFR1_FIRST_INSERT_GPU.calibrated as u64)
        );
        assert_eq!(
            (m.first_insert_p50_ms, m.first_insert_p95_ms),
            (Some(500), Some(1000))
        );
        assert_eq!(m.first_insert_cold_ms, Some(6000));
        assert_eq!(m.nfr1_met, Some(true));
        assert_eq!((m.insertion_attempted, m.insertion_passed), (2, 1));
        assert_eq!(m.insertion_reliability, Some(0.5));
        assert_eq!(m.nfr3_met, Some(false));
        assert_eq!(m.insertion_skipped, ["chromium: chromium is not installed"]);
        assert_eq!(
            m.insertion_failed[0].backend.as_deref(),
            Some("virtual_keyboard")
        );
        assert_eq!(m.wer.as_ref().unwrap().backend, "vulkan");
        assert!(!report.passed);
        let blockers: Vec<&str> = report.blockers.iter().map(|b| b.step.as_str()).collect();
        assert_eq!(blockers, ["preflight", "hotkeys_hyprland", "osd_dictation"]);

        let md = markdown(&report);
        assert!(md.contains("| `insertion_matrix` | atspi | fail | 10 ms | `insertion_matrix.atspi/` | foot: read back 3 characters |"), "{md}");
        assert!(
            md.contains("p50 500 ms, p95 1000 ms over 10 of 10 warm runs; the cold run (6000 ms)"),
            "{md}"
        );
        assert!(md.contains("0.500 (1 of 2 attempted targets) against 0.98: not met. Skipped and excluded: chromium: chromium is not installed."), "{md}");
        assert!(
            md.contains("- `hotkeys_hyprland` (atspi): hyprctl is not on PATH"),
            "{md}"
        );
        let (json, md_path) = write(&report, run_dir).unwrap();
        let back: Report = serde_json::from_str(&std::fs::read_to_string(json).unwrap()).unwrap();
        assert_eq!(back, report);
        assert!(md_path.ends_with("dictation-pack.md"));
    }

    #[test]
    fn each_tier_reads_its_calibrated_first_insert_target() {
        assert_eq!(
            tier(Some("cpu")),
            (nfr::Tier::Cpu, nfr::NFR2_FIRST_INSERT_CPU)
        );
        assert_eq!(tier(None), (nfr::Tier::Cpu, nfr::NFR2_FIRST_INSERT_CPU));
        assert_eq!(
            tier(Some("vulkan")),
            (nfr::Tier::Gpu, nfr::NFR1_FIRST_INSERT_GPU)
        );
    }
}
