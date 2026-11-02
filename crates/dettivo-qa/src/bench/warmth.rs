//! `warm_up` and `gpu_threads` (ADR 0084, ADR 0085): what a take costs on
//! an engine that is cold, loaded or warm, the GPU memory a loaded engine
//! holds while idle, and the CPU thread count a GPU engine runs fastest
//! with. Each speech engine with its model on disk runs over the engine
//! protocol, the way the supervisor drives it, on the 11 s fixture.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::engine_client::{EngineClient, gpu_used_mib, ms, read_pcm};
use super::{Bench, Spread, Status, Step};
use crate::nfr::Tier;

/// A warmed first take within this factor of the warm takes passes.
pub const WARM_FACTOR: f64 = 1.2;

/// The thread counts `gpu_threads` compares.
pub const THREAD_COUNTS: &[u32] = &[1, 2, 4, 8, 16];

/// One engine's `warm_up` row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WarmRow {
    /// The provider.
    pub provider: String,
    /// The model id.
    pub model: String,
    /// The backend the engine loaded on.
    pub backend: String,
    /// Process start to the `load` answer.
    pub load_ms: u64,
    /// The first take on a loaded engine with no warm-up.
    pub loaded_take_ms: u64,
    /// A take on an engine nothing ran: `load_ms + loaded_take_ms`.
    pub cold_take_ms: u64,
    /// The warm-up itself (30 s of silence) in a second process.
    pub warm_up_ms: u64,
    /// The first take after the warm-up.
    pub warmed_take_ms: u64,
    /// The takes after that.
    pub warm: Option<Spread>,
    /// `warmed_take_ms` over the warm p50.
    pub warmed_ratio: Option<f64>,
    /// `warmed_ratio` within `WARM_FACTOR`.
    pub warmed_met: Option<bool>,
    /// GPU memory the loaded engine holds while idle (MiB), when
    /// `nvidia-smi` reports it.
    pub idle_vram_mib: Option<u64>,
}

/// One engine's `gpu_threads` row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThreadRow {
    /// The provider.
    pub provider: String,
    /// The model id.
    pub model: String,
    /// The backend every count loaded on.
    pub backend: String,
    /// The warm takes per thread count.
    pub counts: Vec<ThreadCount>,
    /// The count with the lowest p50.
    pub fastest: Option<u32>,
}

/// The warm takes at one thread count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThreadCount {
    /// The `threads` the load named.
    pub threads: u32,
    /// The warm takes.
    pub warm: Option<Spread>,
}

/// One engine to measure: the provider, the model id, the binary and the
/// model file.
type Measured<'a> = (String, String, &'a Path, &'a Path);

/// The speech engines the steps measure, with their binary and model.
fn engines(bench: &Bench) -> (Vec<Measured<'_>>, Vec<String>) {
    let mut found = Vec::new();
    let mut skipped = Vec::new();
    for provider in ["whisper", "parakeet"] {
        let binary = format!("dettivo-engine-{provider}");
        match (bench.models.get(provider), bench.engine(&binary)) {
            (Ok(model), Some(path)) => found.push((
                provider.to_string(),
                model.id.clone(),
                path,
                model.path.as_path(),
            )),
            (Err(why), _) => skipped.push(why),
            (_, None) => skipped.push(format!("{binary} is not built")),
        }
    }
    (found, skipped)
}

fn spawn(bench: &Bench, binary: &Path, step: &str) -> Result<EngineClient, String> {
    let log = bench.run_dir.join(format!("{step}-engine.log"));
    EngineClient::spawn(binary, bench.opts.cpu, &log)
}

fn backend(loaded: &Value) -> String {
    loaded["backend"].as_str().unwrap_or("unknown").to_string()
}

fn takes(engine: &mut EngineClient, pcm: &[i16], n: usize) -> Result<Option<Spread>, String> {
    let mut samples = Vec::with_capacity(n);
    for _ in 0..n {
        samples.push(ms(engine.recognize(pcm, "en")?.1));
    }
    Ok(Spread::of(samples))
}

