//! The meetings pack's own step kinds (ADR 0039), run for real:
//! `diarization_der` scores the diarization engine against the
//! two-speaker turns golden through the pipeline, `meeting_throughput`
//! times the five-minute import (`meeting_throughput`),
//! `diarization_throughput` runs the diarization engine's CLI mode over
//! the same system track for its own realtime factor, and
//! `gpu_workload_proof` judges what the sampler saw (`gpu_proof`).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::{RunOptions, Step, StepKind, StepOutcome, StepResult, gpu_proof, meeting_throughput};
use crate::nfr;
use crate::pipeline_diarize;
use crate::scenarios;

/// `diarization-throughput.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiarizationThroughput {
    /// `dettivo-engine-diarize`.
    pub engine: String,
    /// The model directory.
    pub model: String,
    /// The backend the engine reported.
    pub backend: String,
    /// The actual spawned diarization process, used for GPU proof.
    #[serde(default)]
    pub engine_pid: u32,
    /// Why automatic provider selection fell back to CPU.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
    /// The audio diarized.
    pub audio_ms: u64,
    /// Wall time of the CLI run, the model load included.
    pub wall_ms: u64,
    /// `audio_ms / wall_ms`.
    pub realtime_factor: f64,
    /// `NFR-4`.
    pub nfr: String,
    /// The calibrated target.
    pub target: f64,
    /// Whether the factor reaches it.
    pub met: bool,
    /// Speakers the engine found on the single-voice track.
    pub speakers: usize,
    /// `--cpu` was passed.
    pub forced_cpu: bool,
}

fn model_dir(models_dir: Option<&Path>) -> Option<PathBuf> {
    let dir = models_dir?.join("diarize/diarization-en");
    (dir.join("segmentation.onnx").is_file() && dir.join("embedding.onnx").is_file()).then_some(dir)
}

fn elapsed_ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn result(step: &Step, outcome: StepOutcome, reason: Option<String>) -> StepResult {
    StepResult {
        id: step.id.into(),
        driver: "none".into(),
        outcome,
        duration_ms: 0,
        evidence: Some(step.id.into()),
        reason,
        surface: None,
        scan: None,
    }
}

/// `diarization_der`: the pipeline over the two-speaker fixture; the
/// report lands in the step directory. An absent model set fails naming
/// the download, because CI caches it and a desktop fetches it once.
fn diarization_der(dir: &Path, opts: &RunOptions) -> Result<String, String> {
    if model_dir(opts.models_dir.as_deref()).is_none() {
        return Err(
            "the diarization model set is not downloaded (scripts/models/fetch-diarization-model.sh)".into(),
        );
    }
    let report = pipeline_diarize::diarization_with_provider(
        &opts.repo_root,
        false,
        if opts.meetings.cpu { "cpu" } else { "auto" },
    )?;
    let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("diarization-report.json"), json).map_err(|e| e.to_string())?;
    if report.verdict == "pass" {
        Ok(report.verdict_reason)
    } else {
        Err(report.verdict_reason)
    }
}

/// `diarization_throughput`: the engine's CLI mode over the system
/// track the throughput step built (built here when it did not run).
fn diarization_throughput(
    dir: &Path,
    run_dir: &Path,
    opts: &RunOptions,
) -> Result<DiarizationThroughput, String> {
    let model = model_dir(opts.models_dir.as_deref()).ok_or(
        "the diarization model set is not downloaded (scripts/models/fetch-diarization-model.sh)",
    )?;
    let binary = scenarios::binary(&opts.repo_root, "dettivo-engine-diarize")?;
    let throughput_dir = run_dir.join("meeting_throughput");
    let fixture_dir = if throughput_dir.join("system.wav").is_file() {
        throughput_dir
    } else {
        dir.to_path_buf()
    };
    let fixture =
        meeting_throughput::fixture(&opts.repo_root, opts.models_dir.as_deref(), &fixture_dir)?;
    let audio_ms = hound::WavReader::open(&fixture.system)
        .map(|r| u64::from(r.duration()) * 1000 / u64::from(r.spec().sample_rate))
        .map_err(|e| e.to_string())?;
    let mut cmd = Command::new(&binary);
    cmd.arg("--wav")
        .arg(&fixture.system)
        .arg("--model")
        .arg(&model)
        .args(["--speakers", "1", "--json"]);
    if opts.meetings.cpu {
        cmd.arg("--cpu");
    }
    let sampler = gpu_proof::Sampler::start(std::process::id(), dir.join("gpu-samples.json"));
    let started = Instant::now();
    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{}: {e}", binary.display()))?;
    let engine_pid = child.id();
    let out = child
        .wait_with_output()
        .map_err(|e| format!("{}: {e}", binary.display()))?;
    let wall_ms = elapsed_ms(started);
    sampler.stop();
    let _ = std::fs::write(dir.join("engine.log"), &out.stderr);
    if !out.status.success() {
        return Err(format!(
            "dettivo-engine-diarize exited {}: {}",
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr)
                .trim()
                .lines()
                .last()
                .unwrap_or("")
        ));
    }
    let answer = pipeline_diarize::EngineAnswer::parse(&out.stdout)?;
    if opts.meetings.cpu && answer.backend != "cpu" {
        return Err(format!(
            "CPU diarization requested, engine reported {}",
            answer.backend
        ));
    }
    let mut labels: Vec<&str> = answer.turns.iter().map(|t| t.speaker.as_str()).collect();
    labels.sort_unstable();
    labels.dedup();
    let target = nfr::NFR4_DIARIZATION_REALTIME_GPU;
    let factor = audio_ms as f64 / wall_ms.max(1) as f64;
    let result = DiarizationThroughput {
        engine: "dettivo-engine-diarize".into(),
        model: model.to_string_lossy().into_owned(),
        backend: answer.backend,
        engine_pid,
        fallback_reason: answer.fallback_reason,
        audio_ms,
        wall_ms,
        realtime_factor: (factor * 100.0).round() / 100.0,
        nfr: target.nfr.to_string(),
        target: target.calibrated,
        met: target.met(factor),
        speakers: labels.len(),
        forced_cpu: opts.meetings.cpu,
    };
    std::fs::write(
        dir.join("diarization-throughput.json"),
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(result)
}

