//! The guard's length rules (fn-94, ADR 0032): a rewrite whose word
//! count is implausible beside the dictation, a rewrite that is the
//! deterministic draft with one or two meaningful words taken out, and
//! the output budget a local rewrite is generated within.

use super::TOKEN_PATTERN;
use crate::polish::text::{is_match, re};

/// A rewrite with more than this many times the dictation's words ran
/// away.
const MAX_GROWTH: usize = 2;
/// A rewrite with under this share of the dictation's words collapsed...
const MIN_SHARE: f64 = 0.4;
/// ...once the dictation has more words than this.
const COLLAPSE_AFTER_WORDS: usize = 8;
/// The most words a deletion may take out and still count as short.
const SHORT_DELETION: usize = 2;

/// Words a rewrite may drop wherever they stand: hesitation sounds,
/// hedges and intensifiers. None of them changes what a sentence says.
#[rustfmt::skip]
const FILLERS: &[&str] = &[
    "um", "umm", "uh", "uhm", "er", "erm", "ah", "eh", "hm", "hmm", "mm", "äh", "ähm", "öhm",
    "basically", "actually", "literally", "really", "just", "probably", "definitely",
    "totally", "very", "quite", "honestly", "obviously", "simply",
];

/// Spoken phrases a rewrite may drop, only as the whole phrase: `you`,
/// `mean` and `kind` alone carry meaning.
const FILLER_PHRASES: &[[&str; 2]] = &[
    ["you", "know"],
    ["i", "mean"],
    ["kind", "of"],
    ["sort", "of"],
];

/// Words a rewrite may drop only where they open a sentence (`So we
/// should`, `Well, ...`); elsewhere they carry meaning (`turn right`, `I
/// like it`).
const OPENERS: &[&str] = &[
    "so", "well", "like", "okay", "ok", "yeah", "right", "anyway",
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

/// True when `output` carries a path, a file name, a flag or a code
/// span: the tokens a spoken `slash`, `dot` or `dash dash` collapse into.
fn joins_technical_tokens(output: &str) -> bool {
    is_match(
        re!(
            r"[\w@.-]/[\w@.-]|\b[\w-]+\.(?:swift|rs|toml|ts|tsx|js|jsx|py|md|json|yaml|yml|sh|zsh|plist|txt|csv)\b|`|(?:^|\s)--[a-z0-9-]"
        ),
        output,
    )
}

/// Why the rewrite's length is implausible beside the dictation: more
/// than twice its words, or under 0.4 of them once it has more than
/// eight. A rewrite that carries a path, file name, flag or code span is
/// exempt from the second rule: a spoken path (`src slash app slash index
/// dot ts`) is five words that rightly become one. The reasons are the
/// guard's existing expansion and contraction reasons, so the Enhanced
/// pass treats them the same way. A dictation in a script written without
/// spaces between words (Chinese, Japanese, Thai) has no word count to
/// compare, so only the guard's character cap applies to it.
pub(super) fn implausible_length(input: &str, output: &str) -> Option<&'static str> {
    if is_match(
        re!(r"[\p{Han}\p{Hiragana}\p{Katakana}\p{Thai}\p{Lao}\p{Khmer}\p{Myanmar}]"),
        input,
    ) {
        return None;
    }
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
        && !joins_technical_tokens(output)
    {
        return Some("output dropped too much dictated content");
    }
    None
}

/// The prefix of the reason [`dropped_dictated_words`] gives.
pub const DROPPED_WORDS: &str = "output dropped the dictated word";

/// The words of `text`, lowercased, each with whether it opens a
/// sentence or a paragraph.
fn tokens(text: &str) -> Vec<(String, bool)> {
    re!(TOKEN_PATTERN)
        .find_iter(text)
        .filter_map(Result::ok)
        .map(|m| {
            let opens = text[..m.start()]
                .trim_end_matches([' ', '\t'])
                .chars()
                .last()
                .is_none_or(|c| ".!?\n".contains(c));
            (m.as_str().to_lowercase(), opens)
        })
        .collect()
}

/// True when the draft's word `i`, one of `removed`, may go: a filler, a
/// sentence opener, half of a filler phrase whose other half goes too,
/// or a word said twice in a row.
fn droppable(draft: &[(String, bool)], removed: &[usize], i: usize) -> bool {
    let (word, opens) = &draft[i];
    let at = |j: usize| draft.get(j).map(|(w, _)| w.as_str());
    let prev = i.checked_sub(1);
    FILLERS.contains(&word.as_str())
        || (*opens && OPENERS.contains(&word.as_str()))
        || FILLER_PHRASES.iter().any(|[a, b]| {
            (word == a && at(i + 1) == Some(*b) && removed.contains(&(i + 1)))
                || (word == b
                    && prev.and_then(at) == Some(*a)
                    && prev.is_some_and(|p| removed.contains(&p)))
        })
        || prev.and_then(at) == Some(word.as_str())
        || at(i + 1) == Some(word.as_str())
}

