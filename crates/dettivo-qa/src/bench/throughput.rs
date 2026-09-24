//! `stt_throughput` and `llm_rewrite`: the engines' CLI modes over the
//! fixtures. The speech clip is repeated into a five-minute WAV (one
//! minute under `--quick`) and each speech engine transcribes it once;
//! the realtime factor is the audio length over the wall time, model
//! load included (a warm-up over the clip alone records what the load
//! and a short clip cost). The Polish goldens' inputs go through
//! `dettivo-engine-llm --prompt` one process each; the tokens per second
//! subtract the faster of two load-only runs, and the latency is the
//! whole wall time a rewrite would wait.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Bench, Comparison, Spread, Status, Step};
use crate::nfr::{self, Tier};

/// The Polish goldens whose inputs the rewrite step sends.
pub const GOLDENS: &str = "crates/dettivo-language/tests/goldens/polish_fallback.json";
/// Audio the throughput step transcribes on a full run.
pub const AUDIO_SECONDS: u64 = 300;
/// Audio under `--quick`.
pub const QUICK_AUDIO_SECONDS: u64 = 60;
/// The system prompt the rewrite step sends (a fixed stand-in for the
/// Polish profile, which lives in the language crate).
pub const SYSTEM_PROMPT: &str = "You are a transcription rewrite engine. Rewrite the transcript as clean prose with punctuation and capitalisation; keep the meaning and every meaningful word; return only the rewritten text.";

/// One speech engine's throughput row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SttRow {
    /// The provider.
    pub provider: String,
    /// The model id.
    pub model: String,
    /// The backend the engine reported.
    pub backend: String,
    /// The engine's `model loaded` reason.
    pub reason: String,
    /// The audio transcribed.
    pub audio_ms: u64,
    /// Wall time of the long run, model load included.
    pub wall_ms: u64,
    /// `audio_ms / wall_ms`.
    pub realtime_factor: f64,
    /// Wall time of the warm-up over the 11 s clip, model load included.
    pub clip_wall_ms: u64,
    /// Runs of the long file.
    pub iterations: usize,
    /// The realtime factor again, the figure held against the target.
    pub figure: f64,
    /// Whether the factor meets the tier's target.
    pub met: bool,
}

/// One rewrite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rewrite {
    /// The golden case.
    pub case: String,
    /// Wall time of the whole process, load included.
    pub wall_ms: u64,
    /// Tokens generated.
    pub tokens: u64,
    /// `stop`, `length` or `cancelled`.
    pub finish_reason: String,
    /// Tokens per second over the wall time past the load-only run.
    pub tokens_per_second: Option<f64>,
}

/// The language model row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmRow {
    /// `llm`.
    pub provider: String,
    /// The model id.
    pub model: String,
    /// The backend the engine reported.
    pub backend: String,
    /// The load-only run (one token).
    pub load_ms: u64,
    /// Every rewrite.
    pub rewrites: Vec<Rewrite>,
    /// The latency spread over the rewrites (wall time).
    pub latency: Option<Spread>,
    /// Tokens per second, p50 over the rewrites that generated past the load.
    pub tokens_per_second_p50: Option<f64>,
    /// The system prompt's SHA-256.
    pub system_prompt_sha256: String,
}

/// Writes `clip` repeated until `seconds` of audio into `out`.
pub fn long_wav(clip: &Path, out: &Path, seconds: u64) -> Result<u64, String> {
    let mut reader =
        hound::WavReader::open(clip).map_err(|e| format!("{}: {e}", clip.display()))?;
    let spec = reader.spec();
    let samples: Vec<i16> = reader
        .samples::<i16>()
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let clip_ms =
        samples.len() as u64 * 1000 / u64::from(spec.sample_rate) / u64::from(spec.channels);
    if clip_ms == 0 {
        return Err("the clip is empty".into());
    }
    let repeats = (seconds * 1000).div_ceil(clip_ms);
    let mut writer = hound::WavWriter::create(out, spec).map_err(|e| e.to_string())?;
    for _ in 0..repeats {
        for s in &samples {
            writer.write_sample(*s).map_err(|e| e.to_string())?;
        }
    }
    writer.finalize().map_err(|e| e.to_string())?;
    Ok(clip_ms * repeats)
}

/// The `model loaded` reason on an engine's stderr.
fn loaded_reason(stderr: &str) -> String {
    stderr
        .lines()
        .find(|l| l.contains("model loaded"))
        .and_then(|l| {
            l.split_once("model loaded")
                .map(|(_, rest)| rest.trim().to_string())
        })
        .or_else(|| {
            stderr
                .lines()
                .find(|l| l.contains("using device"))
                .and_then(|l| {
                    l.split_once("using device")
                        .map(|(_, rest)| rest.trim().to_string())
                })
        })
        .unwrap_or_default()
}

