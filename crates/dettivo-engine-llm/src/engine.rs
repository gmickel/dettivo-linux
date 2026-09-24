//! llama.cpp behind the `LanguageEngine` trait: the backend choice (Vulkan
//! when the build carries it, a device enumerates and the file fits its
//! free memory; CPU otherwise, with the reason), the load with every layer
//! on the chosen device, the prompt profiles, and the token loop that
//! streams pieces, honours stop strings (`stop`: a stop that spans tokens
//! is held back, never streamed) and stops between two tokens on a
//! cancel. A context is created per `generate` (its KV cache is the size
//! of `context_length`), so the model's weights are the only thing that
//! stays on the device between requests.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use dettivo_engine_proto::host_llm::LanguageEngine;
use dettivo_engine_proto::{
    Backend, BackendPreference, EngineError, FinishReason, GenerateParams, GenerateResult,
    LoadParams, LoadedResult, PromptProfile, backend,
};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaLoraAdapter, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::LlamaToken;
use llama_cpp_2::{LlamaBackendDeviceType, list_llama_ggml_backend_devices};

use crate::stop::Streamed;

/// The context length when `load` names none.
pub const DEFAULT_CONTEXT_LENGTH: u32 = 4096;
/// Tokens fed per decode call while the prompt is consumed.
const N_BATCH: u32 = 512;
/// The sampler seed, fixed so a golden run repeats itself.
const SEED: u32 = 42;
/// The ggml device index of the last load; `usize::MAX` means the CPU.
static LAST_DEVICE: AtomicUsize = AtomicUsize::new(usize::MAX);

fn backend_init() -> &'static LlamaBackend {
    static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();
    BACKEND.get_or_init(|| {
        llama_cpp_2::send_logs_to_tracing(
            llama_cpp_2::LogOptions::default().with_logs_enabled(true),
        );
        LlamaBackend::init().expect("llama backend initialises once")
    })
}

/// The ggml Vulkan device to load on, when the build carries the backend
/// and one enumerates: a discrete GPU before an integrated one, then the
/// most memory.
fn vulkan_device() -> Option<llama_cpp_2::LlamaBackendDevice> {
    list_llama_ggml_backend_devices()
        .into_iter()
        .filter(|d| {
            d.backend.eq_ignore_ascii_case("vulkan")
                && matches!(
                    d.device_type,
                    LlamaBackendDeviceType::Gpu | LlamaBackendDeviceType::IntegratedGpu
                )
        })
        .max_by_key(|d| (d.device_type == LlamaBackendDeviceType::Gpu, d.memory_total))
}

/// Bytes in use on `device` (the CPU device for `None`), as ggml reports
/// them: total minus free.
pub fn device_usage(device: Option<usize>) -> Option<u64> {
    backend_init();
    let devices = list_llama_ggml_backend_devices();
    let d = match device {
        Some(i) => devices.into_iter().find(|d| d.index == i),
        None => devices
            .into_iter()
            .find(|d| d.device_type == LlamaBackendDeviceType::Cpu),
    }?;
    // A device that reports no total (the CPU device on some builds) has
    // no number to give.
    (d.memory_total > 0).then(|| d.memory_total.saturating_sub(d.memory_free) as u64)
}

/// Bytes in use on the device of the last load, for `status` after an
/// `unload`; before any load, the device a load would pick, so a reading
/// taken by a fresh process compares with one taken after the unload
/// (the idle-unload evidence reads the drop).
pub fn last_device_usage() -> Option<u64> {
    match LAST_DEVICE.load(Ordering::Relaxed) {
        usize::MAX => match vulkan_device() {
            Some(d) if cfg!(feature = "vulkan") => device_usage(Some(d.index)),
            _ => device_usage(None),
        },
        i => device_usage(Some(i)),
    }
}

/// A loaded model, with the LoRA adapter `load` named applied over it in
/// every context (ADR 0032).
pub struct Engine {
    model: LlamaModel,
    lora: Option<LlamaLoraAdapter>,
    loaded: LoadedResult,
    device: Option<usize>,
    n_ctx: u32,
    threads: i32,
}

