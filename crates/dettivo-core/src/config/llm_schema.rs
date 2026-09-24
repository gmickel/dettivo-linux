//! `[llm]`: the language model provider layer behind the Enhanced mode
//! (ADR 0023, ADR 0026) — which provider, the catalogue model the local
//! engine loads, where Ollama and an endpoint answer, the remote
//! endpoints the user confirmed, and the time budget the whole pass must
//! finish in.

use serde::{Deserialize, Serialize};

/// Which language model provider `[llm] provider` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmProviderChoice {
    /// The first available of `local`, `ollama`, `openai_compatible`.
    Auto,
    /// The llama.cpp engine with a catalogue model (`dettivo-engine-llm`).
    Local,
    /// Ollama on localhost.
    Ollama,
    /// An OpenAI-compatible chat endpoint.
    OpenaiCompatible,
}

impl LlmProviderChoice {
    /// The wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Local => "local",
            Self::Ollama => "ollama",
            Self::OpenaiCompatible => "openai_compatible",
        }
    }
}

/// `[llm]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Llm {
    /// `auto`, `local`, `ollama` or `openai_compatible`.
    pub provider: LlmProviderChoice,
    /// The catalogue model the local engine loads for the Enhanced
    /// rewrite (`dettivo llm download --model <id>`).
    pub model: String,
    /// The catalogue model meeting analysis loads; empty means `model`.
    pub analysis_model: String,
    /// A sideloaded polish fine-tune the Enhanced rewrite runs on instead
    /// of `model` (ADR 0032): the name of a manifest under
    /// `experiments_dir` (`current` reads `current.json`); empty keeps
    /// the catalogue model.
    pub polish_experiment: String,
    /// Where sideloaded fine-tunes and their manifests live; empty means
    /// `<models>/polish-experiments`.
    pub experiments_dir: String,
    /// Where Ollama answers.
    pub ollama_url: String,
    /// The Ollama model.
    pub ollama_model: String,
    /// The OpenAI-compatible base URL (its `/v1/chat/completions` is used).
    pub endpoint_url: String,
    /// The model the endpoint is asked for.
    pub endpoint_model: String,
    /// A file holding the endpoint's API key (mode 0600); the Secret
    /// Service item (service `dettivo`, key `llm-api-key`) is read first.
    pub api_key_file: String,
    /// Non-localhost endpoints the user confirmed (`dettivo llm trust`).
    pub trusted_endpoints: Vec<String>,
    /// The whole Enhanced pass must finish within this; then the Polish
    /// text is inserted with a notice.
    pub timeout_ms: u64,
    /// Repair attempts after a rejected rewrite, within the same budget.
    pub max_retries: u64,
}

impl Llm {
    /// The model meeting analysis loads: `analysis_model`, or `model`
    /// when it is empty.
    pub fn analysis_model(&self) -> &str {
        if self.analysis_model.trim().is_empty() {
            &self.model
        } else {
            &self.analysis_model
        }
    }

    /// The sideloaded fine-tune in force (`polish_experiment`), or `None`
    /// when Enhanced runs on the catalogue model.
    pub fn polish_experiment(&self) -> Option<&str> {
        let name = self.polish_experiment.trim();
        (!name.is_empty()).then_some(name)
    }

    /// Where sideloaded fine-tunes live: `experiments_dir`, or
    /// `<models_dir>/polish-experiments` when it is empty.
    pub fn experiments_dir(&self, models_dir: &std::path::Path) -> std::path::PathBuf {
        if self.experiments_dir.trim().is_empty() {
            models_dir.join("polish-experiments")
        } else {
            std::path::PathBuf::from(self.experiments_dir.trim())
        }
    }
}

impl Default for Llm {
    fn default() -> Self {
        Self {
            provider: LlmProviderChoice::Auto,
            model: "qwen3-4b-instruct-2507".into(),
            analysis_model: String::new(),
            polish_experiment: String::new(),
            experiments_dir: String::new(),
            ollama_url: "http://127.0.0.1:11434".into(),
            ollama_model: "qwen3:4b-instruct".into(),
            endpoint_url: String::new(),
            endpoint_model: String::new(),
            api_key_file: String::new(),
            trusted_endpoints: Vec::new(),
            timeout_ms: 8000,
            max_retries: 2,
        }
    }
}
