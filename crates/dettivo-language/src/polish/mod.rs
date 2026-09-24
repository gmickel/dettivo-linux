//! The deterministic Polish pass (FR-M2): the macOS `LocalPolishFallback`
//! rewrite with enabled-only operations (ADR 0054). Transforms clean dictation (grammar
//! slips, fillers, casing and terminal punctuation), the style shapes the
//! tone, and the preset's post-processors run last. It needs no model and
//! is what `deterministic_polish` inserts and what Enhanced falls back to.
//! The tokens the raw layer protects (paths, code spans, identifiers,
//! addresses) are hidden while the prose rules run and come back byte for
//! byte: a filler inside a backticked command, an `i` in a path or the
//! case of a file name is content, not prose.

mod post;
mod post_lists;
mod post_tokens;
pub(crate) mod text;

use std::collections::BTreeSet;

pub use dettivo_core::config::polish_schema::{PostProcessor, Preset, Style, Transform};

use text::{
    capitalize_leading_word, collapse_whitespace, is_match, is_sentence_terminator,
    looks_like_question, re, replace_all,
};

pub use post::{add_at_prefix_to_file_paths, apply_post_processors};
pub(crate) use post_tokens::normalize_spoken_model_identifiers;

/// The Polish rewrite of `input` under the given transforms, style and
/// post-processors. An input that is only whitespace comes back unchanged;
/// a protected token comes back as dictated.
pub fn rewrite(
    input: &str,
    transforms: &BTreeSet<Transform>,
    post_processors: &[PostProcessor],
    style: Style,
) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return input.to_string();
    }
    let hidden = crate::raw::hide(trimmed);
    let mut rewritten = normalize_whitespace(&hidden.text);
    if transforms.contains(&Transform::FixGrammar) {
        rewritten = apply_common_fixes(&rewritten);
    }
    if transforms.contains(&Transform::RemoveFillers) {
        rewritten = remove_fillers(&rewritten);
        rewritten = normalize_whitespace(&rewritten);
    }
    if transforms.contains(&Transform::SmartPunctuation) {
        rewritten = rewritten.trim().to_string();
        rewritten = tighten_punctuation_spacing(&rewritten);
        rewritten = capitalize_leading_word(&rewritten);
        rewritten = ensure_terminal_punctuation(&rewritten);
    }
    rewritten = apply_style(&rewritten, style);
    rewritten = rewritten.trim().to_string();
    let result = if rewritten.is_empty() {
        trimmed.to_string()
    } else {
        hidden.restore(&rewritten)
    };
    if post_processors.is_empty() {
        return result;
    }
    apply_post_processors(post_processors, &result)
}

