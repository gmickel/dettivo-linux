//! Token-level post-processors: spoken technical tokens (`dot`, `slash`,
//! `dash dash`, `colon`), spaced file system paths, spoken model
//! identifiers, business abbreviations and plus-page phrases.

use super::post::FILE_EXTENSIONS;
use super::text::{
    capitalize_leading_word, collapse_double_spaces, is_match, ranges, re, replace_all,
};

const KNOWN_MODEL_FAMILIES: &[&str] = &["qwen", "llama", "gemma", "mistral", "phi", "deepseek"];

fn extensions() -> String {
    FILE_EXTENSIONS.join("|")
}

/// Spoken separators become their marks and paths are glued back together.
pub(crate) fn normalize_spoken_technical_tokens(text: &str) -> String {
    let mut result = replace_all(re!(r"(?i)([A-Za-z0-9_.-])/?slash\b"), text, "${1} slash");
    for (pattern, replacement) in [
        (r"(?i)\bdash dash\b", "--"),
        (r"(?i)\bdot\b", "."),
        (r"(?i)\bcolumn\b", ":"),
        (r"(?i)\bcolon\b", ":"),
    ] {
        let re = fancy_regex::Regex::new(pattern).expect("regex");
        result = replace_all(&re, &result, replacement);
    }
    result = normalize_spoken_slash_for_technical_tokens(&result);
    result = replace_all(re!(r"(?<=\S)\s*/\s*(?=\S)"), &result, "/");
    result = normalize_spoken_dot_spacing(&result);
    result = replace_all(re!(r"(?<=\S)\s*:\s*(?=\S)"), &result, ":");
    result = normalize_spaced_filesystem_tokens(&result);
    collapse_double_spaces(&result).trim().to_string()
}

fn normalize_spoken_dot_spacing(text: &str) -> String {
    let ext = extensions();
    let before = fancy_regex::Regex::new(&format!(r"(?i)\s+\.\s*(?=(?:{ext})\b)")).expect("regex");
    let after = fancy_regex::Regex::new(&format!(r"(?i)(?<=[A-Za-z0-9_-])\.\s+(?=(?:{ext})\b)"))
        .expect("regex");
    let result = replace_all(&before, text, ".");
    replace_all(&after, &result, ".")
}

fn normalize_spoken_slash_for_technical_tokens(text: &str) -> String {
    let mut result = text.to_string();
    for (start, end) in ranges(re!(r"(?i)\bslash\b"), text).into_iter().rev() {
        if is_match(re!(r"(?i)^\s+commands?\b"), &result[end..]) {
            continue;
        }
        result.replace_range(start..end, "/");
    }
    result
}

fn normalize_spaced_filesystem_tokens(text: &str) -> String {
    let pattern = format!(
        r"(?i)\b[a-z0-9_.-]+(?:/[a-z0-9_.-]+(?: [a-z0-9_.-]+)*)+?\.(?:{})\b",
        extensions()
    );
    let re = fancy_regex::Regex::new(&pattern).expect("regex");
    let mut result = text.to_string();
    for (start, end) in ranges(&re, text).into_iter().rev() {
        let normalized = normalize_filesystem_path_candidate(&result[start..end]);
        result.replace_range(start..end, &normalized);
    }
    result
}

