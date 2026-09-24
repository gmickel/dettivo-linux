//! Text helpers the Polish layers share: cached regular expressions,
//! whitespace and casing, sentence terminators and the multilingual
//! question detector (ported from the macOS `LocalPolishFallback` and
//! `PolishOutputGuard`).

use fancy_regex::Regex;

/// A regular expression compiled once; a pattern that fails to compile
/// is a programming error and panics at first use.
macro_rules! re {
    ($pattern:expr) => {{
        static RE: std::sync::LazyLock<fancy_regex::Regex> =
            std::sync::LazyLock::new(|| fancy_regex::Regex::new($pattern).expect("regex"));
        &*RE
    }};
}
pub(crate) use re;

/// True when `re` matches anywhere in `text`.
pub(crate) fn is_match(re: &Regex, text: &str) -> bool {
    re.is_match(text).unwrap_or(false)
}

/// Every replacement of `re` in `text` by `template` (`$1`, `${1}`).
pub(crate) fn replace_all(re: &Regex, text: &str, template: &str) -> String {
    re.replace_all(text, template).into_owned()
}

/// The match ranges of `re` in `text`, in order.
pub(crate) fn ranges(re: &Regex, text: &str) -> Vec<(usize, usize)> {
    re.find_iter(text)
        .filter_map(Result::ok)
        .map(|m| (m.start(), m.end()))
        .collect()
}

/// Runs of whitespace become one space.
pub(crate) fn collapse_whitespace(text: &str) -> String {
    replace_all(re!(r"\s+"), text, " ")
}

/// Two or more spaces (or other whitespace) become one space.
pub(crate) fn collapse_double_spaces(text: &str) -> String {
    replace_all(re!(r"\s{2,}"), text, " ")
}

/// The first character upper-cased when it is lowercase.
pub(crate) fn capitalize_leading_word(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_lowercase() => first.to_uppercase().chain(chars).collect(),
        _ => text.to_string(),
    }
}

/// `.`, `!`, `?`, the ellipsis and the Arabic and full-width question marks.
pub(crate) fn is_sentence_terminator(c: char) -> bool {
    ".!?…؟？".contains(c)
}

/// The number of characters (Unicode scalar values), what the macOS
/// `String.count` measures for the guard's bounds.
pub(crate) fn char_count(text: &str) -> usize {
    text.chars().count()
}

const QUESTION_STARTERS: &[&str] = &[
    "can ",
    "could ",
    "would ",
    "should ",
    "do ",
    "does ",
    "did ",
    "is ",
    "are ",
    "am ",
    "was ",
    "were ",
    "will ",
    "have ",
    "has ",
    "had ",
    "what ",
    "when ",
    "where ",
    "why ",
    "who ",
    "how ",
    "kann ",
    "kannst ",
    "können ",
    "könnt ",
    "wie ",
    "warum ",
    "wer ",
    "wo ",
    "wann ",
    "puede ",
    "puedes ",
    "podria ",
    "podría ",
    "por que ",
    "por qué ",
    "como ",
    "cómo ",
    "que ",
    "qué ",
    "cuando ",
    "cuándo ",
    "donde ",
    "dónde ",
    "quien ",
    "quién ",
    "peux ",
    "pouvez ",
    "est ce que ",
    "est-ce que ",
    "comment ",
    "pourquoi ",
    "quand ",
    "où ",
    "pode ",
    "podes ",
    "por que ",
    "por quê ",
    "como ",
    "quando ",
    "onde ",
    "هل ",
];

const QUESTION_FILLERS: &[&str] = &[
    "hey ", "hi ", "hello ", "hallo ", "hola ", "salut ", "oi ", "yo ", "oye ", "team ", "uh ",
    "um ", "äh ", "euh ", "so ", "well ", "ok ", "okay ",
];

/// Leading greetings and fillers removed, repeatedly.
pub(crate) fn normalize_question_lead(text: &str) -> String {
    let mut normalized = text.to_string();
    let mut stripped = true;
    while stripped {
        stripped = false;
        for filler in QUESTION_FILLERS {
            if let Some(rest) = normalized.strip_prefix(filler) {
                normalized = rest.trim_start_matches(' ').to_string();
                stripped = true;
                break;
            }
        }
    }
    normalized
}

/// True when the text reads as a question in any of the languages the
/// prompt lists: a question mark, a question starter after the fillers,
/// an in-sentence cue or a Japanese or Chinese question suffix.
pub(crate) fn looks_like_question(text: &str) -> bool {
    if text.contains('?') || text.contains('؟') || text.contains('？') {
        return true;
    }
    let lowered = normalize_question_lead(&text.trim().to_lowercase());
    if lowered.is_empty() {
        return false;
    }
    if QUESTION_STARTERS.iter().any(|s| lowered.starts_with(s)) {
        return true;
    }
    if is_match(
        re!(
            r"\b(?:can you|could you|would you|should we|do you|does it|did you|is it|are you|will you|have you|has it|had you|kannst du|könnt ihr|können wir|puedes|puede|podr(?:ia|ía)|me pasas|peux tu|pouvez vous|est ce que|est-ce que|pode|podes|هل)\b"
        ),
        &lowered,
    ) {
        return true;
    }
    let trimmed = text.trim();
    trimmed.ends_with("ですか") || trimmed.ends_with("ますか") || trimmed.ends_with('吗')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_behave_like_the_swift_originals() {
        assert_eq!(collapse_whitespace("a  b\n c"), "a b c");
        assert_eq!(capitalize_leading_word("hello"), "Hello");
        assert_eq!(capitalize_leading_word("Hello"), "Hello");
        assert_eq!(capitalize_leading_word("ärger"), "Ärger");
        assert!(looks_like_question("hey did you push the branch yet"));
        assert!(looks_like_question(
            "hallo kannst du mir den stand schicken"
        ));
        assert!(looks_like_question("この案で進めてもいいですか"));
        assert!(!looks_like_question("hola necesito el reporte"));
        assert_eq!(
            replace_all(
                re!(r"(?i)(@?)readme\.md\b"),
                "the readme.md file",
                "${1}README.md"
            ),
            "the README.md file"
        );
    }
}
