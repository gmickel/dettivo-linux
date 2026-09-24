//! `dettivo-engine-llm`: llama.cpp in its own process (ADR 0003, ADR
//! 0026). Protocol mode (the default) speaks frames on stdin/stdout for
//! the daemon's supervisor, streaming `partial` events while it
//! generates; CLI mode takes `--prompt` and prints the same JSON the
//! protocol's `generate` response carries, so goldens and benchmarks
//! exercise the exact inference path. Logs go to stderr and never carry
//! prompt or answer text.

mod engine;
mod stop;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use dettivo_engine_proto::{
    BackendPreference, GenerateParams, LoadParams, PromptProfile, host, host_llm,
};

/// Package names of the workspace crates this binary builds on.
const UPSTREAM: &[&str] = &[dettivo_engine_proto::CRATE_NAME];

#[derive(Debug, Parser)]
#[command(
    name = "dettivo-engine-llm",
    version,
    about = "Local language model engine process (llama.cpp)"
)]
struct Cli {
    /// CLI mode: generate an answer to this prompt and print it.
    #[arg(long, value_name = "TEXT")]
    prompt: Option<String>,
    /// Model file (CLI mode).
    #[arg(long, value_name = "PATH")]
    model: Option<PathBuf>,
    /// A GGUF LoRA adapter applied over the model (CLI mode).
    #[arg(long, value_name = "PATH")]
    lora: Option<PathBuf>,
    /// System prompt (CLI mode, the polish profile).
    #[arg(long, value_name = "TEXT")]
    system: Option<String>,
    /// Feed the prompt as is, without the chat template (CLI mode).
    #[arg(long)]
    raw: bool,
    /// The most tokens to generate (CLI mode).
    #[arg(long, default_value_t = 1024)]
    max_tokens: u32,
    /// Sampling temperature; 0 is greedy (CLI mode).
    #[arg(long, default_value_t = 0.2)]
    temperature: f64,
    /// Context length in tokens (CLI mode).
    #[arg(long, value_name = "TOKENS")]
    context_length: Option<u32>,
    /// A stop string; repeatable (CLI mode).
    #[arg(long = "stop", value_name = "TEXT")]
    stop: Vec<String>,
    /// Force the CPU backend (also DETTIVO_FORCE_CPU=1).
    #[arg(long)]
    cpu: bool,
    /// The backend preference (CLI mode): `auto` falls back to the CPU
    /// with the reason, `vulkan` is Vulkan or a refusal, `cpu` never
    /// touches the GPU.
    #[arg(long, value_name = "auto|vulkan|cpu", default_value = "auto")]
    backend: String,
    /// Print the generate result as JSON instead of the text alone.
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
    match cli.prompt {
        Some(prompt) => {
            let Some(model) = cli.model else {
                eprintln!("dettivo-engine-llm: --model is required with --prompt");
                return ExitCode::from(4);
            };
            let backend_preference = match cli.backend.as_str() {
                "auto" => BackendPreference::Auto,
                "vulkan" => BackendPreference::Vulkan,
                "cpu" => BackendPreference::Cpu,
                other => {
                    eprintln!("dettivo-engine-llm: --backend {other} is not auto, vulkan or cpu");
                    return ExitCode::from(4);
                }
            };
            let load = LoadParams {
                model: model.to_string_lossy().into_owned(),
                backend_preference,
                vad_model: None,
                context_length: cli.context_length,
                lora: cli.lora.map(|p| p.to_string_lossy().into_owned()),
                threads: None,
            };
            let params = GenerateParams {
                prompt_profile: if cli.raw {
                    PromptProfile::Raw
                } else {
                    PromptProfile::Polish
                },
                system: cli.system,
                user: prompt,
                max_tokens: cli.max_tokens,
                temperature: cli.temperature,
                stop: cli.stop,
            };
            match host_llm::run_cli::<engine::Engine>(&load, &params, force_cpu) {
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
                    eprintln!("dettivo-engine-llm: {e}");
                    ExitCode::from(1)
                }
            }
        }
        None => host_llm::serve::<engine::Engine>(force_cpu, engine::last_device_usage),
    }
}
