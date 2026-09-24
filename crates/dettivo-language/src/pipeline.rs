//! The mode step (FR-M1 to FR-M3): what a dictation's text goes through
//! between the engine and the insertion. Every mode starts at the raw
//! layer; `deterministic_polish` adds the Polish pass under the resolved
//! policy; `enhanced` asks the provider layer for a rewrite and keeps the
//! Polish result when it cannot have one. The session runs this on a
//! finished take and `polish.test` runs it on a sample, so both answer
//! with the same text, the same notice and the same policy hash.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dettivo_core::config::polish_schema::Llm;

use crate::enhanced::{self, Notice};
use crate::policy::{Backend, Context, EffectivePolicy, RulesConfig, resolve};
use crate::provider::{self, LlmProvider, LocalEngine};
use crate::raw;

/// Which layers a dictation runs through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The raw layer only.
    Raw,
    /// The raw layer and the deterministic Polish pass.
    DeterministicPolish,
    /// The Polish pass, then a model rewrite when one can be had.
    Enhanced,
}

impl Mode {
    /// The identifier on the wire; `polish` is the contract's alias for
    /// `deterministic_polish`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::DeterministicPolish => "deterministic_polish",
            Self::Enhanced => "enhanced",
        }
    }

    /// The mode an identifier names, or `None`.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "raw" => Some(Self::Raw),
            "polish" | "deterministic_polish" => Some(Self::DeterministicPolish),
            "enhanced" => Some(Self::Enhanced),
            _ => None,
        }
    }

    /// The valid spellings, for an error message.
    pub fn names() -> &'static str {
        "raw, polish, deterministic_polish, enhanced"
    }
}

/// Everything the mode step reads: the raw layer's settings, the words
/// the prompt is primed with, the `[polish]` rules and the `[llm]`
/// section.
#[derive(Clone, Copy)]
pub struct Settings<'a> {
    /// The replacement pairs the raw layer applies.
    pub replacements: &'a [(String, String)],
    /// Whether spoken punctuation becomes marks.
    pub spoken_punctuation: bool,
    /// Whether file names, paths, URLs, addresses, versions and dotted
    /// identifiers are kept byte-identical through the raw layer.
    pub protect_tokens: bool,
    /// The vocabulary the Enhanced prompt is primed with.
    pub vocabulary: &'a [String],
    /// The `[polish]` rules in force.
    pub rules: &'a RulesConfig,
    /// The `[llm]` section in force.
    pub llm: &'a Llm,
    /// The daemon's local engine, when it attached one (ADR 0026).
    pub local: Option<&'a Arc<dyn LocalEngine>>,
}

impl std::fmt::Debug for Settings<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("replacements", &self.replacements.len())
            .field("spoken_punctuation", &self.spoken_punctuation)
            .field("protect_tokens", &self.protect_tokens)
            .field("vocabulary", &self.vocabulary.len())
            .field("llm", &self.llm.provider)
            .field("local", &self.local.is_some())
            .finish()
    }
}

/// What the mode step produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The text after the raw layer.
    pub raw: String,
    /// The text after the deterministic Polish pass; the raw text under
    /// the `raw` mode.
    pub polished: String,
    /// The model's rewrite, when one was inserted.
    pub enhanced: Option<String>,
    /// What is inserted.
    pub text: String,
    /// Why the deterministic result was used instead of a rewrite.
    pub notice: Option<Notice>,
    /// The policy this ran under.
    pub policy: EffectivePolicy,
    /// Its hash, for the completion event, the history item and the logs.
    pub policy_hash: String,
    /// The model that answered, or `deterministic`.
    pub model: String,
}

/// The backend a provider stands for, as the policy freezes it.
pub fn backend_of(provider: Option<&dyn LlmProvider>, llm: &Llm) -> Backend {
    let Some(provider) = provider else {
        return Backend::Disabled;
    };
    let model = provider.model();
    match provider.name() {
        "ollama" => Backend::Ollama {
            endpoint: provider::canonicalize(&llm.ollama_url)
                .unwrap_or_else(|_| llm.ollama_url.clone()),
            model,
        },
        "openai_compatible" => Backend::OpenaiCompatible {
            endpoint: provider::canonicalize(&llm.endpoint_url)
                .unwrap_or_else(|_| llm.endpoint_url.clone()),
            model,
        },
        "local" => Backend::Local { model },
        _ => Backend::Mock { model },
    }
}

/// The provider asked for the model the effective policy names (a custom
/// preset's override), so the request carries what the policy hash
/// describes; a provider bound to one model keeps it, and the policy is
/// set back to that model so the two never disagree.
fn bind_model(
    provider: Option<Box<dyn LlmProvider>>,
    mut policy: EffectivePolicy,
    llm: &Llm,
) -> (Option<Box<dyn LlmProvider>>, EffectivePolicy) {
    let Some(provider) = provider else {
        return (None, policy);
    };
    let wanted = policy.backend.model_name();
    if wanted == provider.model() {
        return (Some(provider), policy);
    }
    match provider.with_model(&wanted) {
        Some(bound) => (Some(bound), policy),
        None => {
            tracing::warn!(
                provider = provider.name(),
                model = %provider.model(),
                wanted = %wanted,
                "the provider is bound to its model; the preset's model override does not apply"
            );
            policy.backend = backend_of(Some(provider.as_ref()), llm);
            (Some(provider), policy)
        }
    }
}