/// The weight a sideloaded adapter is applied with: the fine-tune as
/// trained, neither damped nor amplified.
const LORA_SCALE: f32 = 1.0;

fn threads() -> i32 {
    let n = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    i32::try_from((n / 2).clamp(2, 16)).unwrap_or(4)
}

fn load_failed(path: &str, reason: impl std::fmt::Display) -> EngineError {
    EngineError::new("load_failed", format!("{path}: {reason}"))
}

impl Engine {
    fn try_load(path: &str, device: Option<usize>) -> Result<LlamaModel, EngineError> {
        let mut params = LlamaModelParams::default();
        params = match device {
            Some(i) => params
                .with_n_gpu_layers(1000)
                .with_devices(&[i])
                .map_err(|e| load_failed(path, e))?,
            None => params.with_n_gpu_layers(0),
        };
        LlamaModel::load_from_file(backend_init(), path, &params)
            .map_err(|e| load_failed(path, format!("llama.cpp could not load the model ({e})")))
    }
}

impl LanguageEngine for Engine {
    fn load(params: &LoadParams, force_cpu: bool) -> Result<Self, EngineError> {
        let path = params.model.as_str();
        let meta = std::fs::metadata(path).map_err(|e| {
            EngineError::new(
                "model_missing",
                format!("model file not found: {path} ({e})"),
            )
        })?;
        if !meta.is_file() {
            return Err(EngineError::new(
                "model_missing",
                format!("model file not found: {path} (not a file)"),
            ));
        }
        backend_init();
        let (mut chosen, mut reason) = backend::choose(
            params.backend_preference,
            force_cpu,
            cfg!(feature = "vulkan"),
        )
        .map_err(|why| load_failed(path, backend::strict_vulkan_refused(&why)))?;
        let mut device = None;
        if chosen == Backend::Vulkan {
            match vulkan_device() {
                None => {
                    if params.backend_preference == BackendPreference::Vulkan {
                        return Err(load_failed(
                            path,
                            backend::strict_vulkan_refused("no Vulkan device enumerates in ggml"),
                        ));
                    }
                    chosen = Backend::Cpu;
                    reason = "no Vulkan device enumerates in ggml".into();
                }
                Some(d) if (meta.len() as usize) > d.memory_free => {
                    let why = format!(
                        "the model ({} bytes) exceeds the free memory of {} ({} bytes)",
                        meta.len(),
                        d.description,
                        d.memory_free
                    );
                    if params.backend_preference == BackendPreference::Vulkan {
                        return Err(load_failed(path, why));
                    }
                    chosen = Backend::Cpu;
                    reason = format!("{why}; CPU fallback");
                }
                Some(d) => {
                    reason = format!("Vulkan device {} ({})", d.name, d.description);
                    device = Some(d.index);
                }
            }
        }
        let model = match Self::try_load(path, device) {
            Ok(m) => m,
            Err(e) if device.is_some() && params.backend_preference == BackendPreference::Auto => {
                tracing::warn!(error = %e, "Vulkan load failed; loading on the CPU");
                reason = format!("Vulkan load failed ({}); CPU fallback", e.message);
                chosen = Backend::Cpu;
                device = None;
                Self::try_load(path, None)?
            }
            Err(e) => return Err(e),
        };
        LAST_DEVICE.store(device.unwrap_or(usize::MAX), Ordering::Relaxed);
        let lora = match params.lora.as_deref() {
            Some(adapter) => {
                if !std::path::Path::new(adapter).is_file() {
                    return Err(EngineError::new(
                        "model_missing",
                        format!("lora adapter not found: {adapter}"),
                    ));
                }
                let loaded = model.lora_adapter_init(adapter).map_err(|e| {
                    load_failed(
                        adapter,
                        format!("llama.cpp could not load the adapter ({e})"),
                    )
                })?;
                reason = format!("{reason}; LoRA adapter applied");
                Some(loaded)
            }
            None => None,
        };
        let n_ctx = params
            .context_length
            .unwrap_or(DEFAULT_CONTEXT_LENGTH)
            .clamp(256, model.n_ctx_train().max(256));
        tracing::info!(backend = ?chosen, n_ctx, lora = lora.is_some(), "model loaded");
        Ok(Self {
            model,
            lora,
            loaded: LoadedResult {
                model: path.to_string(),
                backend: chosen,
                reason,
                fallback_reason: None,
            },
            device,
            n_ctx,
            threads: threads(),
        })
    }