struct CliRun {
    result: Value,
    wall_ms: u64,
    reason: String,
}

fn cli(binary: &Path, args: &[&str], cpu: bool) -> Result<CliRun, String> {
    let mut cmd = Command::new(binary);
    cmd.args(args).arg("--json");
    if cpu {
        cmd.arg("--cpu");
    }
    let at = Instant::now();
    let out = cmd
        .output()
        .map_err(|e| format!("{}: {e}", binary.display()))?;
    let wall_ms = u64::try_from(at.elapsed().as_millis()).unwrap_or(u64::MAX);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(format!(
            "{} exited {}: {}",
            binary.display(),
            out.status,
            stderr.trim().lines().last().unwrap_or("")
        ));
    }
    let result: Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("engine JSON: {e}"))?;
    Ok(CliRun {
        result,
        wall_ms,
        reason: loaded_reason(&stderr),
    })
}

/// `stt_throughput`.
pub fn stt(bench: &Bench, tier: Tier) -> Step {
    let name = "stt_throughput";
    if !bench.fixture.is_file() {
        return Step::without(
            name,
            Status::Skipped,
            "jfk.wav is missing under the model directory (scripts/models/fetch-test-model.sh)"
                .into(),
        );
    }
    let seconds = if bench.opts.quick {
        QUICK_AUDIO_SECONDS
    } else {
        AUDIO_SECONDS
    };
    let long: PathBuf = bench.run_dir.join("long.wav");
    let audio_ms = match long_wav(&bench.fixture, &long, seconds) {
        Ok(ms) => ms,
        Err(e) => return Step::without(name, Status::Failed, format!("long.wav: {e}")),
    };
    let target = nfr::stt_realtime(tier);
    let mut rows = Vec::new();
    let mut skipped = Vec::new();
    let mut failed = None;
    for provider in ["whisper", "parakeet"] {
        let model = match bench.models.get(provider) {
            Ok(m) => m.clone(),
            Err(why) => {
                skipped.push(why);
                continue;
            }
        };
        let Some(binary) = bench.engine(&format!("dettivo-engine-{provider}")) else {
            skipped.push(format!("dettivo-engine-{provider} is not built"));
            continue;
        };
        let model_arg = model.path.to_string_lossy().into_owned();
        let clip_arg = bench.fixture.to_string_lossy().into_owned();
        let long_arg = long.to_string_lossy().into_owned();
        let warm = cli(
            binary,
            &["--wav", &clip_arg, "--model", &model_arg],
            bench.opts.cpu,
        );
        let run = warm.and_then(|w| {
            cli(
                binary,
                &["--wav", &long_arg, "--model", &model_arg],
                bench.opts.cpu,
            )
            .map(|r| (w, r))
        });
        match run {
            Ok((warm, run)) => {
                let factor = audio_ms as f64 / run.wall_ms.max(1) as f64;
                rows.push(SttRow {
                    provider: provider.into(),
                    model: model.id.clone(),
                    backend: run.result["backend"].as_str().unwrap_or("").to_string(),
                    reason: run.reason,
                    audio_ms,
                    wall_ms: run.wall_ms,
                    realtime_factor: (factor * 100.0).round() / 100.0,
                    clip_wall_ms: warm.wall_ms,
                    iterations: 1,
                    figure: (factor * 100.0).round() / 100.0,
                    met: target.met(factor),
                });
            }
            Err(e) => failed = Some(format!("{provider}: {e}")),
        }
    }
    // The headline figure is the tier's engine's (ADR 0029); the other
    // rows carry their own verdict.
    let headline = rows
        .iter()
        .find(|r| r.provider == nfr::headline_provider(tier))
        .or(rows.first())
        .map(|r| r.figure);
    let status = if failed.is_some() {
        Status::Failed
    } else if rows.is_empty() {
        Status::Skipped
    } else {
        Status::Measured
    };
    let reason = match (failed, skipped.is_empty()) {
        (Some(f), true) => Some(f),
        (Some(f), false) => Some(format!("{f}; {}", skipped.join("; "))),
        (None, false) => Some(skipped.join("; ")),
        (None, true) => None,
    };
    Step {
        name: name.into(),
        status,
        reason,
        comparison: Some(Comparison::of(&target, headline)),
        results: rows
            .iter()
            .map(|r| serde_json::to_value(r).unwrap_or(Value::Null))
            .collect(),
        duration_ms: 0,
    }
}

/// The goldens' inputs, by case name.
pub fn golden_inputs(repo_root: &Path, limit: usize) -> Result<Vec<(String, String)>, String> {
    let path = repo_root.join(GOLDENS);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let goldens: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(goldens["cases"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            Some((
                c["name"].as_str()?.to_string(),
                c["input"].as_str()?.to_string(),
            ))
        })
        .take(limit)
        .collect())
}

