//! The guard's length rules (fn-94, ADR 0032): a rewrite whose word
//! count is implausible beside the dictation, a rewrite that is the
//! deterministic draft with one or two meaningful words taken out, and
//! the output budget a local rewrite is generated within.

use super::TOKEN_PATTERN;
use crate::polish::text::re;

/// A rewrite with more than this many times the dictation's words ran
/// away.
const MAX_GROWTH: usize = 2;
/// A rewrite with under this share of the dictation's words collapsed...
const MIN_SHARE: f64 = 0.4;
/// ...once the dictation has more words than this.
const COLLAPSE_AFTER_WORDS: usize = 8;
/// The most words a deletion may take out and still count as short.
const SHORT_DELETION: usize = 2;

/// Words a rewrite may drop: fillers, hedges, intensifiers and the
/// parts of the spoken phrases ("you know", "I mean", "kind of") they
/// come in.
const FILLERS: &[&str] = &[
    "um",
    "umm",
    "uh",
    "uhm",
    "er",
    "erm",
    "ah",
    "eh",
    "hm",
    "hmm",
    "mm",
    "äh",
    "ähm",
    "öhm",
    "like",
    "so",
    "well",
    "basically",
    "actually",
    "literally",
    "really",
    "just",
    "okay",
    "ok",
    "yeah",
    "right",
    "anyway",
    "kind",
    "sort",
    "of",
    "you",
    "know",
    "i",
    "mean",
    "probably",
    "definitely",
    "totally",
    "pretty",
    "very",
    "quite",
    "honestly",
    "obviously",
    "simply",
];

/// Tokens the Qwen tokenizer spends on the two output markers and the
/// line breaks around them (14 per marker, measured).
const FRAMING_TOKENS: u32 = 32;

fn words(text: &str) -> Vec<String> {
    re!(TOKEN_PATTERN)
        .find_iter(text)
        .filter_map(Result::ok)
        .map(|m| m.as_str().to_lowercase())
        .collect()
}

/// Why the rewrite's length is implausible beside the dictation: more
/// than twice its words, or under 0.4 of them once it has more than
/// eight. Technical dictation is exempt from the second rule, as from the
/// guard's contraction rule: a spoken path (`src slash app slash index
/// dot ts`) is five words that rightly become one. The reasons are the
/// guard's existing expansion and contraction reasons, so the Enhanced
/// pass treats them the same way.
pub(super) fn implausible_length(input: &str, output: &str) -> Option<&'static str> {
    let input_words = words(input).len();
    let output_words = words(output).len();
    if input_words == 0 {
        return None;
    }
    if output_words > input_words * MAX_GROWTH {
        return Some("output expanded beyond rewrite bounds");
    }
    if input_words > COLLAPSE_AFTER_WORDS
        && (output_words as f64) < input_words as f64 * MIN_SHARE
        && !super::contains_technical_markers(input)
    {
        return Some("output dropped too much dictated content");
    }
    None
}

/// The prefix of the reason [`dropped_dictated_words`] gives.
pub const DROPPED_WORDS: &str = "output dropped the dictated word";

/// Why a rewrite is rejected when it is `draft` (the deterministic
/// result) with one or two meaningful words taken out and nothing else
/// changed but casing and punctuation: `not` dropped from "we should not
/// ship" turns the dictation around. Fillers may go, and so may a word
/// said twice in a row and kept once. The words the rewrite keeps must
/// stand in the draft's order, so a reordered sentence is a rewrite. The
/// draft, not the dictation, is the reference, because the deterministic
/// transforms drop words of their own (`R and D` becomes `R&D`).
pub fn dropped_dictated_words(draft: &str, output: &str) -> Option<String> {
    let meaningful = |text: &str| {
        let mut out: Vec<String> = Vec::new();
        for word in words(text) {
            if !FILLERS.contains(&word.as_str()) && out.last() != Some(&word) {
                out.push(word);
            }
        }
        out
    };
    let draft = meaningful(draft);
    let mut kept = meaningful(output).into_iter().peekable();
    let mut removed: Vec<&str> = Vec::new();
    for word in &draft {
        if kept.peek() == Some(word) {
            kept.next();
        } else {
            removed.push(word);
        }
    }
    // A word left over is new or moved: a rewrite, not a deletion.
    if kept.next().is_some() || removed.is_empty() || removed.len() > SHORT_DELETION {
        return None;
    }
    Some(format!("{DROPPED_WORDS} {}", removed.join(" ")))
}

/// The most tokens a rewrite of `input` may generate: 1.5 times the
/// input's tokens plus 32, plus the output markers. The input's tokens
/// are estimated at one per three characters, which the Qwen tokenizer
/// stays under on 421 of the 431 private eval transcripts, so the budget
/// errs long rather than cutting a sound rewrite short.
pub fn output_budget(input: &str) -> u32 {
    let tokens = input.chars().count().div_ceil(3) as u32;
    tokens + tokens.div_ceil(2) + 32 + FRAMING_TOKENS
}

