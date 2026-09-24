//! The frozen policy (FR-M5): the language model a session runs against,
//! the app class a caller may name, and the effective policy itself with
//! the deterministic hash the logs, the completion payload and the
//! history item carry.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::polish::{PostProcessor, Preset, Style, Transform};

/// The language model behind the Enhanced pass, as the policy freezes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Backend {
    /// The llama.cpp engine (arrives with its spec).
    Local {
        /// The model id.
        model: String,
    },
    /// Ollama at `endpoint`.
    Ollama {
        /// The canonical endpoint.
        endpoint: String,
        /// The model.
        model: String,
    },
    /// An OpenAI-compatible endpoint.
    OpenaiCompatible {
        /// The canonical endpoint.
        endpoint: String,
        /// The model.
        model: String,
    },
    /// The QA mock (`DETTIVO_MOCK_LLM`).
    Mock {
        /// `mock-echo` or the fixture's model name.
        model: String,
    },
    /// No provider: Enhanced falls back to Polish.
    Disabled,
}

impl Backend {
    /// The model name a result reports (`deterministic` when disabled).
    pub fn model_name(&self) -> String {
        match self {
            Self::Local { model }
            | Self::Ollama { model, .. }
            | Self::OpenaiCompatible { model, .. }
            | Self::Mock { model } => model.clone(),
            Self::Disabled => "deterministic".into(),
        }
    }

    /// The same backend with another model.
    pub fn with_model(&self, model: &str) -> Self {
        match self {
            Self::Local { .. } => Self::Local {
                model: model.into(),
            },
            Self::Ollama { endpoint, .. } => Self::Ollama {
                endpoint: endpoint.clone(),
                model: model.into(),
            },
            Self::OpenaiCompatible { endpoint, .. } => Self::OpenaiCompatible {
                endpoint: endpoint.clone(),
                model: model.into(),
            },
            Self::Mock { .. } => Self::Mock {
                model: model.into(),
            },
            Self::Disabled => Self::Disabled,
        }
    }

    /// The description the hash and the logs use.
    pub fn describe(&self) -> String {
        match self {
            Self::Local { model } => format!("local:{model}"),
            Self::Ollama { endpoint, model } => format!("ollama:{endpoint}:{model}"),
            Self::OpenaiCompatible { endpoint, model } => {
                format!("openai_compatible:{endpoint}:{model}")
            }
            Self::Mock { model } => format!("mock:{model}"),
            Self::Disabled => "disabled".into(),
        }
    }
}

/// The app class a context adapter reports (none exists on Linux yet; the
/// resolution honours it when a caller names one).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppClass {
    /// An editor or IDE.
    Ide,
    /// A mail client.
    Mail,
    /// A browser.
    Browser,
    /// Anything else.
    Generic,
}

impl AppClass {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ide => "ide",
            Self::Mail => "mail",
            Self::Browser => "browser",
            Self::Generic => "generic",
        }
    }
}

/// The policy a session runs under, frozen at its start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectivePolicy {
    /// The preset.
    pub preset: Preset,
    /// The style.
    pub style: Style,
    /// The transforms in force.
    pub transforms: BTreeSet<Transform>,
    /// The custom rules the prompt carries.
    pub custom_rules: String,
    /// The language model.
    pub backend: Backend,
    /// The post-processors, in run order.
    pub post_processors: Vec<PostProcessor>,
    /// The app id the policy was resolved for.
    pub target_app_id: Option<String>,
    /// The app class, when a caller named one.
    pub app_class: Option<AppClass>,
}

impl EffectivePolicy {
    /// The first eight hex characters of the SHA-256 of the sorted
    /// description, the same recipe as macOS `configHash`.
    pub fn config_hash(&self) -> String {
        let transforms = self
            .transforms
            .iter()
            .map(|t| t.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let mut processors: Vec<&str> = self.post_processors.iter().map(|p| p.as_str()).collect();
        processors.sort_unstable();
        let description = format!(
            "{}|{}|{}|{}|{}|{}|{}|off|||{}",
            self.preset.as_str(),
            self.style.as_str(),
            transforms,
            self.custom_rules,
            self.backend.describe(),
            processors.join(","),
            self.target_app_id.as_deref().unwrap_or(""),
            self.app_class.map(AppClass::as_str).unwrap_or("")
        );
        let digest = Sha256::digest(description.as_bytes());
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()[..8]
            .to_string()
    }
}
