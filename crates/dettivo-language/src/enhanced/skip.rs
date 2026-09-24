//! The rewrite guard (FR-M3), ported from the macOS
//! `PolishRewriteEngine.shouldReturnBaselineDirectly`: short text the
//! deterministic pass already handles well never reaches the model, so a
//! question, a plain command, an already formatted code command and a
//! high-confidence spelling fix are inserted from the Polish result
//! straight away.

use crate::policy::EffectivePolicy;
use crate::polish::text::{collapse_whitespace, is_match, re};
use crate::polish::{Preset, Transform};

use super::guard::rejection_reason;

/// True when the Polish result may be inserted without asking a model.
pub fn should_skip_model(input: &str, baseline: &str, policy: &EffectivePolicy) -> bool {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.contains('\n') {
        return false;
    }
    let words = trimmed.split_whitespace().count();
    if words > 18 {
        return false;
    }
    if is_short_sensitive_prose(trimmed) && rejection_reason(input, baseline).is_none() {
        return true;
    }
    if baseline == input {
        return false;
    }
    if is_short_generic_deterministic_fix(input, baseline, policy, words) {
        return true;
    }
    if policy.preset == Preset::Code && is_already_formatted_code_command(trimmed) {
        return true;
    }
    if policy.preset != Preset::Code && is_command_like(trimmed) {
        return true;
    }
    looks_like_question_lead(trimmed) && words <= 14
}

fn normalize(text: &str) -> String {
    collapse_whitespace(text.trim())
}

fn is_short_generic_deterministic_fix(
    input: &str,
    baseline: &str,
    policy: &EffectivePolicy,
    words: usize,
) -> bool {
    policy.preset == Preset::Generic
        && words <= 12
        && normalize(baseline) != normalize(input)
        && has_high_confidence_local_fix_cue(input, policy)
        && is_plain_generic_statement(input)
        && rejection_reason(input, baseline).is_none()
}

fn has_high_confidence_local_fix_cue(text: &str, policy: &EffectivePolicy) -> bool {
    policy.transforms.contains(&Transform::FixGrammar)
        && is_match(
            re!(r"(?i)\b(?:ithink|thats|ths|teh|dont|cant|wont)\b"),
            text,
        )
}

fn is_plain_generic_statement(text: &str) -> bool {
    !looks_like_question_lead(text)
        && !is_command_like(text)
        && !contains_spoken_formatting_markers(text)
        && !contains_fragmented_filename(text)
        && !contains_flattened_path_token(text)
        && is_plain_code_command(text)
        && !is_match(
            re!(
                r"(?i)\b[\w./-]+\.(?:ts|tsx|js|jsx|swift|py|md|json|yaml|yml|go|rs|java|kt|sh|zsh|plist)\b"
            ),
            text,
        )
}

fn is_short_sensitive_prose(text: &str) -> bool {
    if looks_like_question_lead(text) && text.split_whitespace().count() <= 14 {
        return true;
    }
    let lowered = text.trim().to_lowercase();
    ["i mean ", "no, ", "no ", "actually ", "sorry "]
        .iter()
        .any(|cue| lowered.starts_with(cue))
}

fn is_plain_code_command(text: &str) -> bool {
    let lowered = text.to_lowercase();
    !["```", "{", "}", ";", "=>", "function", "class "]
        .iter()
        .any(|m| lowered.contains(m))
}

fn is_already_formatted_code_command(text: &str) -> bool {
    is_plain_code_command(text)
        && is_command_like(text)
        && !contains_spoken_formatting_markers(text)
        && !contains_fragmented_filename(text)
        && !contains_flattened_path_token(text)
}

fn is_command_like(text: &str) -> bool {
    let lowered = text.to_lowercase();
    let has_action_prefix = [
        "read ", "open ", "check ", "review ", "show ", "run ", "find ", "inspect ",
    ]
    .iter()
    .any(|p| lowered.starts_with(*p));
    has_action_prefix
        && is_match(
            re!(r"\b[\w./-]+\.(?:ts|tsx|js|jsx|swift|py|md|json|yaml|yml|go|rs|java|kt|toml)\b"),
            &lowered,
        )
}