#[cfg(test)]
mod tests {
    use super::super::rejection_reason;
    use super::*;

    /// Rule 1: twice the dictation's words. The character cap the guard
    /// had never looks at a dictation under 24 characters.
    #[test]
    fn a_rewrite_with_more_than_twice_the_words_is_rejected() {
        let input = "ship it tomorrow";
        let output = "Ship it tomorrow, and tell everyone you know.";
        assert!(!super::super::is_unexpected_expansion(input, output));
        assert_eq!(
            rejection_reason(input, output),
            Some("output expanded beyond rewrite bounds".into())
        );
        assert_eq!(rejection_reason(input, "Ship it tomorrow, please."), None);
    }

    /// Rule 1: under 0.4 of the words once the dictation passes eight.
    /// The contraction rule the guard had lets any rewrite through that
    /// ends a sentence.
    #[test]
    fn a_rewrite_that_collapses_to_a_fraction_of_the_words_is_rejected() {
        let input = "we need to move the launch review to thursday because the design team is out";
        let output = "Move the review.";
        assert!(!super::super::is_unexpected_contraction(input, output));
        assert_eq!(
            rejection_reason(input, output),
            Some("output dropped too much dictated content".into())
        );
        // Eight words or fewer never trip the share rule.
        assert_eq!(
            implausible_length("please send me the notes from today", "Send notes."),
            None
        );
        // A spoken path rightly collapses into one token.
        assert_eq!(
            rejection_reason(
                "read dettivo slash services slash polish slash polish rewrite engine dot swift",
                "Read @dettivo/services/polish/PolishRewriteEngine.swift"
            ),
            None
        );
    }

    /// Rule 3: a short deletion of meaningful words. Nothing in the
    /// guard noticed `not` going, `not` being a stop word.
    #[test]
    fn a_draft_with_one_or_two_meaningful_words_taken_out_is_rejected() {
        let draft = "We should not ship the release on Friday.";
        let output = "We should ship the release on Friday.";
        assert_eq!(rejection_reason(draft, output), None);
        assert_eq!(
            dropped_dictated_words(draft, output),
            Some("output dropped the dictated word not".into())
        );
        assert_eq!(
            dropped_dictated_words(
                "We should not ship the big release.",
                "We should ship the release."
            ),
            Some("output dropped the dictated word not big".into())
        );
    }

    /// A repeated word kept once never covers a meaningful word dropped
    /// beside it (review finding, fn-94).
    #[test]
    fn a_repeat_kept_once_does_not_excuse_a_dropped_not() {
        assert_eq!(
            dropped_dictated_words(
                "We should not ship the the release.",
                "We should ship the release."
            ),
            Some("output dropped the dictated word not".into())
        );
        assert_eq!(
            dropped_dictated_words(
                "We should not ship the release and ship the hotfix.",
                "We should ship the release and the hotfix."
            ),
            Some("output dropped the dictated word not ship".into())
        );
    }

    /// A reordered sentence is a rewrite, never a deletion.
    #[test]
    fn a_reordered_rewrite_is_not_a_deletion() {
        assert_eq!(
            dropped_dictated_words(
                "The hotfix is what we should ship.",
                "We should ship the hotfix."
            ),
            None
        );
    }

    #[test]
    fn fillers_repeats_rewrites_and_longer_cuts_are_not_short_deletions() {
        let draft = "So I think we should really ship the the release.";
        // Fillers and a repeated word go.
        assert_eq!(
            dropped_dictated_words(draft, "I think we should ship the release."),
            None
        );
        // A word the draft lacks makes it a rewrite.
        assert_eq!(
            dropped_dictated_words(draft, "We should launch the release."),
            None
        );
        // Unchanged, or recased and repunctuated, is no deletion at all.
        assert_eq!(dropped_dictated_words(draft, draft), None);
        assert_eq!(dropped_dictated_words("ship it now", "Ship it now!"), None);
        // Three meaningful words are past a short deletion.
        assert_eq!(
            dropped_dictated_words(
                "We should not ship the release on Friday.",
                "We should ship the release."
            ),
            None
        );
    }

    /// Rule 2: 1.5 times the input's tokens plus 32, plus the markers.
    #[test]
    fn the_output_budget_grows_with_the_dictation() {
        // 30 characters: 10 tokens, 15 for the rewrite, 32 slack, 32 markers.
        assert_eq!(output_budget("we should ship this on friday."), 79);
        assert_eq!(output_budget(""), 64);
        assert!(output_budget(&"word ".repeat(200)) > 500);
    }
}
