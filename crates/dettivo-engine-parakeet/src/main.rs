//! `dettivo-engine-parakeet`: parakeet.cpp in its own process (ADR 0003,
//! ADR 0018). Protocol mode (the default) speaks frames on stdin/stdout
//! for the daemon's supervisor; CLI mode takes a WAV and prints the same
//! JSON the protocol would, so benchmarks, the WER fixtures and the
//! alignment spike exercise the exact inference path. Logs go to stderr
//! and never carry transcript text.

mod align;
mod bias;
mod engine;
mod ffi;
mod gguf;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use dettivo_engine_proto::{
    BackendPreference, BiasTerm, LoadParams, RecognizeParams, VocabularyBias, host,
};

/// Package names of the workspace crates this binary builds on.
const UPSTREAM: &[&str] = &[dettivo_engine_proto::CRATE_NAME];

#[derive(Debug, Parser)]
#[command(
    name = "dettivo-engine-parakeet",
    version,
    about = "Parakeet speech recognition engine process"
)]
struct Cli {
    /// CLI mode: recognize this WAV file and print JSON.
    #[arg(long, value_name = "FILE")]
    wav: Option<PathBuf>,
    /// Model file (CLI mode).
    #[arg(long, value_name = "PATH")]
    model: Option<PathBuf>,
    /// Language code or `auto` (CLI mode).
    #[arg(long, default_value = "auto")]
    language: String,
    /// Force the CPU backend (also DETTIVO_FORCE_CPU=1).
    #[arg(long)]
    cpu: bool,
    /// Print the recognize result as JSON instead of the transcript text.
    #[arg(long)]
    json: bool,
    /// Vocabulary to rescore towards (CLI mode): a JSON list of
    /// `{"term": ..., "aliases": [...]}`.
    #[arg(long, value_name = "JSON")]
    vocabulary: Option<String>,
    /// Vocabulary boost (CLI mode): off, light, normal or strong.
    #[arg(long, default_value = "normal", value_name = "LEVEL")]
    boost: String,
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
                eprintln!("dettivo-engine-parakeet: --model is required with --wav");
                return ExitCode::from(4);
            };
            let load = LoadParams {
                model: model.to_string_lossy().into_owned(),
                backend_preference: BackendPreference::Auto,
                vad_model: None,
                context_length: None,
                lora: None,
                threads: None,
            };
            let vocabulary = match cli.vocabulary.as_deref().map(|json| {
                let terms = serde_json::from_str::<Vec<BiasTerm>>(json)
                    .map_err(|e| format!("--vocabulary: {e}"))?;
                let boost = VocabularyBias::boost_of(&cli.boost).ok_or_else(|| {
                    format!("--boost {}: not off, light, normal or strong", cli.boost)
                })?;
                Ok::<_, String>(VocabularyBias { terms, boost })
            }) {
                None => None,
                Some(Ok(bias)) => Some(bias),
                Some(Err(why)) => {
                    eprintln!("dettivo-engine-parakeet: {why}");
                    return ExitCode::from(4);
                }
            };
            let params = RecognizeParams {
                language: cli.language,
                prompt: None,
                timestamps: true,
                vocabulary,
            };
            match host::run_cli::<engine::Engine>(&wav, &load, &params, force_cpu) {
                Ok(json) => {
                    if cli.json {
                        println!("{json}");
                    } else {
                        let text = serde_json::from_str::<serde_json::Value>(&json)
                            .ok()
                            .and_then(|v| v["text"].as_str().map(str::to_string))
                            .unwrap_or_default();
                        println!("{text}");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("dettivo-engine-parakeet: {e}");
                    ExitCode::from(1)
                }
            }
        }
        None => host::serve::<engine::Engine>(force_cpu),
    }
}
