//! The output guard (FR-M7), ported from the macOS `PolishOutputGuard`
//! and `ThinkingBlockStripper`: a rewrite that answers the transcript,
//! refuses it, asks the user for input, leaks the markers or the repair
//! scaffold, invents a code fence, drops dictated content, loses a
//! question mark or runs away in length is rejected, and the Polish
//! result is inserted instead.

use std::collections::BTreeSet;

use crate::polish::text::{char_count, is_match, looks_like_question, ranges, re, replace_all};

/// Reasoning blocks removed. An output that is only a reasoning block
/// comes back unchanged when `preserve_original_if_empty`, and empty
/// otherwise so the caller can fall back.
pub fn strip_thinking(output: &str, preserve_original_if_empty: bool) -> String {
    let stripped = replace_all(re!(r"(?is)<think\b[^>]*>.*?</think>"), output, "");
    let trimmed = stripped.trim();
    if trimmed.is_empty() && preserve_original_if_empty {
        return output.to_string();
    }
    trimmed.to_string()
}

const REFUSAL_PHRASES: &[&str] = &[
    "i don't have access",
    "i do not have access",
    "i can't access",
    "i cannot access",
    "not provided in your message",
    "as an ai",
    "i can help you understand",
    "however, i can help",
    "i'm glad you",
    "if you have any specific questions",
    "feel free to ask",
];

const STOP_WORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "but", "if", "to", "of", "in", "on", "for", "with", "this",
    "that", "these", "those", "it", "is", "are", "was", "were", "be", "been", "i", "you", "we",
    "they", "he", "she", "my", "your", "our", "their", "can", "could", "would", "should", "do",
    "does", "did", "will", "have", "has", "had", "whether", "before", "after", "not", "now",
    "just", "really",
];

const TOKEN_PATTERN: &str = r"(?:[\p{L}\p{N}_]+(?:[./:-][\p{L}\p{N}_]+)*)";

/// Why a rewrite was rejected, or `None` when it may be inserted.
pub fn rejection_reason(input: &str, output: &str) -> Option<String> {
    let trimmed_input = input.trim();
    let normalized = strip_thinking(output, false);
    let trimmed_output = normalized.trim();
    if trimmed_output.is_empty() {
        return Some("empty output".into());
    }
    if contains_transcript_markers(trimmed_output) {
        return Some("output included transcript markers".into());
    }
    if contains_repair_scaffold_tags(trimmed_output) {
        return Some("output included repair scaffold tags".into());
    }
    if trimmed_output.contains("```") && !trimmed_input.contains("```") {
        return Some("output included unexpected code fence".into());
    }
    if let Some(token) = dropped_protected_token(trimmed_input, trimmed_output) {
        return Some(format!("output dropped the protected token {token}"));
    }
    if !contains_technical_markers(trimmed_input)
        && (dropped_important_short_phrase_tokens(trimmed_input, trimmed_output)
            || is_unexpected_contraction(trimmed_input, trimmed_output))
    {
        return Some("output dropped too much dictated content".into());
    }
    if dropped_question_punctuation(trimmed_input, trimmed_output) {
        return Some("output lost question punctuation".into());
    }
    if is_unexpected_expansion(trimmed_input, trimmed_output) {
        return Some("output expanded beyond rewrite bounds".into());
    }
    let input_lower = trimmed_input.to_lowercase();
    let output_lower = trimmed_output.to_lowercase();
    if REFUSAL_PHRASES
        .iter()
        .any(|p| output_lower.contains(p) && !input_lower.contains(p))
    {
        return Some("output appears to answer/refuse instead of rewriting".into());
    }
    for pattern in [
        re!(r"\b(?:please|kindly)\s+(?:provide|paste|share)\b"),
        re!(r"\b(?:can|could)\s+you\s+(?:provide|paste|share)\b"),
        re!(r"\bi(?:'|\u{2019})ll\s+(?:read|review|analy[sz]e|help)\b"),
    ] {
        if is_match(pattern, &output_lower) && !is_match(pattern, &input_lower) {
            return Some("output requested user input instead of rewriting".into());
        }
    }
    None
}

/// The numbers in `text` as the guard compares them: trailing sentence
/// punctuation dropped and thousands commas removed, the one formatting
/// change a rewrite may make. The decimal point, a colon and the
/// digits stay, so `1.5` is not `15`, `42` is not `420`, and `12` is not
/// `1` and `2`.
fn numbers(text: &str) -> Vec<String> {
    ranges(re!(r"\d[\d.,:]*"), text)
        .into_iter()
        .map(|(start, end)| {
            text[start..end]
                .trim_end_matches(['.', ',', ':'])
                .replace(',', "")
        })
        .collect()
}

