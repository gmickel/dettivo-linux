//! `dettivo-engine-nemotron`: NVIDIA Nemotron 3 Diarization through
//! NeMo-Speech.cpp in its own process (ADR 0003, ADR 0073). Protocol mode
//! (the default) speaks frames on stdin/stdout for the daemon's
//! supervisor; CLI mode takes a WAV and prints the same JSON the protocol's
//! `diarize` response carries, so the fixture, the bench and `dettivo
//! doctor` exercise the exact path. NeMo-Speech.cpp and its ggml are
//! linked into this binary alone.

mod backend;
mod engine;
mod model;
mod turns;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use dettivo_engine_proto::{BackendPreference, DiarizeParams, LoadParams, host, host_diarize};

/// Package names of the workspace crates this binary builds on.
const UPSTREAM: &[&str] = &[dettivo_engine_proto::CRATE_NAME];

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Provider {
    Auto,
    Cpu,
    Cuda,
    Vulkan,
}

impl From<Provider> for BackendPreference {
    fn from(provider: Provider) -> Self {
        match provider {
            Provider::Auto => Self::Auto,
            Provider::Cpu => Self::Cpu,
            Provider::Cuda => Self::Cuda,
            Provider::Vulkan => Self::Vulkan,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "dettivo-engine-nemotron",
    version,
    about = "Speaker diarization engine process (Nemotron 3 Diarization, NeMo-Speech.cpp)"
)]
struct Cli {
    /// CLI mode: diarize this WAV file and print JSON.
    #[arg(long, value_name = "FILE")]
    wav: Option<PathBuf>,
    /// The model directory, or the GGUF itself (CLI mode).
    #[arg(long, value_name = "PATH")]
    model: Option<PathBuf>,
    /// The speaker count when known (at most eight); the channels over
    /// the speech floor otherwise.
    #[arg(long)]
    speakers: Option<u32>,
    /// Accepted for the other diarization engine's command line; ignored.
    #[arg(long)]
    threshold: Option<f64>,
    /// Accepted for the other diarization engine's command line; the
    /// runtime runs its CPU graphs on four threads.
    #[arg(long, default_value_t = 0)]
    threads: u32,
    /// Backend: auto tries the build's GPU backend, otherwise the CPU.
    #[arg(long, value_enum)]
    provider: Option<Provider>,
    /// Force the CPU backend (also DETTIVO_FORCE_CPU=1).
    #[arg(long)]
    cpu: bool,
    /// Print JSON (CLI mode; the only output format).
    #[arg(long)]
    json: bool,
    /// Also write the per-frame speaker probabilities here (CLI mode;
    /// `.npy`, float32, frames by speakers).
    #[arg(long, value_name = "FILE")]
    probs: Option<PathBuf>,
}

/// Writes `data` (`rows` by `cols`, row-major) as a version 1 `.npy`.
fn write_npy(path: &Path, data: &[u8], rows: u64, cols: u32) -> std::io::Result<()> {
    let mut header =
        format!("{{'descr': '<f4', 'fortran_order': False, 'shape': ({rows}, {cols}), }}");
    // Magic, version and length take ten bytes; the header ends in a
    // newline and pads the whole preamble to a multiple of 64.
    while (10 + header.len() + 1) % 64 != 0 {
        header.push(' ');
    }
    header.push('\n');
    let mut out = Vec::with_capacity(10 + header.len() + data.len());
    out.extend_from_slice(b"\x93NUMPY\x01\x00");
    out.extend_from_slice(&(header.len() as u16).to_le_bytes());
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(data);
    std::fs::write(path, out)
}

fn cli_mode(cli: Cli, wav: PathBuf, force_cpu: bool) -> ExitCode {
    let Some(model) = cli.model else {
        eprintln!("dettivo-engine-nemotron: --model is required with --wav");
        return ExitCode::from(4);
    };
    let load = LoadParams {
        model: model.to_string_lossy().into_owned(),
        backend_preference: cli.provider.map(Into::into).unwrap_or_default(),
        vad_model: None,
        context_length: None,
        lora: None,
        threads: None,
    };
    let params = DiarizeParams {
        speakers: cli.speakers,
        clustering_threshold: None,
        frame_probabilities: cli.probs.is_some(),
    };
    let pcm = match host::read_wav(&wav) {
        Ok(pcm) => pcm,
        Err(e) => {
            eprintln!("dettivo-engine-nemotron: {e}");
            return ExitCode::from(1);
        }
    };
    let started = std::time::Instant::now();
    let mut progress = |done: u32, total: u32| {
        tracing::info!(completed = done, total, "progress");
    };
    let (json, result) = match host_diarize::run_cli::<engine::Nemotron>(
        &pcm,
        &load,
        &params,
        force_cpu,
        &mut progress,
    ) {
        Ok(out) => out,
        Err(e) => {
            eprintln!("dettivo-engine-nemotron: {e}");
            return ExitCode::from(1);
        }
    };
    tracing::info!(
        audio_ms = pcm.len() as u64 * 1000 / 16_000,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "diarized"
    );
    if let (Some(path), Some(frames)) = (&cli.probs, &result.frames)
        && let Err(e) = write_npy(path, &result.probabilities, frames.count, frames.speakers)
    {
        eprintln!("dettivo-engine-nemotron: {}: {e}", path.display());
        return ExitCode::from(1);
    }
    if cli.json {
        println!("{json}");
    } else {
        for t in &result.turns {
            println!("{:>9} {:>9}  {}", t.start_ms, t.end_ms, t.speaker);
        }
    }
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let _ = UPSTREAM;
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let force_cpu = cli.cpu || host::env_flag("DETTIVO_FORCE_CPU");
    let _ = (cli.threshold, cli.threads);
    match cli.wav.clone() {
        Some(wav) => cli_mode(cli, wav, force_cpu),
        None => host_diarize::serve::<engine::Nemotron>(force_cpu, cli.provider.map(Into::into)),
    }
}