fn normalize_whitespace(text: &str) -> String {
    text.split('\n')
        .map(collapse_whitespace)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The filler words the `removeFillers` transform drops.
pub(crate) const FILLER_PATTERN: &str = r"(?i)\b(?:uh|um|uhm|erm|hmm|ah|eh|äh|ähm)\b[ \t,]*";

fn remove_fillers(text: &str) -> String {
    replace_all(re!(FILLER_PATTERN), text, " ")
}

fn tighten_punctuation_spacing(text: &str) -> String {
    replace_all(re!(r"\s+([,.;:!?])"), text, "$1")
}

fn ensure_terminal_punctuation(text: &str) -> String {
    match text.chars().last() {
        Some(last) if !is_sentence_terminator(last) => {
            let mark = if looks_like_question(text) { "?" } else { "." };
            format!("{text}{mark}")
        }
        _ => text.to_string(),
    }
}

fn apply_common_fixes(text: &str) -> String {
    let mut fixed = replace_quoted_speech(text);
    for (pattern, replacement) in [
        (r"(?i)\bithink\b", "i think"),
        (r"(?i)\bthats\b", "that's"),
        (r"(?i)\bths\b", "this"),
        (r"(?i)\bteh\b", "the"),
        (r"(?i)\bdont\b", "don't"),
        (r"(?i)\bcant\b", "can't"),
        (r"(?i)\bwont\b", "won't"),
        (r"(?i)\bat sign\b", "@ sign"),
    ] {
        let re = fancy_regex::Regex::new(pattern).expect("regex");
        fixed = replace_all(&re, &fixed, replacement);
    }
    replace_all(re!(r"\bi\b"), &fixed, "I")
}

fn replace_quoted_speech(text: &str) -> String {
    replace_all(
        re!(r"(?i)\bquote\b[\s,]+(.+?)[\s,]+\bunquote\b"),
        text,
        "\"$1\"",
    )
}

fn apply_style(text: &str, style: Style) -> String {
    match style {
        Style::AsDictated => text.to_string(),
        Style::Formal => {
            let styled = replace_all(re!(r"(?i)^\s*hey\b"), text, "Hello");
            let styled = replace_all(re!(r"(?i)\bthanks\b"), &styled, "thank you");
            capitalize_leading_word(&styled)
        }
        Style::Casual => capitalize_leading_word(text),
        Style::VeryCasual => {
            let mut styled = text.to_lowercase();
            if styled.chars().last().is_some_and(is_sentence_terminator) {
                styled.pop();
            }
            styled
        }
    }
}

/// True when `text` carries a high-confidence fixable issue for one of
/// the enabled transforms (the no-op guard's signal).
pub(crate) fn has_fixable_issue(text: &str, transforms: &BTreeSet<Transform>) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    if transforms.contains(&Transform::RemoveFillers)
        && is_match(
            re!(r"(?i)\b(?:uh|um|uhm|erm|hmm|ah|eh|like|you know|basically|actually)\b"),
            trimmed,
        )
    {
        return true;
    }
    if transforms.contains(&Transform::FixGrammar)
        && is_match(re!(r"(?i)\b(?:ithink|ths|teh|dont|cant|wont)\b"), trimmed)
    {
        return true;
    }
    if transforms.contains(&Transform::SmartPunctuation) && is_likely_english_ascii(trimmed) {
        if trimmed.chars().next().is_some_and(char::is_lowercase) {
            return true;
        }
        if trimmed.chars().last().is_some_and(|c| !".!?…".contains(c)) {
            return true;
        }
    }
    false
}

fn is_likely_english_ascii(text: &str) -> bool {
    if text.is_empty() || !text.is_ascii() {
        return false;
    }
    let lowered = text.to_lowercase();
    [" the ", " and ", " i ", " you ", " we ", " to ", " for "]
        .iter()
        .any(|m| lowered.contains(m))
        || lowered.starts_with("i ")
        || lowered.starts_with("you ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> BTreeSet<Transform> {
        Transform::ALL.into_iter().collect()
    }

    #[test]
    fn fillers_grammar_and_style_follow_the_port() {
        assert_eq!(
            rewrite(
                "um  i need the update uh now",
                &all(),
                &[],
                Style::AsDictated
            ),
            "I need the update now."
        );
        assert_eq!(
            rewrite("hey team thanks", &all(), &[], Style::Formal),
            "Hello team thank you."
        );
        assert_eq!(
            rewrite("Hey Team Now.", &all(), &[], Style::VeryCasual),
            "hey team now"
        );
        assert_eq!(rewrite("   ", &all(), &[], Style::AsDictated), "   ");
    }

    /// dictation/F12: the prose rules stop at a protected token.
    #[test]
    fn protected_tokens_are_not_prose() {
        assert_eq!(
            rewrite("um run `um  i think` now", &all(), &[], Style::AsDictated),
            "Run `um  i think` now."
        );
        assert_eq!(
            rewrite("open src/i/index.ts", &all(), &[], Style::AsDictated),
            "Open src/i/index.ts."
        );
        assert_eq!(
            rewrite(
                "Hey read README.md and MyApp.swift.",
                &all(),
                &[],
                Style::VeryCasual
            ),
            "hey read README.md and MyApp.swift"
        );
        assert_eq!(
            rewrite("thanks for gordon@mickel.tech", &all(), &[], Style::Formal),
            "Thank you for gordon@mickel.tech."
        );
        assert!(has_fixable_issue(
            "i need the final draft by tomorrow",
            &all()
        ));
        assert!(!has_fixable_issue(
            "hola necesito el reporte para mañana",
            &all()
        ));
    }
}
