//! The Enhanced pass (FR-M3, FR-M7): the Polish result rewritten by a
//! language model. It is the macOS `PolishRewriteEngine` ported: the
//! deterministic pass runs first and stays the fallback, the rewrite
//! guard keeps short text away from the model, the output guard and the
//! no-op guard decide whether a rewrite may be inserted, a rejected
//! rewrite gets one repair pass per attempt, and anything that goes wrong
//! ends with the Polish text and a notice saying why.

pub mod guard;
pub mod prompt;
pub mod skip;

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::policy::EffectivePolicy;
use crate::polish;
use crate::polish::text::collapse_whitespace;
use crate::provider::{LlmProvider, ProviderError, RewriteRequest};

/// Why the inserted text is the Polish result rather than a rewrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeKind {
    /// The provider could not be reached, was not trusted, or ran out of
    /// time.
    ProviderUnavailable,
    /// Every rewrite the model returned was rejected by the guard.
    GuardRejected,
    /// The model answered, but the deterministic result was the better
    /// text (an empty, degenerate or no-op rewrite).
    FallbackUsed,
}

impl NoticeKind {
    /// The wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProviderUnavailable => "provider_unavailable",
            Self::GuardRejected => "guard_rejected",
            Self::FallbackUsed => "fallback_used",
        }
    }
}

/// What a completion event, a history item and `polish.test` report when
/// Enhanced did not insert a rewrite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notice {
    /// Which of the three cases this is.
    pub kind: NoticeKind,
    /// One line for a person.
    pub reason: String,
}

impl Notice {
    /// A notice of `kind` with `reason`.
    pub fn new(kind: NoticeKind, reason: impl Into<String>) -> Self {
        Self {
            kind,
            reason: reason.into(),
        }
    }
}

/// The result of an Enhanced pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewrite {
    /// The text to insert.
    pub text: String,
    /// Why it is the Polish result, when it is.
    pub notice: Option<Notice>,
    /// The model that answered, or the deterministic pass.
    pub model: String,
    /// How many times the provider was asked.
    pub attempts: u32,
}

/// The most repair passes a rewrite gets, whatever `[llm] max_retries`
/// says (the macOS `defaultMaxAttempts`).
pub const MAX_ATTEMPTS: u32 = 3;

fn normalize(text: &str) -> String {
    collapse_whitespace(text.trim())
}

/// True when the model changed nothing a transform would have changed, so
/// the deterministic result is the better text (the macOS
/// `PolishNoOpGuard`).
fn should_use_fallback(
    input: &str,
    output: &str,
    fallback: &str,
    policy: &EffectivePolicy,
) -> bool {
    if policy.transforms.is_empty() {
        return false;
    }
    let meaning = |text: &str| {
        normalize(text)
            .trim_end_matches(['.', '!', '?', '\u{2026}'])
            .to_lowercase()
    };
    meaning(output) == meaning(input)
        && normalize(fallback) != normalize(input)
        && polish::has_fixable_issue(input, &policy.transforms)
}

/// The Polish result for `input` under `policy`, the fallback of every
/// Enhanced pass.
pub fn deterministic(input: &str, policy: &EffectivePolicy) -> String {
    polish::rewrite(
        input,
        &policy.transforms,
        &policy.post_processors,
        policy.style,
    )
}

/// Runs the Enhanced pass: the Polish result, then a model rewrite when
/// one is worth asking for and the whole pass fits in `timeout`.
pub fn rewrite(
    input: &str,
    policy: &EffectivePolicy,
    provider: &dyn LlmProvider,
    vocabulary: &[String],
    timeout: Duration,
) -> Rewrite {
    let baseline = deterministic(input, policy);
    let deadline = Instant::now() + timeout;
    if skip::should_skip_model(input, &baseline, policy) {
        return Rewrite {
            text: baseline,
            notice: None,
            model: "deterministic".into(),
            attempts: 0,
        };
    }
    let base_prompt = prompt::system_prompt(policy, vocabulary);
    let user = prompt::wrap_transcript(input);
    let model = provider.model();
    let mut last_rejection: Option<String> = None;
    let mut last_output = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return fallback(
                baseline,
                NoticeKind::ProviderUnavailable,
                "the rewrite did not finish inside [llm] timeout_ms",
                attempt - 1,
            );
        }
        let system = if attempt == 1 {
            base_prompt.clone()
        } else {
            prompt::repair_prompt(
                &base_prompt,
                last_rejection
                    .as_deref()
                    .unwrap_or("invalid rewrite output"),
                &last_output,
                &baseline,
            )
        };
        let request = RewriteRequest {
            system,
            user: user.clone(),
        };
        let answer = match provider.rewrite(&request, remaining) {
            Ok(answer) => answer,
            Err(e) => {
                let kind = match e {
                    ProviderError::Failed(_) | ProviderError::PromptTooLong(_) => {
                        NoticeKind::FallbackUsed
                    }
                    _ => NoticeKind::ProviderUnavailable,
                };
                return fallback(baseline, kind, e.to_string(), attempt);
            }
        };
        let sanitized = guard::strip_thinking(&answer, false);
        let extracted = prompt::extract_rewrite(&sanitized).unwrap_or_else(|| sanitized.clone());
        last_output = sanitized;
        if extracted.trim().is_empty() {
            last_rejection = Some("empty output".into());
            continue;
        }
        let final_text = polish::apply_post_processors(&policy.post_processors, &extracted);
        if let Some(rejection) = guard::rejection_reason(input, &final_text) {
            if prefers_baseline_after_rejection(input, &final_text, &baseline, &rejection) {
                return Rewrite {
                    text: baseline,
                    notice: None,
                    model: "deterministic".into(),
                    attempts: attempt,
                };
            }
            last_rejection = Some(rejection);
            continue;
        }
        if should_use_fallback(input, &final_text, &baseline, policy) {
            return Rewrite {
                text: baseline,
                notice: None,
                model: "deterministic".into(),
                attempts: attempt,
            };
        }
        return Rewrite {
            text: final_text,
            notice: None,
            model,
            attempts: attempt,
        };
    }
    let reason = last_rejection.unwrap_or_else(|| "invalid rewrite output".into());
    fallback(baseline, NoticeKind::GuardRejected, reason, MAX_ATTEMPTS)
}

