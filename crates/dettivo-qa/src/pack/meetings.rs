//! The meetings pack (fn-36, ADR 0039): the meeting lane in order on the
//! machine in front of you. Two-source capture on the virtual rig with
//! the device swap, the kill and recovery, the live windowed transcript,
//! diarization against the turns golden, the export goldens, the three
//! meetings screens on both drivers, the two-voice token fixture through
//! capture, transcription and diarization, then the NFR-4 and NFR-5
//! measurements: a five-minute meeting import timed through the daemon's
//! own finalisation path, the diarization engine over the same track, and
//! the proof that the GPU tier's engine did its work on the GPU. `--cpu`
//! records the CPU tier with `base.en`, `--record` files the figures
//! into the checked-in benchmark report.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{Expect, Pack, RunOptions, Step, StepKind, scenario};
use crate::audio;
use crate::bench::host::CATALOGUE;

/// The pack's name.
pub const NAME: &str = "meetings";
/// The NFR-5 model: what `--cpu` transcribes with unless `--model` says.
pub const CPU_MODEL: &str = "base.en";
/// The GPU tier's headline model (ADR 0029), when it is on disk.
pub const GPU_MODEL: &str = "large-v3-turbo";
/// The test model every machine has.
pub const TEST_MODEL: &str = "tiny.en";

/// The pack's own switches.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// `DETTIVO_FORCE_CPU=1` for every daemon and engine the pack drives,
    /// and the CPU tier's model for the throughput steps.
    pub cpu: bool,
    /// The Whisper model the throughput steps transcribe with.
    pub model: Option<String>,
    /// The engine binaries (a Vulkan build); the workspace build otherwise.
    pub engines_dir: Option<PathBuf>,
    /// File the figures into `docs/reports/benchmarks/`.
    pub record: bool,
}

impl Options {
    /// The variables every scenario step's processes get.
    pub fn scenario_env(&self) -> BTreeMap<String, String> {
        let mut env = BTreeMap::new();
        if self.cpu {
            env.insert("DETTIVO_FORCE_CPU".to_string(), "1".to_string());
        }
        env
    }

    /// The model the throughput steps use: `--model`, else `base.en`
    /// under `--cpu` and `large-v3-turbo` on the GPU tier when on disk,
    /// else `tiny.en`.
    pub fn model(&self, models_dir: Option<&Path>) -> String {
        if let Some(m) = &self.model {
            return m.clone();
        }
        let preferred = if self.cpu { CPU_MODEL } else { GPU_MODEL };
        let on_disk = models_dir.is_some_and(|d| {
            d.join("whisper")
                .join(preferred)
                .join(format!("ggml-{preferred}.bin"))
                .is_file()
        });
        if on_disk {
            preferred.to_string()
        } else {
            TEST_MODEL.to_string()
        }
    }
}

const fn on(
    id: &'static str,
    driver: &'static str,
    expect: Expect,
    contributes: &'static str,
) -> Step {
    Step {
        id,
        kind: StepKind::Scenario,
        driver: Some(driver),
        expect,
        surface: None,
        contributes,
    }
}

const fn kind(id: &'static str, kind: StepKind, expect: Expect, contributes: &'static str) -> Step {
    Step {
        id,
        kind,
        driver: None,
        expect,
        surface: None,
        contributes,
    }
}