fn warm_row(
    bench: &Bench,
    (provider, model, binary, path): &Measured<'_>,
    pcm: &[i16],
) -> Result<WarmRow, String> {
    let before = gpu_used_mib();
    let mut engine = spawn(bench, binary, "warm_up")?;
    let (loaded, load) = engine.load(path, None)?;
    // The engine settles before its idle footprint is read.
    std::thread::sleep(Duration::from_secs(2));
    let idle_vram_mib = before.zip(gpu_used_mib()).map(|(b, a)| a.saturating_sub(b));
    let loaded_take = ms(engine.recognize(pcm, "en")?.1);
    drop(engine);
    let mut engine = spawn(bench, binary, "warm_up")?;
    engine.load(path, None)?;
    let warm_up = ms(engine.warm_up()?);
    let warmed_take = ms(engine.recognize(pcm, "en")?.1);
    let warm = takes(&mut engine, pcm, bench.iterations())?;
    let warmed_ratio = warm
        .as_ref()
        .filter(|w| w.p50 > 0)
        .map(|w| warmed_take as f64 / w.p50 as f64);
    Ok(WarmRow {
        provider: provider.clone(),
        model: model.clone(),
        backend: backend(&loaded),
        load_ms: ms(load),
        loaded_take_ms: loaded_take,
        cold_take_ms: ms(load) + loaded_take,
        warm_up_ms: warm_up,
        warmed_take_ms: warmed_take,
        warm,
        warmed_ratio,
        warmed_met: warmed_ratio.map(|r| r <= WARM_FACTOR),
        idle_vram_mib,
    })
}

fn thread_row(
    bench: &Bench,
    (provider, model, binary, path): &Measured<'_>,
    pcm: &[i16],
) -> Result<ThreadRow, String> {
    let mut counts = Vec::new();
    let mut backends = Vec::new();
    for &threads in THREAD_COUNTS {
        let mut engine = spawn(bench, binary, "gpu_threads")?;
        let (loaded, _) = engine.load(path, Some(threads))?;
        backends.push(backend(&loaded));
        engine.warm_up()?;
        counts.push(ThreadCount {
            threads,
            warm: takes(&mut engine, pcm, bench.iterations())?,
        });
    }
    backends.dedup();
    let fastest = counts
        .iter()
        .filter_map(|c| c.warm.as_ref().map(|w| (w.p50, c.threads)))
        .min()
        .map(|(_, t)| t);
    Ok(ThreadRow {
        provider: provider.clone(),
        model: model.clone(),
        backend: backends.join("/"),
        counts,
        fastest,
    })
}

fn step<T: Serialize>(
    name: &str,
    bench: &Bench,
    row: impl Fn(&Bench, &Measured<'_>, &[i16]) -> Result<T, String>,
) -> Step {
    let pcm = match read_pcm(&bench.fixture) {
        Ok(pcm) => pcm,
        Err(why) => return Step::without(name, Status::Skipped, why),
    };
    let (found, skipped) = engines(bench);
    let mut results = Vec::new();
    let mut failed = Vec::new();
    for engine in &found {
        match row(bench, engine, &pcm) {
            Ok(r) => results.push(serde_json::to_value(r).unwrap_or(Value::Null)),
            Err(e) => failed.push(format!("{}: {e}", engine.0)),
        }
    }
    let status = if !failed.is_empty() {
        Status::Failed
    } else if results.is_empty() {
        Status::Skipped
    } else {
        Status::Measured
    };
    let reason = [failed, skipped].concat();
    Step {
        name: name.into(),
        status,
        reason: (!reason.is_empty()).then(|| reason.join("; ")),
        comparison: None,
        results,
        duration_ms: 0,
    }
}

/// `warm_up`: per engine, the load, the first take without and with the
/// warm-up, the warm takes, and the idle GPU memory.
pub fn warm_up(bench: &Bench) -> Step {
    step("warm_up", bench, warm_row)
}

/// `gpu_threads`: per engine on the GPU tier, the warm takes at each of
/// `THREAD_COUNTS`; skipped on the CPU tier.
pub fn gpu_threads(bench: &Bench, tier: Tier) -> Step {
    if tier == Tier::Cpu {
        return Step::without(
            "gpu_threads",
            Status::Skipped,
            "the CPU tier: the thread count of a GPU engine is measured on the GPU tier".into(),
        );
    }
    step("gpu_threads", bench, thread_row)
}