    fn loaded(&self) -> LoadedResult {
        self.loaded.clone()
    }

    fn memory_bytes(&self) -> Option<u64> {
        device_usage(self.device)
    }

    fn generate(
        &mut self,
        params: &GenerateParams,
        partial: &mut dyn FnMut(&str),
        cancelled: &mut dyn FnMut() -> bool,
    ) -> Result<GenerateResult, EngineError> {
        let prompt = self.prompt(params)?;
        let tokens = self
            .model
            .str_to_token(&prompt, AddBos::Never)
            .map_err(|e| EngineError::new("bad_request", format!("tokenize: {e}")))?;
        let n_ctx = self.n_ctx as usize;
        if tokens.len() + 8 > n_ctx {
            return Err(EngineError::new(
                "bad_request",
                format!(
                    "prompt of {} tokens exceeds the context of {n_ctx}",
                    tokens.len()
                ),
            ));
        }
        let budget = (n_ctx - tokens.len()).min(params.max_tokens.max(1) as usize);
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(std::num::NonZeroU32::new(self.n_ctx))
            .with_n_batch(N_BATCH.min(self.n_ctx))
            .with_n_threads(self.threads)
            .with_n_threads_batch(self.threads);
        let mut ctx = self
            .model
            .new_context(backend_init(), ctx_params)
            .map_err(|e| EngineError::new("internal", format!("context: {e}")))?;
        if let Some(adapter) = self.lora.as_mut() {
            ctx.lora_adapter_set(adapter, LORA_SCALE)
                .map_err(|e| EngineError::new("internal", format!("lora: {e}")))?;
        }
        let mut batch = LlamaBatch::new(N_BATCH.min(self.n_ctx) as usize, 1);
        let mut pos: i32 = 0;
        for chunk in tokens.chunks(N_BATCH.min(self.n_ctx) as usize) {
            batch.clear();
            for (i, token) in chunk.iter().enumerate() {
                let last = i + 1 == chunk.len();
                batch
                    .add(*token, pos, &[0], last)
                    .map_err(|e| EngineError::new("internal", format!("batch: {e}")))?;
                pos += 1;
            }
            ctx.decode(&mut batch)
                .map_err(|e| EngineError::new("internal", format!("decode: {e}")))?;
        }
        let mut sampler = sampler(params.temperature);
        let mut streamed = Streamed::default();
        let mut bytes: Vec<u8> = Vec::new();
        let mut generated: u32 = 0;
        let mut finish = FinishReason::Length;
        while (generated as usize) < budget {
            if cancelled() {
                finish = FinishReason::Cancelled;
                break;
            }
            let token = sampler.sample(&ctx, -1);
            if self.model.is_eog_token(token) {
                finish = FinishReason::Stop;
                break;
            }
            generated += 1;
            bytes.extend(self.piece(token)?);
            let valid = match std::str::from_utf8(&bytes) {
                Ok(s) => s.len(),
                Err(e) => e.valid_up_to(),
            };
            if valid > 0 {
                let piece: String = String::from_utf8_lossy(&bytes[..valid]).into_owned();
                bytes.drain(..valid);
                if streamed.push(&piece, &params.stop, partial) {
                    finish = FinishReason::Stop;
                    break;
                }
            }
            batch.clear();
            batch
                .add(token, pos, &[0], true)
                .map_err(|e| EngineError::new("internal", format!("batch: {e}")))?;
            pos += 1;
            ctx.decode(&mut batch)
                .map_err(|e| EngineError::new("internal", format!("decode: {e}")))?;
        }
        Ok(GenerateResult {
            text: streamed.finish(partial),
            tokens: generated,
            finish_reason: finish,
            backend: self.loaded.backend,
        })
    }
}