/// The tokens a rewrite may never drop (a Linux addition to the macOS
/// guard, ADR 0023): a path, a code span between backticks, and every
/// number the dictation carried, each as often as it was dictated. The
/// macOS guard exempts technical text from its content checks, which is
/// exactly the text where a dropped path costs the most.
fn dropped_protected_token(input: &str, output: &str) -> Option<String> {
    let mut output_numbers = numbers(output);
    for (start, end) in ranges(re!(r"`[^`]+`"), input) {
        let span = &input[start..end];
        if !output.contains(span.trim_matches('`')) {
            return Some(span.to_string());
        }
    }
    for (start, end) in ranges(
        re!(
            r"(?:[\w@.-]+/[\w@./-]+|\b[\w-]+\.(?:swift|rs|toml|ts|tsx|js|jsx|py|md|json|yaml|yml|sh|zsh|plist|txt|csv))"
        ),
        input,
    ) {
        let span = &input[start..end];
        if !output.contains(span) {
            return Some(span.to_string());
        }
    }
    for (span, number) in ranges(re!(r"\d[\d.,:]*"), input)
        .into_iter()
        .map(|(start, end)| &input[start..end])
        .zip(numbers(input))
    {
        match output_numbers.iter().position(|n| *n == number) {
            Some(i) => {
                output_numbers.swap_remove(i);
            }
            None => return Some(span.to_string()),
        }
    }
    None
}

fn contains_transcript_markers(text: &str) -> bool {
    [
        super::prompt::TRANSCRIPT_BEGIN,
        super::prompt::TRANSCRIPT_END,
        super::prompt::OUTPUT_BEGIN,
        super::prompt::OUTPUT_END,
    ]
    .iter()
    .any(|m| text.contains(m))
}

fn contains_repair_scaffold_tags(text: &str) -> bool {
    let lowered = text.to_lowercase();
    [
        "<rewrite_draft>",
        "</rewrite_draft>",
        "<invalid_output>",
        "</invalid_output>",
    ]
    .iter()
    .any(|t| lowered.contains(t))
}

fn content_token_count(text: &str) -> usize {
    re!(TOKEN_PATTERN)
        .find_iter(text)
        .filter_map(Result::ok)
        .count()
}

fn important_tokens(text: &str) -> Vec<String> {
    re!(TOKEN_PATTERN)
        .find_iter(text)
        .filter_map(Result::ok)
        .map(|m| m.as_str().to_lowercase())
        .filter(|t| t.chars().count() >= 3 && !STOP_WORDS.contains(&t.as_str()))
        .collect()
}

fn is_unexpected_contraction(input: &str, output: &str) -> bool {
    if contains_technical_markers(input) {
        return false;
    }
    let input_words = content_token_count(input);
    if input_words < 6 {
        return false;
    }
    let output_words = content_token_count(output);
    let dropped = input_words.saturating_sub(output_words);
    let ratio = output_words as f64 / input_words as f64;
    let ends_sentence = output
        .chars()
        .last()
        .is_some_and(|c| ".!?\u{2026}".contains(c));
    if dropped >= 3 && ratio <= 0.70 && !ends_sentence {
        return true;
    }
    dropped >= 4 && !ends_sentence
}

fn dropped_important_short_phrase_tokens(input: &str, output: &str) -> bool {
    if contains_technical_markers(input) {
        return false;
    }
    if !(looks_like_question(input)
        || starts_with_conversational_repair_cue(input)
        || looks_like_repeated_short_list(input))
    {
        return false;
    }
    let tokens = important_tokens(input);
    if tokens.len() < 3 || tokens.len() > 10 {
        return false;
    }
    let present: BTreeSet<String> = important_tokens(output).into_iter().collect();
    tokens.iter().filter(|t| !present.contains(*t)).count() >= 2
}

fn is_unexpected_expansion(input: &str, output: &str) -> bool {
    let input_chars = char_count(input);
    if input_chars < 24 {
        return false;
    }
    let hard_cap = (input_chars + 40).max((input_chars as f64 * 1.8) as usize);
    char_count(output) > hard_cap
}

fn dropped_question_punctuation(input: &str, output: &str) -> bool {
    if !looks_like_question(input) {
        return false;
    }
    match output.trim().chars().last() {
        None => true,
        Some(last) => !"?؟？".contains(last),
    }
}

/// True when the text carries file names, code keywords, flags, spoken
/// separators or code punctuation.
pub fn contains_technical_markers(text: &str) -> bool {
    let lowered = text.to_lowercase();
    [
        re!(r"\b[a-z0-9_./-]+\.(?:swift|rs|toml|ts|tsx|js|jsx|py|md|json|yaml|yml|sh|zsh|plist)\b"),
        re!(r"\b(?:const|let|var|func|class|struct|enum|console\.log)\b"),
        re!(r"\s--[a-z0-9-]+"),
        re!(r"\bslash\b"),
        re!(r"\bdot\b"),
        re!(r"[=();{}\[\]`]"),
    ]
    .iter()
    .any(|re| is_match(re, &lowered))
}