fn contains_spoken_formatting_markers(text: &str) -> bool {
    let lowered = text.to_lowercase();
    [
        " dot ",
        " slash ",
        " dash dash ",
        " colon ",
        " point ",
        " at sign ",
        " quote ",
        " unquote",
    ]
    .iter()
    .any(|m| lowered.contains(*m))
}

fn contains_fragmented_filename(text: &str) -> bool {
    is_match(
        re!(
            r"\b[A-Za-z0-9]+(?:\s+[A-Za-z0-9]+)+\.(?:ts|tsx|js|jsx|swift|py|md|json|yaml|yml|go|rs|java|kt)\b"
        ),
        text,
    )
}

fn contains_flattened_path_token(text: &str) -> bool {
    is_match(
        re!(
            r"\b[A-Za-z0-9]+(?:-[A-Za-z0-9]+){3,}\.(?:ts|tsx|js|jsx|swift|py|md|json|yaml|yml|go|rs|java|kt)\b"
        ),
        text,
    )
}

/// The macOS engine's own short question check: the leading greetings
/// stripped, then an English question starter.
fn looks_like_question_lead(text: &str) -> bool {
    let mut normalized = text.trim().to_lowercase();
    let mut stripped = true;
    while stripped {
        stripped = false;
        for filler in ["hey ", "hi ", "hello ", "so ", "well ", "ok ", "okay "] {
            if let Some(rest) = normalized.strip_prefix(filler) {
                normalized = rest.trim_start().to_string();
                stripped = true;
                break;
            }
        }
    }
    [
        "can ", "could ", "would ", "should ", "do ", "does ", "did ", "is ", "are ", "am ",
        "was ", "were ", "will ", "have ", "has ", "had ", "what ", "when ", "where ", "why ",
        "who ", "how ",
    ]
    .iter()
    .any(|s| normalized.starts_with(*s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::Backend;
    use crate::polish::{Style, Transform};
    use std::collections::BTreeSet;

    fn policy(preset: Preset) -> EffectivePolicy {
        EffectivePolicy {
            preset,
            style: Style::AsDictated,
            transforms: Transform::ALL.into_iter().collect::<BTreeSet<_>>(),
            custom_rules: String::new(),
            backend: Backend::Mock {
                model: "mock-echo".into(),
            },
            post_processors: Vec::new(),
            target_app_id: None,
            app_class: None,
        }
    }

    #[test]
    fn short_questions_and_plain_commands_never_reach_the_model() {
        assert!(should_skip_model(
            "did you push the branch yet?",
            "Did you push the branch yet?",
            &policy(Preset::Generic)
        ));
        assert!(should_skip_model(
            "read src/app/index.ts",
            "Read src/app/index.ts.",
            &policy(Preset::Generic)
        ));
        assert!(should_skip_model(
            "open crates/dettivod/src/main.rs",
            "Open crates/dettivod/src/main.rs",
            &policy(Preset::Code)
        ));
    }

    #[test]
    fn a_high_confidence_spelling_fix_is_taken_from_the_deterministic_pass() {
        assert!(should_skip_model(
            "well ithink thats fine",
            "Well I think that's fine.",
            &policy(Preset::Generic)
        ));
        // The same sentence dictated with spoken markers still wants the
        // model: the deterministic pass cannot reassemble the path.
        assert!(!should_skip_model(
            "open the source slash app dot ts file and then rename every helper we touched today",
            "Open the source/app.ts file and then rename every helper we touched today.",
            &policy(Preset::Generic)
        ));
    }

    #[test]
    fn long_or_multiline_prose_always_reaches_the_model() {
        let long = "we should probably rewrite the whole onboarding flow because the current one confuses every single new user we have";
        assert!(!should_skip_model(long, long, &policy(Preset::Generic)));
        assert!(!should_skip_model(
            "one\ntwo",
            "One\ntwo",
            &policy(Preset::Generic)
        ));
        assert!(!should_skip_model("", "", &policy(Preset::Generic)));
    }
}
