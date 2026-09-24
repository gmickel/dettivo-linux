//! `dettivo-engine-diarize`: sherpa-onnx speaker diarization in its own
//! process (ADR 0003, ADR 0035). Protocol mode (the default) speaks frames
//! on stdin/stdout for the daemon's supervisor; CLI mode takes a WAV and
//! prints the same JSON the protocol's `diarize` response carries, so the
//! fixture, the benchmark and `dettivo doctor` exercise the exact path.
//! ONNX Runtime is linked into this binary alone.

mod engine;
mod provider;

use std::path::PathBuf;
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
}

impl From<Provider> for BackendPreference {
    fn from(provider: Provider) -> Self {
        match provider {
            Provider::Auto => Self::Auto,
            Provider::Cpu => Self::Cpu,
            Provider::Cuda => Self::Cuda,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "dettivo-engine-diarize",
    version,
    about = "Speaker diarization engine process (sherpa-onnx)"
)]
struct Cli {
    /// CLI mode: diarize this WAV file and print JSON.
    #[arg(long, value_name = "FILE")]
    wav: Option<PathBuf>,
    /// Model directory holding segmentation.onnx and embedding.onnx (CLI mode).
    #[arg(long, value_name = "DIR")]
    model: Option<PathBuf>,
    /// The speaker count when known; the clustering decides otherwise.
    #[arg(long)]
    speakers: Option<u32>,
    /// Initial average-linkage cosine cutoff before low-support reassignment.
    #[arg(long)]
    threshold: Option<f64>,
    /// Threads per model; 0 is the core count capped at four.
    #[arg(long, default_value_t = 0)]
    threads: u32,
    /// Provider: auto tries CUDA when built in and available, otherwise CPU.
    #[arg(long, value_enum)]
    provider: Option<Provider>,
    /// Force the CPU backend (also DETTIVO_FORCE_CPU=1).
    #[arg(long)]
    cpu: bool,
    /// Print JSON (CLI mode; the only output format).
    #[arg(long)]
    json: bool,
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
    match cli.wav {
        Some(wav) => {
            let Some(model) = cli.model else {
                eprintln!("dettivo-engine-diarize: --model is required with --wav");
                return ExitCode::from(4);
            };
            let load = LoadParams {
                model: model.to_string_lossy().into_owned(),
                backend_preference: cli.provider.map(Into::into).unwrap_or_default(),
                vad_model: None,
                context_length: None,
                lora: None,
                threads: (cli.threads > 0).then_some(cli.threads),
            };
            let params = DiarizeParams {
                speakers: cli.speakers,
                clustering_threshold: cli.threshold,
            };
            let pcm = match host::read_wav(&wav) {
                Ok(pcm) => pcm,
                Err(e) => {
                    eprintln!("dettivo-engine-diarize: {e}");
                    return ExitCode::from(1);
                }
            };
            let started = std::time::Instant::now();
            let mut progress = |done: u32, total: u32| {
                tracing::info!(completed = done, total, "progress");
            };
            match host_diarize::run_cli::<engine::Diarizer>(
                &pcm,
                &load,
                &params,
                force_cpu,
                &mut progress,
            ) {
                Ok(json) => {
                    let audio_ms = pcm.len() as u64 * 1000 / 16_000;
                    tracing::info!(
                        audio_ms,
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "diarized"
                    );
                    if cli.json {
                        println!("{json}");
                    } else {
                        let turns = serde_json::from_str::<serde_json::Value>(&json)
                            .ok()
                            .and_then(|v| v["turns"].as_array().cloned())
                            .unwrap_or_default();
                        for t in turns {
                            println!("{:>9} {:>9}  {}", t["start_ms"], t["end_ms"], t["speaker"]);
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("dettivo-engine-diarize: {e}");
                    ExitCode::from(1)
                }
            }
        }
        None => host_diarize::serve::<engine::Diarizer>(force_cpu, cli.provider.map(Into::into)),
    }
}