fn starts_with_conversational_repair_cue(text: &str) -> bool {
    let lowered = text.trim().to_lowercase();
    ["i mean ", "no, ", "no ", "actually ", "sorry "]
        .iter()
        .any(|cue| lowered.starts_with(cue))
}

fn looks_like_repeated_short_list(text: &str) -> bool {
    let tokens = important_tokens(text);
    if tokens.len() < 4 || tokens.len() > 8 {
        return false;
    }
    let unique: BTreeSet<&String> = tokens.iter().collect();
    unique.len() < tokens.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thinking_blocks_are_stripped_before_the_guard_sees_the_output() {
        assert_eq!(
            strip_thinking("<think>hmm</think>Hello there.", false),
            "Hello there."
        );
        assert_eq!(
            strip_thinking("<think>only</think>", true),
            "<think>only</think>"
        );
        assert_eq!(strip_thinking("<think>only</think>", false), "");
        assert_eq!(
            rejection_reason("hello there", "<think>plan</think>"),
            Some("empty output".into())
        );
    }

    #[test]
    fn answers_refusals_fences_and_markers_are_rejected() {
        assert_eq!(
            rejection_reason("read the file", "As an AI I cannot access your files."),
            Some("output appears to answer/refuse instead of rewriting".into())
        );
        assert_eq!(
            rejection_reason("fix this", "Please provide the transcript."),
            Some("output requested user input instead of rewriting".into())
        );
        assert_eq!(
            rejection_reason("plain text", "```rust\nfn main() {}\n```"),
            Some("output included unexpected code fence".into())
        );
        assert_eq!(
            rejection_reason(
                "plain text",
                &format!("{} leak", super::super::prompt::OUTPUT_BEGIN)
            ),
            Some("output included transcript markers".into())
        );
        assert_eq!(
            rejection_reason("plain text", "<rewrite_draft>x</rewrite_draft>"),
            Some("output included repair scaffold tags".into())
        );
    }

    #[test]
    fn dropped_content_lost_questions_and_runaway_length_are_rejected() {
        assert_eq!(
            rejection_reason(
                "can you send the quarterly revenue deck to marketing today",
                "Send the deck"
            ),
            Some("output dropped too much dictated content".into())
        );
        assert_eq!(
            rejection_reason(
                "did you push the branch yet",
                "Did you push the branch yet."
            ),
            Some("output lost question punctuation".into())
        );
        let input = "we should review the onboarding flow before the release";
        let long = "We should review the onboarding flow before the release, and then also rewrite the documentation, the tests and the whole marketing site as well.";
        assert_eq!(
            rejection_reason(input, long),
            Some("output expanded beyond rewrite bounds".into())
        );
        assert_eq!(
            rejection_reason(
                input,
                "We should review the onboarding flow before the release."
            ),
            None
        );
    }

    #[test]
    fn a_dropped_path_code_span_or_number_is_rejected() {
        assert_eq!(
            rejection_reason(
                "check the report at docs/reports/q3.md and send it to the team",
                "Check the report and send it to the team."
            ),
            Some("output dropped the protected token docs/reports/q3.md".into())
        );
        assert_eq!(
            rejection_reason(
                "run `cargo test` before you push",
                "Run the tests before you push."
            ),
            Some("output dropped the protected token `cargo test`".into())
        );
        assert_eq!(
            rejection_reason(
                "we need 42 more seats by friday",
                "We need more seats by Friday."
            ),
            Some("output dropped the protected token 42".into())
        );
        // A number written back with a thousands separator is still there.
        assert_eq!(
            rejection_reason(
                "the 1000 page framework needs a summary",
                "The 1,000-page framework needs a summary."
            ),
            None
        );
    }

    /// dictation/F17: a number is compared whole and as often as it was
    /// said, never as a digit string another number happens to contain.
    #[test]
    fn a_changed_number_is_rejected_even_when_its_digits_survive() {
        let dropped = |n: &str| Some(format!("output dropped the protected token {n}"));
        assert_eq!(
            rejection_reason("we need 42 seats", "We need 420 seats."),
            dropped("42")
        );
        assert_eq!(
            rejection_reason("order 12 units", "Order 1 unit and 2 more."),
            dropped("12")
        );
        assert_eq!(
            rejection_reason("it takes 1.5 hours", "It takes 15 hours."),
            dropped("1.5")
        );
        assert_eq!(
            rejection_reason("call 3 people and 3 vendors", "Call 3 people and vendors."),
            dropped("3")
        );
        assert_eq!(
            rejection_reason("meet at 10:30 on the 5th", "Meet at 10:30 on the 5th."),
            None
        );
        assert_eq!(rejection_reason("we counted 42.", "We counted 42"), None);
    }

    #[test]
    fn technical_input_is_exempt_from_the_content_checks() {
        assert!(contains_technical_markers("check src/app/index.ts"));
        assert_eq!(
            rejection_reason("read src/app/index.ts and fix it", "Read src/app/index.ts."),
            None
        );
    }
}