impl Engine {
    /// The model input for `params` under its prompt profile.
    fn prompt(&self, params: &GenerateParams) -> Result<String, EngineError> {
        match params.prompt_profile {
            PromptProfile::Raw => Ok(params.user.clone()),
            PromptProfile::Polish => {
                let template = self.model.chat_template(None).map_err(|e| {
                    EngineError::new(
                        "bad_request",
                        format!("the model has no chat template: {e}"),
                    )
                })?;
                let mut chat = Vec::new();
                if let Some(system) = params.system.as_deref().filter(|s| !s.trim().is_empty()) {
                    chat.push(message("system", system)?);
                }
                chat.push(message("user", &params.user)?);
                let mut prompt = self
                    .model
                    .apply_chat_template(&template, &chat, true)
                    .map_err(|e| EngineError::new("bad_request", format!("chat template: {e}")))?;
                // A thinking model (Qwen3) answers straight away when the
                // assistant turn opens with an empty thinking block.
                if template.to_string().is_ok_and(|t| t.contains("<think>")) {
                    prompt.push_str("<think>\n\n</think>\n\n");
                }
                Ok(prompt)
            }
        }
    }

    /// The bytes of one token, special tokens rendered as nothing.
    fn piece(&self, token: LlamaToken) -> Result<Vec<u8>, EngineError> {
        use llama_cpp_2::TokenToStringError;
        match self.model.token_to_piece_bytes(token, 16, false, None) {
            Ok(b) => Ok(b),
            Err(TokenToStringError::InsufficientBufferSpace(n)) => self
                .model
                .token_to_piece_bytes(token, n.unsigned_abs() as usize, false, None)
                .map_err(|e| EngineError::new("internal", format!("token: {e}"))),
            Err(e) => Err(EngineError::new("internal", format!("token: {e}"))),
        }
    }
}

fn message(role: &str, content: &str) -> Result<LlamaChatMessage, EngineError> {
    LlamaChatMessage::new(role.to_string(), content.to_string())
        .map_err(|e| EngineError::new("bad_request", format!("{role} message: {e}")))
}

/// Greedy at temperature 0; otherwise Qwen's recommended top-k 20, top-p
/// 0.8 in front of the temperature and a seeded draw.
fn sampler(temperature: f64) -> LlamaSampler {
    if temperature <= 0.0 {
        return LlamaSampler::greedy();
    }
    LlamaSampler::chain_simple([
        LlamaSampler::top_k(20),
        LlamaSampler::top_p(0.8, 1),
        LlamaSampler::temp(temperature as f32),
        LlamaSampler::dist(SEED),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_model_names_the_path() {
        let err = Engine::load(
            &LoadParams {
                model: "/nonexistent/model.gguf".into(),
                backend_preference: BackendPreference::Cpu,
                vad_model: None,
                context_length: None,
                lora: None,
                threads: None,
            },
            true,
        )
        .err()
        .expect("missing model");
        assert_eq!(err.code, "model_missing");
        assert!(err.message.contains("/nonexistent/model.gguf"), "{err}");
    }

    #[test]
    fn a_missing_adapter_names_its_path_before_the_model_is_touched() {
        // The model is checked first, so the adapter check needs a real
        // file there; a missing adapter over a missing model names the
        // model, which is the first thing the user has to fix.
        let err = Engine::load(
            &LoadParams {
                model: "/nonexistent/model.gguf".into(),
                backend_preference: BackendPreference::Cpu,
                vad_model: None,
                context_length: None,
                lora: Some("/nonexistent/adapter.gguf".into()),
                threads: None,
            },
            true,
        )
        .err()
        .expect("missing model");
        assert_eq!(err.code, "model_missing");
        assert!(err.message.contains("/nonexistent/model.gguf"), "{err}");
    }
}