fn normalize_filesystem_path_candidate(candidate: &str) -> String {
    let segments: Vec<&str> = candidate.split('/').collect();
    if segments.is_empty() {
        return candidate.to_string();
    }
    let last = segments.len() - 1;
    segments
        .iter()
        .enumerate()
        .map(|(i, segment)| {
            if i == last {
                normalize_filename_segment(segment)
            } else {
                normalize_directory_segment(segment)
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn normalize_directory_segment(segment: &str) -> String {
    segment
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join("")
}

fn normalize_filename_segment(segment: &str) -> String {
    let Some(dot) = segment.rfind('.') else {
        return normalize_directory_segment(segment);
    };
    let stem = &segment[..dot];
    let ext = segment[dot + 1..].to_lowercase();
    let parts: Vec<&str> = stem.split_whitespace().collect();
    if parts.len() <= 1 {
        return format!("{}.{ext}", normalize_directory_segment(stem));
    }
    let normalized = match ext.as_str() {
        "swift" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "rs" | "java" | "kt" | "md" => parts
            .iter()
            .map(|p| capitalize_leading_word(&p.to_lowercase()))
            .collect::<Vec<_>>()
            .join(""),
        _ => parts
            .iter()
            .map(|p| p.to_lowercase())
            .collect::<Vec<_>>()
            .join(""),
    };
    format!("{normalized}.{ext}")
}

/// `quality slash speed` becomes `quality/speed`, never after a stop word
/// and never before `command(s)`.
pub(crate) fn normalize_simple_spoken_slash(text: &str) -> String {
    let re = re!(r"(?i)\b([A-Za-z][A-Za-z0-9+-]*) slash ([A-Za-z][A-Za-z0-9+-]*)\b");
    const LEFT_STOP: &[&str] = &[
        "a", "an", "and", "are", "as", "at", "be", "for", "from", "in", "is", "of", "on", "or",
        "the", "to", "with",
    ];
    let mut result = text.to_string();
    let matches: Vec<(usize, usize, String, String)> = re
        .captures_iter(text)
        .filter_map(Result::ok)
        .filter_map(|c| {
            let whole = c.get(0)?;
            Some((
                whole.start(),
                whole.end(),
                c.get(1)?.as_str().to_string(),
                c.get(2)?.as_str().to_string(),
            ))
        })
        .collect();
    for (start, end, left, right) in matches.into_iter().rev() {
        let left_lower = left.to_lowercase();
        let right_lower = right.to_lowercase();
        if LEFT_STOP.contains(&left_lower.as_str())
            || right_lower == "command"
            || right_lower == "commands"
        {
            continue;
        }
        result.replace_range(start..end, &format!("{left}/{right}"));
    }
    result
}

/// `R and D`, `M and A`, `Q and A` in their ampersand forms.
pub(crate) fn normalize_business_abbreviations(text: &str) -> String {
    let mut result = replace_all(re!(r"(?i)\bR\s*(?:&|and)?\s*D\b"), text, "R&D");
    result = replace_all(re!(r"(?i)\bM\s*(?:&|and)?\s*A\b"), &result, "M&A");
    replace_all(
        re!(r"(?i)\bQ\s*(?:&|and)?\s*A\b(?!\s+session\b)"),
        &result,
        "Q&A",
    )
}

/// `qwen three colon four b` becomes `qwen3:4b`; only known families.
pub(crate) fn normalize_spoken_model_identifiers(text: &str) -> String {
    let re = re!(
        r"(?i)\b([A-Za-z][A-Za-z0-9-]*)\s*(?:(\d+)|\s+(zero|one|two|three|four|five|six|seven|eight|nine)(?:\s+point\s+(zero|one|two|three|four|five|six|seven|eight|nine))?)?\s+colon\s+(zero|one|two|three|four|five|six|seven|eight|nine|\d+)(?:\s+point\s+(zero|one|two|three|four|five|six|seven|eight|nine|\d+))?\s*([bm])\b"
    );
    let captures: Vec<_> = re.captures_iter(text).filter_map(Result::ok).collect();
    if captures.is_empty() {
        return text.to_string();
    }
    let mut result = text.to_string();
    for c in captures.into_iter().rev() {
        let (Some(whole), Some(family), Some(size_major), Some(unit)) =
            (c.get(0), c.get(1), c.get(5), c.get(7))
        else {
            continue;
        };
        let family_token = family.as_str().to_lowercase();
        let (family_name, embedded) = split_embedded_model_version(&family_token);
        if !KNOWN_MODEL_FAMILIES.contains(&family_name.as_str()) {
            continue;
        }
        let version = embedded.unwrap_or_else(|| {
            if let Some(digits) = c.get(2).filter(|m| !m.as_str().is_empty()) {
                return digits.as_str().to_string();
            }
            let major = spoken_digit(c.get(3).map(|m| m.as_str()).unwrap_or(""));
            match c.get(4).filter(|m| !m.as_str().is_empty()) {
                Some(minor) => format!("{major}.{}", spoken_digit(minor.as_str())),
                None => major,
            }
        });
        let major = spoken_digit(size_major.as_str());
        let size = match c.get(6).filter(|m| !m.as_str().is_empty()) {
            Some(minor) => format!("{major}.{}", spoken_digit(minor.as_str())),
            None => major,
        };
        let replacement = format!(
            "{family_name}{version}:{size}{}",
            unit.as_str().to_lowercase()
        );
        result.replace_range(whole.start()..whole.end(), &replacement);
    }
    result
}

fn spoken_digit(token: &str) -> String {
    match token.to_lowercase().as_str() {
        "zero" => "0",
        "one" => "1",
        "two" => "2",
        "three" => "3",
        "four" => "4",
        "five" => "5",
        "six" => "6",
        "seven" => "7",
        "eight" => "8",
        "nine" => "9",
        _ => token,
    }
    .to_string()
}

fn split_embedded_model_version(token: &str) -> (String, Option<String>) {
    match re!(r"^([a-z-]+)(\d+(?:\.\d+)?)$")
        .captures(token)
        .ok()
        .flatten()
    {
        Some(c) => (
            c.get(1).map(|m| m.as_str().to_string()).unwrap_or_default(),
            c.get(2).map(|m| m.as_str().to_string()),
        ),
        None => (token.to_string(), None),
    }
}

/// `1000 plus page` becomes `1,000-plus-page`.
pub(crate) fn normalize_plus_page_phrases(text: &str) -> String {
    let mut result = replace_all(re!(r"(?i)\b1000 plus page\b"), text, "1,000-plus-page");
    result = replace_all(
        re!(r"(?i)\bthousand plus page\b"),
        &result,
        "1,000-plus-page",
    );
    result = replace_all(re!(r"(?i)\b(\d+) plus page\b"), &result, "${1}-plus-page");
    replace_all(re!(r"(?i)\b(\d+) plus pages\b"), &result, "${1}-plus-pages")
}