/// The steps, in order. The rig capture step must pass where PipeWire
/// runs; without it (a container) the mock steps carry the capture path
/// and the rig step is an allowed skip the blockers name.
pub fn steps(pipewire: bool) -> Vec<Step> {
    let rig = if pipewire {
        Expect::Pass
    } else {
        Expect::SkipAllowed
    };
    let mut steps = vec![
        scenario(
            "meeting_rig",
            rig,
            "two-source capture on the rig, the gap marker after the swap",
        ),
        scenario(
            "meeting_recovery",
            Expect::Pass,
            "SIGKILL, recover, the transcript over both takes",
        ),
        scenario(
            "meeting_live",
            Expect::Pass,
            "the live transcript against the interleave golden",
        ),
        kind(
            "diarization_der",
            StepKind::DiarizationDer,
            Expect::Pass,
            "two speakers, DER under 0.20",
        ),
        Step {
            id: "meeting_export_goldens",
            kind: StepKind::Command,
            driver: None,
            expect: Expect::Pass,
            surface: None,
            contributes: "the five formats byte-equal",
        },
    ];
    for (id, what) in [
        (
            "meetings_seeded",
            "the list, detail, export, rename, delete",
        ),
        ("meetings_live_gui", "disclosure to live to detail"),
        (
            "meetings_import_gui",
            "the import dialog onto transcripts.import",
        ),
    ] {
        steps.push(on(id, "atspi", Expect::Pass, what));
        steps.push(on(id, "cua", Expect::SkipAllowed, what));
    }
    steps.extend([
        scenario(
            "meeting_token_coverage",
            Expect::Pass,
            "every alpha token on its source, two named speakers in md",
        ),
        kind(
            "meeting_throughput",
            StepKind::MeetingThroughput,
            Expect::Pass,
            "meeting_rtf (NFR-4 or NFR-5)",
        ),
        kind(
            "diarization_throughput",
            StepKind::DiarizationThroughput,
            Expect::Pass,
            "diarization_rtf (NFR-4)",
        ),
        kind(
            "gpu_workload_proof",
            StepKind::GpuWorkloadProof,
            Expect::SkipAllowed,
            "the engine pid on the GPU",
        ),
    ]);
    steps
}

/// The pack.
pub fn pack() -> Pack {
    Pack {
        name: NAME,
        summary: "the meeting lane end to end: the rig, recovery, the live transcript, diarization, the exports, the three screens, the token fixture and the throughput report",
        steps: steps(audio::pipewire_available()),
    }
}