fn fallback(
    baseline: String,
    kind: NoticeKind,
    reason: impl Into<String>,
    attempts: u32,
) -> Rewrite {
    Rewrite {
        text: baseline,
        notice: Some(Notice::new(kind, reason)),
        model: "deterministic".into(),
        attempts,
    }
}

/// True when a rejected rewrite is one the deterministic result recovers
/// cleanly, so the Polish text is inserted without a notice.
fn prefers_baseline_after_rejection(
    input: &str,
    model_output: &str,
    baseline: &str,
    rejection: &str,
) -> bool {
    if normalize(baseline) == normalize(input) || guard::rejection_reason(input, baseline).is_some()
    {
        return false;
    }
    if normalize(model_output) == normalize(input) {
        return true;
    }
    [
        "output dropped too much dictated content",
        "output lost question punctuation",
        "output included transcript markers",
    ]
    .contains(&rejection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{Backend, EffectivePolicy};
    use crate::polish::{PostProcessor, Preset, Style, Transform};
    use crate::provider::LocalProvider;
    use crate::provider::mock::{MockProvider, Mode};
    use std::collections::BTreeSet;

    fn policy() -> EffectivePolicy {
        EffectivePolicy {
            preset: Preset::Generic,
            style: Style::AsDictated,
            transforms: Transform::ALL.into_iter().collect::<BTreeSet<_>>(),
            custom_rules: String::new(),
            backend: Backend::Mock {
                model: "mock-echo".into(),
            },
            post_processors: vec![PostProcessor::NormalizeSpokenLists],
            target_app_id: None,
            app_class: None,
        }
    }

    fn fixture_provider(body: &str, input: &str) -> (tempfile::TempDir, MockProvider) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path()
                .join(format!("{}.txt", MockProvider::fixture_name(input))),
            body,
        )
        .unwrap();
        let provider = MockProvider::new(Mode::Fixture(dir.path().to_path_buf()));
        (dir, provider)
    }

    const LONG: &str =
        "um so we should probably rewrite the onboarding flow before the release lands";

    #[test]
    fn echo_inserts_the_polish_text_and_never_notices() {
        let provider = MockProvider::new(Mode::Echo);
        let out = rewrite(LONG, &policy(), &provider, &[], Duration::from_secs(5));
        assert_eq!(out.notice, None);
        assert_eq!(out.text, deterministic(LONG, &policy()));
        assert_eq!(out.model, "deterministic");
    }

    #[test]
    fn a_thinking_block_is_stripped_before_the_guard_sees_the_rewrite() {
        let (_dir, provider) = fixture_provider(
            "<think>weighing it up</think>So we should rewrite the onboarding flow before the release lands.",
            LONG,
        );
        let out = rewrite(LONG, &policy(), &provider, &[], Duration::from_secs(5));
        assert_eq!(out.notice, None);
        assert_eq!(
            out.text,
            "So we should rewrite the onboarding flow before the release lands."
        );
        assert_eq!(out.model, format!("mock-fixture:{}", _dir.path().display()));
    }

    #[test]
    fn a_rewrite_that_drops_a_path_is_rejected_and_falls_back() {
        let input =
            "the quarterly report lives at docs/reports/q3.md and marketing needs it by tomorrow";
        let (_dir, provider) = fixture_provider(
            "The quarterly report lives in the repository and marketing needs it by tomorrow.",
            input,
        );
        let out = rewrite(input, &policy(), &provider, &[], Duration::from_secs(5));
        assert_eq!(
            out.notice.map(|n| n.kind),
            Some(NoticeKind::GuardRejected),
            "a rewrite that drops the path must not be inserted"
        );
    }

    #[test]
    fn a_provider_timeout_falls_back_with_provider_unavailable() {
        let (_dir, provider) = fixture_provider("__TIMEOUT__", LONG);
        let out = rewrite(LONG, &policy(), &provider, &[], Duration::from_secs(5));
        let notice = out.notice.expect("a timeout is noticed");
        assert_eq!(notice.kind, NoticeKind::ProviderUnavailable);
        assert_eq!(out.text, deterministic(LONG, &policy()));
    }

    #[test]
    fn an_unavailable_provider_inserts_the_polish_text_with_the_notice() {
        let detached = LocalProvider::detached();
        let out = rewrite(LONG, &policy(), &detached, &[], Duration::from_secs(5));
        assert_eq!(
            out.notice.map(|n| n.kind),
            Some(NoticeKind::ProviderUnavailable)
        );
        assert!(!detached.probe().available);
        assert!(detached.probe().detail.contains("not attached"));
    }

    #[test]
    fn a_zero_budget_never_asks_the_provider() {
        let provider = MockProvider::new(Mode::Echo);
        let out = rewrite(LONG, &policy(), &provider, &[], Duration::ZERO);
        assert_eq!(out.attempts, 0);
        assert_eq!(
            out.notice.map(|n| n.kind),
            Some(NoticeKind::ProviderUnavailable)
        );
    }
}