/// Why a rewrite is rejected when it is `draft` (the deterministic
/// result) with one or two meaningful words taken out and nothing else
/// changed but casing and punctuation: `not` dropped from "we should not
/// ship" turns the dictation around. The rewrite's words are aligned with
/// the draft's as written, in order, before any word is excused, so a
/// reordered or reworded sentence is a rewrite, and a deletion that brings
/// two words together (`you do not know` to `you know`) still shows what
/// it dropped. Each removed word is then judged where it stood in the
/// draft: fillers, sentence openers, whole filler phrases and a word said
/// twice in a row may go. The draft, not the dictation, is the reference,
/// because the deterministic transforms drop words of their own (`R and
/// D` becomes `R&D`).
pub fn dropped_dictated_words(draft: &str, output: &str) -> Option<String> {
    let draft = tokens(draft);
    let mut kept = tokens(output).into_iter().map(|(w, _)| w).peekable();
    let mut removed: Vec<usize> = Vec::new();
    for (i, (word, _)) in draft.iter().enumerate() {
        if kept.peek() == Some(word) {
            kept.next();
        } else {
            removed.push(i);
        }
    }
    // A word left over is new or moved: a rewrite, not a deletion.
    if kept.next().is_some() {
        return None;
    }
    let dropped: Vec<&str> = removed
        .iter()
        .filter(|&&i| !droppable(&draft, &removed, i))
        .map(|&i| draft[i].0.as_str())
        .collect();
    if dropped.is_empty() || dropped.len() > SHORT_DELETION {
        return None;
    }
    Some(format!("{DROPPED_WORDS} {}", dropped.join(" ")))
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
        // `Let` is a code keyword to the technical check, never an
        // exemption here (PR #57 review).
        assert_eq!(
            rejection_reason(
                "let me know if the launch review can move to thursday because design is out",
                "Let me know."
            ),
            Some("output dropped too much dictated content".into())
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

    /// `right`, `you` or `like` mid-sentence carry meaning; only a whole
    /// filler phrase or a sentence opener may go (PR #57 review).
    #[test]
    fn ambiguous_fillers_count_unless_they_open_a_sentence_or_form_a_phrase() {
        let dropped = |w: &str| Some(format!("output dropped the dictated word {w}"));
        assert_eq!(
            dropped_dictated_words(
                "We need to turn right at the next junction.",
                "We need to turn at the next junction."
            ),
            dropped("right")
        );
        assert_eq!(
            dropped_dictated_words("I need you to call them.", "I need to call them."),
            dropped("you")
        );
        assert_eq!(
            dropped_dictated_words("I like the new layout.", "I the new layout."),
            dropped("like")
        );
        assert_eq!(
            dropped_dictated_words(
                "So we should ship it. Well, maybe.",
                "We should ship it. Maybe."
            ),
            None
        );
        assert_eq!(
            dropped_dictated_words("We should, you know, ship it.", "We should ship it."),
            None
        );
    }

    /// A repeat counts only as dictated, in the draft (PR #57 review): a
    /// filler between two words, or a deletion that brings two words
    /// together, never hides a dropped `not`.
    #[test]
    fn only_a_repeat_dictated_in_a_row_collapses() {
        let dropped = |w: &str| Some(format!("output dropped the dictated word {w}"));
        assert_eq!(
            dropped_dictated_words(
                "We should not just not ship the release.",
                "We should just not ship the release."
            ),
            dropped("not")
        );
        assert_eq!(
            dropped_dictated_words(
                "They had not had the risky migration approved by the team.",
                "They had had the migration approved by the team."
            ),
            dropped("not risky")
        );
    }

    /// Words are aligned before any is excused (PR #57 review): a
    /// deletion that forms a filler phrase, or one beside a repeat the
    /// rewrite keeps, still shows the dropped words, and a paragraph's
    /// opening `Well` may go.
    #[test]
    fn words_are_aligned_before_fillers_and_repeats_are_excused() {
        let dropped = |w: &str| Some(format!("output dropped the dictated word {w}"));
        assert_eq!(
            dropped_dictated_words(
                "I think you do not know the release date.",
                "I think you know the release date."
            ),
            dropped("do not")
        );
        assert_eq!(
            dropped_dictated_words(
                "We should not ship the the release.",
                "We should ship the the release."
            ),
            dropped("not")
        );
        assert_eq!(
            dropped_dictated_words(
                "We should ship tomorrow\n\nWell, the tests need another pass.",
                "We should ship tomorrow.\n\nThe tests need another pass."
            ),
            None
        );
    }

    /// Text without spaces between words has no word count to compare
    /// (PR #57 review); punctuation cleanup passes.
    #[test]
    fn unsegmented_scripts_skip_the_word_ratio_rules() {
        assert_eq!(
            rejection_reason(
                "今天先修复登录问题明天再检查支付流程最后安排上线时间",
                "今天先修复登录问题。明天再检查支付流程。最后安排上线时间。"
            ),
            None
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