/// The Whisper model ids the catalogue offers.
pub fn catalogue_models(repo_root: &Path) -> Result<Vec<String>, String> {
    let path = repo_root.join(CATALOGUE);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let catalogue: toml::Value = toml::from_str(&text).map_err(|e| e.to_string())?;
    Ok(catalogue["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["provider"].as_str() == Some("whisper"))
        .filter_map(|m| m["id"].as_str().map(str::to_string))
        .collect())
}

/// The meetings pack's refusals before anything starts: a `--model` the
/// catalogue does not know exits 2 naming the ids it does.
pub fn preflight(pack: &Pack, opts: &RunOptions) -> Result<(), String> {
    if pack.name != NAME {
        return Ok(());
    }
    if let Some(model) = &opts.meetings.model {
        let ids = catalogue_models(&opts.repo_root)?;
        if !ids.iter().any(|id| id == model) {
            return Err(format!(
                "unknown model {model:?}; the catalogue's Whisper models are: {}",
                ids.join(", ")
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::{StepOutcome, StepResult, by_name, driver_for, passed, run_steps};

    fn result(step: &Step, driver: &str, outcome: StepOutcome) -> StepResult {
        StepResult {
            id: step.id.into(),
            driver: driver.into(),
            outcome,
            duration_ms: 1,
            evidence: None,
            reason: None,
            surface: None,
            scan: None,
        }
    }

    #[test]
    fn the_pack_runs_the_lane_in_order_from_real_steps() {
        let ids: Vec<&str> = steps(true).iter().map(|s| s.id).collect();
        assert_eq!(
            ids,
            [
                "meeting_rig",
                "meeting_recovery",
                "meeting_live",
                "diarization_der",
                "meeting_export_goldens",
                "meetings_seeded",
                "meetings_seeded",
                "meetings_live_gui",
                "meetings_live_gui",
                "meetings_import_gui",
                "meetings_import_gui",
                "meeting_token_coverage",
                "meeting_throughput",
                "diarization_throughput",
                "gpu_workload_proof",
            ]
        );
        for step in steps(true) {
            match step.kind {
                StepKind::Scenario => assert!(
                    crate::scenarios::by_id(step.id).is_some(),
                    "{} is not a scenario",
                    step.id
                ),
                StepKind::Command => assert!(
                    crate::pack::gui::command_for(step.id).is_some(),
                    "{} has no command",
                    step.id
                ),
                _ => assert_eq!(driver_for(&step, "cua"), "none"),
            }
        }
        let pack = by_name(NAME).unwrap();
        assert_eq!(driver_for(&pack.steps[5], "cua"), "atspi");
        assert_eq!(driver_for(&pack.steps[6], "atspi"), "cua");
        assert_eq!(steps(true)[0].expect, Expect::Pass);
        assert_eq!(steps(false)[0].expect, Expect::SkipAllowed);
        assert_eq!(steps(true).last().unwrap().expect, Expect::SkipAllowed);
    }

    #[test]
    fn the_gpu_proof_and_cua_may_skip_while_the_rest_must_pass() {
        let pack = Pack {
            name: NAME,
            summary: "",
            steps: steps(true),
        };
        let results = run_steps(&pack, "atspi", false, &mut |step, driver| {
            let outcome = if driver == "cua" || step.id == "gpu_workload_proof" {
                StepOutcome::Skip
            } else {
                StepOutcome::Pass
            };
            result(step, driver, outcome)
        });
        assert!(passed(&pack, &results));
        let results = run_steps(&pack, "atspi", false, &mut |step, driver| {
            let outcome = if step.id == "meeting_token_coverage" {
                StepOutcome::Skip
            } else {
                StepOutcome::Pass
            };
            result(step, driver, outcome)
        });
        assert!(!passed(&pack, &results));
        assert_eq!(results[12].outcome, StepOutcome::NotRun);
    }

    #[test]
    fn cpu_forces_the_engines_and_picks_the_nfr5_model() {
        let dir = tempfile::tempdir().unwrap();
        let models = dir.path();
        let plain = Options::default();
        assert!(plain.scenario_env().is_empty());
        assert_eq!(plain.model(Some(models)), TEST_MODEL);
        let cpu = Options {
            cpu: true,
            ..Options::default()
        };
        assert_eq!(
            cpu.scenario_env()
                .get("DETTIVO_FORCE_CPU")
                .map(String::as_str),
            Some("1")
        );
        assert_eq!(cpu.model(Some(models)), TEST_MODEL);
        std::fs::create_dir_all(models.join("whisper/base.en")).unwrap();
        std::fs::write(models.join("whisper/base.en/ggml-base.en.bin"), b"x").unwrap();
        assert_eq!(cpu.model(Some(models)), CPU_MODEL);
        std::fs::create_dir_all(models.join("whisper/large-v3-turbo")).unwrap();
        std::fs::write(
            models.join("whisper/large-v3-turbo/ggml-large-v3-turbo.bin"),
            b"x",
        )
        .unwrap();
        assert_eq!(plain.model(Some(models)), GPU_MODEL);
        let named = Options {
            model: Some("small.en".into()),
            ..Options::default()
        };
        assert_eq!(named.model(None), "small.en");
    }

    #[test]
    fn an_unknown_model_is_refused_naming_the_catalogue() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let ids = catalogue_models(&root).unwrap();
        assert!(ids.iter().any(|i| i == "base.en"), "{ids:?}");
        let opts = RunOptions {
            release: Default::default(),
            repo_root: root,
            driver: "atspi".into(),
            continue_after_failure: false,
            evidence_base: PathBuf::new(),
            models_dir: None,
            timeout: std::time::Duration::from_secs(1),
            meetings: Options {
                model: Some("nope".into()),
                ..Options::default()
            },
        };
        let err = preflight(&pack(), &opts).unwrap_err();
        assert!(err.contains("nope") && err.contains("base.en"), "{err}");
        let other = by_name("dictation").unwrap();
        assert!(preflight(&other, &opts).is_ok());
    }
}