/// Runs `engine_text` through the layers `mode` asks for.
pub fn run(
    mode: Mode,
    engine_text: &str,
    app_id: Option<&str>,
    settings: &Settings<'_>,
) -> Outcome {
    let raw_text = raw::apply(
        engine_text,
        settings.replacements,
        settings.spoken_punctuation,
        settings.protect_tokens,
    );
    // The budget starts before the provider is looked for: probing and
    // credential lookup spend the same `[llm] timeout_ms` as the rewrite.
    let deadline = Instant::now() + Duration::from_millis(settings.llm.timeout_ms.max(1));
    let provider = (mode == Mode::Enhanced)
        .then(|| provider::resolve(settings.llm, settings.local))
        .flatten();
    let backend = backend_of(provider.as_deref(), settings.llm);
    let policy = resolve(
        settings.rules,
        &backend,
        &Context {
            app_id,
            app_class: None,
            raw_transcript: Some(&raw_text),
        },
    );
    let (provider, policy) = bind_model(provider, policy, settings.llm);
    let policy_hash = policy.config_hash();
    if mode == Mode::Raw {
        return Outcome {
            polished: raw_text.clone(),
            enhanced: None,
            text: raw_text.clone(),
            raw: raw_text,
            notice: None,
            policy,
            policy_hash,
            model: "raw".into(),
        };
    }
    let polished = enhanced::deterministic(&raw_text, &policy);
    if mode == Mode::DeterministicPolish {
        return Outcome {
            raw: raw_text,
            text: polished.clone(),
            polished,
            enhanced: None,
            notice: None,
            policy,
            policy_hash,
            model: "deterministic".into(),
        };
    }
    let Some(provider) = provider else {
        return Outcome {
            raw: raw_text,
            text: polished.clone(),
            polished,
            enhanced: None,
            notice: Some(Notice::new(
                enhanced::NoticeKind::ProviderUnavailable,
                "no language model provider is available; see [llm] in config.toml",
            )),
            policy,
            policy_hash,
            model: "deterministic".into(),
        };
    };
    let rewrite = enhanced::rewrite(
        &raw_text,
        &policy,
        provider.as_ref(),
        settings.vocabulary,
        deadline.saturating_duration_since(Instant::now()),
    );
    let used_model = rewrite.notice.is_none() && rewrite.text != polished;
    Outcome {
        raw: raw_text,
        enhanced: used_model.then(|| rewrite.text.clone()),
        text: rewrite.text,
        polished,
        notice: rewrite.notice,
        policy,
        policy_hash,
        model: rewrite.model,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enhanced::NoticeKind;
    use dettivo_core::config::polish_schema::Polish;

    fn settings<'a>(rules: &'a RulesConfig, llm: &'a Llm) -> Settings<'a> {
        Settings {
            replacements: &[],
            spoken_punctuation: true,
            protect_tokens: true,
            vocabulary: &[],
            rules,
            llm,
            local: None,
        }
    }

    /// dictation/F15: the rewrite budget starts before the provider is
    /// probed; a probe that eats it leaves no time for the model, and the
    /// pass falls back instead of running past `[llm] timeout_ms`.
    #[test]
    fn the_budget_covers_provider_resolution() {
        use crate::provider::local::LocalEngine;
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct SlowProbe(AtomicUsize);
        impl LocalEngine for SlowProbe {
            fn model(&self) -> String {
                "fake".into()
            }
            fn probe(&self) -> crate::provider::Availability {
                std::thread::sleep(Duration::from_millis(60));
                crate::provider::Availability::up("fake")
            }
            fn generate(
                &self,
                _: &crate::provider::RewriteRequest,
                _: Duration,
            ) -> Result<crate::provider::Answer, crate::provider::ProviderError> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(crate::provider::Answer::unknown(
                    "Rewritten by the model.".into(),
                ))
            }
        }
        let rules = RulesConfig::from_config(&Polish::default());
        let llm = Llm {
            provider: dettivo_core::config::llm_schema::LlmProviderChoice::Auto,
            timeout_ms: 20,
            ..Llm::default()
        };
        let engine: Arc<dyn LocalEngine> = Arc::new(SlowProbe(AtomicUsize::new(0)));
        let mut settings = settings(&rules, &llm);
        settings.local = Some(&engine);
        let out = run(
            Mode::Enhanced,
            "so we should um ship the thing tomorrow and tell the team",
            None,
            &settings,
        );
        assert_eq!(
            out.notice.as_ref().map(|n| n.kind),
            Some(NoticeKind::ProviderUnavailable),
            "{out:?}"
        );
        assert_eq!(out.text, out.polished, "the deterministic result stands");
    }

    #[test]
    fn every_mode_starts_at_the_raw_layer() {
        let rules = RulesConfig::from_config(&Polish::default());
        let llm = Llm::default();
        let settings = settings(&rules, &llm);
        let out = run(Mode::Raw, "um hello comma world period", None, &settings);
        assert_eq!(out.raw, "Um hello, world.");
        assert_eq!(out.text, out.raw);
        assert_eq!(out.model, "raw");
        assert_eq!(out.notice, None);
        assert_eq!(out.policy_hash.len(), 8);
        let polished = run(
            Mode::DeterministicPolish,
            "um hello comma world period",
            None,
            &settings,
        );
        assert_eq!(polished.raw, "Um hello, world.");
        assert_eq!(polished.text, "Hello, world.");
        assert_eq!(polished.model, "deterministic");
        assert_eq!(polished.enhanced, None);
    }

    /// R1: each protected token class comes back byte-identical through
    /// `raw` and `deterministic_polish`; a change names the token.
    #[test]
    fn protected_tokens_survive_raw_and_deterministic_polish() {
        let rules = RulesConfig::from_config(&Polish::default());
        let llm = Llm::default();
        let settings = settings(&rules, &llm);
        let tokens = [
            "index.ts",
            "src/app/index.ts",
            "https://example.com/docs",
            "gordon@mickel.tech",
            "v1.2.3",
            "foo.bar()",
            "std::io::Error",
            "`cargo build`",
        ];
        for token in tokens {
            let input = format!("um open {token} period then run it");
            for mode in [Mode::Raw, Mode::DeterministicPolish] {
                let out = run(mode, &input, None, &settings);
                assert!(
                    out.text.contains(token),
                    "{token} changed bytes under {}: {}",
                    mode.as_str(),
                    out.text
                );
            }
            let polished = run(Mode::DeterministicPolish, &input, None, &settings);
            assert_eq!(
                polished.text,
                format!("Open {token}. Then run it."),
                "{token}"
            );
        }
    }

    /// A custom preset's model override reaches the request itself, and a
    /// provider that cannot switch models keeps the policy on the model
    /// that answers.
    #[test]
    fn a_preset_model_override_is_the_model_the_request_carries() {
        use crate::provider::testing::MockServer;
        use dettivo_core::config::polish_schema::{AppEntry, LlmProviderChoice, PresetEntry};
        let server = MockServer::start(vec![(
            "/v1/chat/completions".into(),
            200,
            r#"{"choices":[{"message":{"role":"assistant","content":"We should rewrite the onboarding flow before the release lands."}}]}"#.into(),
        )]);
        let mut polish = Polish::default();
        polish.presets.insert(
            "fast".into(),
            PresetEntry {
                base: crate::polish::Preset::Generic,
                style: crate::polish::Style::AsDictated,
                transforms: None,
                custom_rules: String::new(),
                post_processors: None,
                model: Some("special".into()),
            },
        );
        polish.apps.insert(
            "org.example.App".into(),
            AppEntry {
                preset: crate::polish::Preset::Generic,
                style: None,
                post_processors: None,
                custom_preset: Some("fast".into()),
            },
        );
        let rules = RulesConfig::from_config(&polish);
        let llm = Llm {
            provider: LlmProviderChoice::OpenaiCompatible,
            endpoint_url: server.url(),
            endpoint_model: "base".into(),
            ..Llm::default()
        };
        let endpoint_settings = settings(&rules, &llm);
        let input = "um so we should rewrite the onboarding flow before the release lands";
        let out = run(
            Mode::Enhanced,
            input,
            Some("org.example.App"),
            &endpoint_settings,
        );
        match &out.policy.backend {
            Backend::OpenaiCompatible { model, .. } => assert_eq!(model, "special"),
            other => panic!("{other:?}"),
        }
        let (_, body) = server.requests().pop().expect("the endpoint was asked");
        let body: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["model"], "special", "the request carries the override");

        // The local engine is bound to its catalogue model: the override
        // does not apply and the policy says which model answers.
        struct Fixed;
        impl crate::provider::LocalEngine for Fixed {
            fn model(&self) -> String {
                "qwen3-4b".into()
            }
            fn probe(&self) -> crate::provider::Availability {
                crate::provider::Availability::up("attached")
            }
            fn generate(
                &self,
                _request: &crate::provider::RewriteRequest,
                _timeout: Duration,
            ) -> Result<crate::provider::Answer, crate::provider::ProviderError> {
                Ok(crate::provider::Answer::unknown(
                    "We should rewrite the onboarding flow before the release lands.".into(),
                ))
            }
        }
        let local: std::sync::Arc<dyn crate::provider::LocalEngine> = std::sync::Arc::new(Fixed);
        let llm = Llm {
            provider: LlmProviderChoice::Local,
            ..Llm::default()
        };
        let local_settings = Settings {
            local: Some(&local),
            ..settings(&rules, &llm)
        };
        let out = run(
            Mode::Enhanced,
            input,
            Some("org.example.App"),
            &local_settings,
        );
        assert_eq!(
            out.policy.backend,
            Backend::Local {
                model: "qwen3-4b".into()
            },
            "the policy names the model that answered"
        );
    }
}

#[cfg(test)]
#[path = "pipeline_extra_tests.rs"]
mod extra_tests;