/// The step, run into `<run>/<id>/`.
pub fn run(step: &Step, run_dir: &Path, opts: &RunOptions) -> StepResult {
    let started = Instant::now();
    let dir = run_dir.join(step.id);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return result(
            step,
            StepOutcome::Fail,
            Some(format!("{}: {e}", dir.display())),
        );
    }
    let mut out = match step.kind {
        StepKind::DiarizationDer => match diarization_der(&dir, opts) {
            Ok(why) => result(step, StepOutcome::Pass, Some(why)),
            Err(why) => result(step, StepOutcome::Fail, Some(why)),
        },
        StepKind::MeetingThroughput => match meeting_throughput::run(opts, &dir) {
            Ok(t) => result(
                step,
                StepOutcome::Pass,
                Some(format!(
                    "{:.2}x realtime ({} ms audio in {} ms) on {} {} ({}), {} against {} {:.0}x: {}{}",
                    t.realtime_factor,
                    t.audio_ms,
                    t.wall_ms,
                    t.engine,
                    t.model,
                    t.backend,
                    t.tier,
                    t.nfr,
                    t.target,
                    if t.met { "met" } else { "not met" },
                    t.harness_warning
                        .as_deref()
                        .map(|w| format!("; warning: {w}"))
                        .unwrap_or_default()
                )),
            ),
            Err(why) => result(step, StepOutcome::Fail, Some(why)),
        },
        StepKind::DiarizationThroughput => match diarization_throughput(&dir, run_dir, opts) {
            Ok(d) => result(
                step,
                StepOutcome::Pass,
                Some(format!(
                    "{:.2}x realtime ({} ms audio in {} ms) on {}, {} against {:.0}x: {}",
                    d.realtime_factor,
                    d.audio_ms,
                    d.wall_ms,
                    d.backend,
                    d.nfr,
                    d.target,
                    if d.met { "met" } else { "not met" }
                )),
            ),
            Err(why) => result(step, StepOutcome::Fail, Some(why)),
        },
        StepKind::GpuWorkloadProof => gpu_proof_step(step, run_dir, opts),
        _ => result(
            step,
            StepOutcome::Fail,
            Some(format!("{} is not a meetings step", step.id)),
        ),
    };
    out.duration_ms = elapsed_ms(started);
    out
}

fn gpu_proof_step(step: &Step, run_dir: &Path, opts: &RunOptions) -> StepResult {
    let samples = match gpu_proof::read(run_dir) {
        Ok(s) => s,
        Err(why) => return result(step, StepOutcome::Fail, Some(why)),
    };
    let tier = std::fs::read_to_string(run_dir.join("meeting_throughput/meeting-throughput.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["tier"].as_str().map(str::to_string))
        .unwrap_or_else(|| "cpu".into());
    let _ = std::fs::copy(
        run_dir.join("meeting_throughput/gpu-samples.json"),
        run_dir.join(step.id).join("gpu-samples.json"),
    );
    let stt = gpu_proof::judge(&samples, &tier, opts.meetings.cpu);
    let diarize = diarization_proof(run_dir);
    let (outcome, why) = match (stt, diarize) {
        (gpu_proof::Verdict::Fail(why), _) | (_, Err(why)) => (StepOutcome::Fail, why),
        (gpu_proof::Verdict::Pass(why), Ok((_, diarize))) => {
            (StepOutcome::Pass, format!("{why}; {diarize}"))
        }
        (gpu_proof::Verdict::Skip(why), Ok((cuda, diarize))) => (
            if cuda {
                StepOutcome::Pass
            } else {
                StepOutcome::Skip
            },
            format!("{why}; {diarize}"),
        ),
    };
    result(step, outcome, Some(why))
}

fn diarization_proof(run_dir: &Path) -> Result<(bool, String), String> {
    let dir = run_dir.join("diarization_throughput");
    let report: DiarizationThroughput = serde_json::from_str(
        &std::fs::read_to_string(dir.join("diarization-throughput.json"))
            .map_err(|e| format!("diarization throughput evidence: {e}"))?,
    )
    .map_err(|e| format!("diarization throughput evidence: {e}"))?;
    if report.backend == "cpu" {
        return Ok((false, "diarization used CPU: no CUDA proof expected".into()));
    }
    if report.backend != "cuda" || report.engine_pid == 0 {
        return Err("diarization throughput evidence lacks CUDA backend or engine pid".into());
    }
    let samples: gpu_proof::Samples = serde_json::from_str(
        &std::fs::read_to_string(dir.join("gpu-samples.json"))
            .map_err(|e| format!("diarization GPU samples: {e}"))?,
    )
    .map_err(|e| format!("diarization GPU samples: {e}"))?;
    std::fs::copy(
        dir.join("gpu-samples.json"),
        run_dir.join("gpu_workload_proof/diarization-gpu-samples.json"),
    )
    .map_err(|e| format!("diarization GPU proof evidence: {e}"))?;
    match gpu_proof::judge_diarize(&samples, report.engine_pid) {
        gpu_proof::Verdict::Pass(why) => Ok((true, format!("diarization: {why}"))),
        gpu_proof::Verdict::Fail(why) | gpu_proof::Verdict::Skip(why) => {
            Err(format!("diarization: {why}"))
        }
    }
}