/// `llm_rewrite`.
pub fn llm(bench: &Bench) -> Step {
    let name = "llm_rewrite";
    let model = match bench.models.get("llm") {
        Ok(m) => m.clone(),
        Err(why) => return Step::without(name, Status::Skipped, why),
    };
    let Some(binary) = bench.engine("dettivo-engine-llm") else {
        return Step::without(
            name,
            Status::Skipped,
            "dettivo-engine-llm is not built".into(),
        );
    };
    let inputs = match golden_inputs(
        &bench.opts.repo_root,
        if bench.opts.quick { 3 } else { usize::MAX },
    ) {
        Ok(i) => i,
        Err(e) => return Step::without(name, Status::Failed, e),
    };
    let model_arg = model.path.to_string_lossy().into_owned();
    // Two load-only runs, the faster one: the first also warms the page
    // cache for the model file, which a rewrite never pays again.
    let mut load: Option<CliRun> = None;
    for _ in 0..2 {
        match cli(
            binary,
            &[
                "--prompt",
                "Hello",
                "--raw",
                "--max-tokens",
                "1",
                "--model",
                &model_arg,
            ],
            bench.opts.cpu,
        ) {
            Ok(l) if load.as_ref().is_none_or(|best| l.wall_ms < best.wall_ms) => load = Some(l),
            Ok(_) => {}
            Err(e) => return Step::without(name, Status::Failed, format!("load-only run: {e}")),
        }
    }
    let Some(load) = load else {
        return Step::without(name, Status::Failed, "no load-only run".into());
    };
    let mut rewrites = Vec::new();
    for (case, input) in inputs {
        match cli(
            binary,
            &[
                "--prompt",
                &input,
                "--system",
                SYSTEM_PROMPT,
                "--max-tokens",
                "256",
                "--model",
                &model_arg,
            ],
            bench.opts.cpu,
        ) {
            Ok(run) => {
                let tokens = run.result["tokens"].as_u64().unwrap_or(0);
                let generate_ms = run.wall_ms.saturating_sub(load.wall_ms);
                rewrites.push(Rewrite {
                    case,
                    wall_ms: run.wall_ms,
                    tokens,
                    finish_reason: run.result["finish_reason"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    tokens_per_second: (generate_ms > 0 && tokens > 1).then(|| {
                        ((tokens as f64 * 1000.0 / generate_ms as f64) * 10.0).round() / 10.0
                    }),
                });
            }
            Err(e) => return Step::without(name, Status::Failed, format!("{case}: {e}")),
        }
    }
    let mut rates: Vec<u64> = rewrites
        .iter()
        .filter_map(|r| r.tokens_per_second.map(|t| (t * 10.0) as u64))
        .collect();
    rates.sort_unstable();
    let row = LlmRow {
        provider: "llm".into(),
        model: model.id.clone(),
        backend: load.result["backend"].as_str().unwrap_or("").to_string(),
        load_ms: load.wall_ms,
        latency: Spread::of(rewrites.iter().map(|r| r.wall_ms).collect()),
        tokens_per_second_p50: crate::stats::percentile(&rates, 0.5).map(|t| t as f64 / 10.0),
        rewrites,
        system_prompt_sha256: {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(SYSTEM_PROMPT.as_bytes()))
        },
    };
    Step {
        name: name.into(),
        status: Status::Measured,
        reason: Some("recorded; no NFR names a rewrite budget beyond [llm] timeout_ms".into()),
        comparison: None,
        results: vec![serde_json::to_value(&row).unwrap_or(Value::Null)],
        duration_ms: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_long_wav_repeats_the_clip_to_the_length_asked() {
        let dir = tempfile::tempdir().unwrap();
        let clip = dir.path().join("clip.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&clip, spec).unwrap();
        for i in 0..16_000 {
            w.write_sample((i % 100) as i16).unwrap();
        }
        w.finalize().unwrap();
        let out = dir.path().join("long.wav");
        assert_eq!(long_wav(&clip, &out, 3).unwrap(), 3000);
        let reader = hound::WavReader::open(&out).unwrap();
        assert_eq!(reader.duration(), 48_000);
    }

    #[test]
    fn the_reason_comes_from_the_loaded_line_and_the_goldens_have_inputs() {
        assert_eq!(
            loaded_reason("x\nINFO model loaded backend=Vulkan reason=built in\n"),
            "backend=Vulkan reason=built in"
        );
        assert_eq!(
            loaded_reason("INFO load: using device Vulkan0 (RTX) - 17887 MiB free\n"),
            "Vulkan0 (RTX) - 17887 MiB free"
        );
        assert_eq!(loaded_reason("nothing"), "");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let inputs = golden_inputs(&root, 2).unwrap();
        assert_eq!(inputs.len(), 2);
        assert!(!inputs[0].1.is_empty());
    }
}
