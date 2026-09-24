//! The selected post-processors, in configuration order. Editorial changes
//! run only through their named transform or processor (ADR 0054).

use super::PostProcessor;
use super::post_lists::normalize_spoken_lists;
use super::post_tokens::{
    normalize_business_abbreviations, normalize_plus_page_phrases, normalize_simple_spoken_slash,
    normalize_spoken_technical_tokens,
};
use super::text::ranges;

/// The file extensions `@path` insertion recognises (a conservative list).
pub(crate) const FILE_EXTENSIONS: &[&str] = &[
    "md", "swift", "txt", "json", "yml", "yaml", "ts", "js", "jsx", "tsx", "py", "sh", "css",
    "html", "xml", "plist",
];

/// Runs each selected processor once, in the configured order.
pub fn apply_post_processors(processors: &[PostProcessor], text: &str) -> String {
    let mut seen = Vec::new();
    let mut result = text.to_string();
    for id in processors {
        if !seen.contains(id) {
            result = apply_one(*id, &result);
            seen.push(*id);
        }
    }
    result
}

fn apply_one(id: PostProcessor, text: &str) -> String {
    match id {
        PostProcessor::AtPrefixFilePaths => text
            .split('\n')
            .map(|line| add_at_prefix_to_file_paths(&normalize_spoken_technical_tokens(line)))
            .collect::<Vec<_>>()
            .join("\n"),
        PostProcessor::NormalizeSimpleSpokenSlash => normalize_simple_spoken_slash(text),
        PostProcessor::NormalizeBusinessAbbreviations => normalize_business_abbreviations(text),
        PostProcessor::NormalizePlusPagePhrases => normalize_plus_page_phrases(text),
        PostProcessor::NormalizeSpokenLists => normalize_spoken_lists(text),
    }
}

fn extension_alternation() -> String {
    FILE_EXTENSIONS.join("|")
}

/// `foo.swift` becomes `@foo.swift`; idempotent, URL, email and glued
/// relative paths left alone.
pub fn add_at_prefix_to_file_paths(text: &str) -> String {
    let pattern = format!(
        r"(?:^|(?<=\s))(?:\.\.?/)?(?:[a-zA-Z0-9_][a-zA-Z0-9_.-]*/)*[a-zA-Z0-9_][a-zA-Z0-9_.-]*\.(?:{})(?=\s|[.,;:!?]|$)",
        extension_alternation()
    );
    let re = fancy_regex::Regex::new(&pattern).expect("regex");
    let mut result = text.to_string();
    for (start, end) in ranges(&re, text).into_iter().rev() {
        let matched = result[start..end].to_string();
        let preceding: String = result[..start]
            .chars()
            .rev()
            .take(10)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if preceding.ends_with('@') || preceding.contains("://") {
            continue;
        }
        if preceding.contains('@') && !preceding.contains(' ') {
            continue;
        }
        if (matched.starts_with("./") || matched.starts_with("../"))
            && preceding.chars().last().is_some_and(char::is_alphabetic)
        {
            continue;
        }
        if let Some(pos) = matched.find("./").or_else(|| matched.find("../")) {
            if pos > 0 && matched[..pos].chars().any(char::is_alphabetic) {
                continue;
            }
        }
        result.replace_range(start..end, &format!("@{matched}"));
    }
    result
}
