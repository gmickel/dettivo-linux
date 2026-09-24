//! Spoken bullets, ordinals and labelled lists become real lists only
//! through the selected NormalizeSpokenLists processor.

use super::text::{capitalize_leading_word, collapse_double_spaces, re};

/// Spoken bullets (`bullet one ...`), ordinals (`first ..., second ...`),
/// dotted ordinals (`one dot ...`) and labelled lists (`roadmap colon ...`)
/// become bullet or numbered lists; anything else comes back unchanged.
pub(crate) fn normalize_spoken_lists(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return text.to_string();
    }
    normalize_bullet_word_list(trimmed)
        .or_else(|| normalize_ordinal_list(trimmed))
        .or_else(|| normalize_dotted_ordinal_list(trimmed))
        .or_else(|| normalize_labeled_list(trimmed))
        .unwrap_or_else(|| text.to_string())
}

fn three_items(pattern: &str, text: &str) -> Option<[String; 3]> {
    let re = fancy_regex::Regex::new(pattern).expect("regex");
    let c = re.captures(text).ok().flatten()?;
    Some([
        cleanup_list_item(c.get(1)?.as_str()),
        cleanup_list_item(c.get(2)?.as_str()),
        cleanup_list_item(c.get(3)?.as_str()),
    ])
}

fn bullets(items: &[String]) -> String {
    items
        .iter()
        .map(|i| format!("- {}", capitalize_leading_word(i)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn numbered(items: &[String]) -> String {
    items
        .iter()
        .enumerate()
        .map(|(i, item)| format!("{}. {}", i + 1, capitalize_leading_word(item)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_bullet_word_list(text: &str) -> Option<String> {
    const PATTERNS: &[&str] = &[
        r"(?i)^\s*bullet\s+(?:1|one)[: ]+\s*(.+?),\s*bullet\s+(?:2|two)[: ]+\s*(.+?),\s*bullet\s+(?:3|three)[: ]+\s*(.+?)[.!?]?\s*$",
        r"(?i)^\s*bullet\s+(?:1|one):\s*(.+?)\.\s*bullet\s+(?:2|two):\s*(.+?)\.\s*bullet\s+(?:3|three):\s*(.+?)[.!?]?\s*$",
        r"(?i)^\s*bullet\s+(?:1|one)[: ]+\s*(.+?)\s+bullet\s+(?:2|two)[: ]+\s*(.+?)\s+bullet\s+(?:3|three)[: ]+\s*(.+?)[.!?]?\s*$",
        r"(?i)^\s*bullet\s+(?:1|one)\s*,?\s*(.+?),\s*bullet\s+(?:2|two)\s*,?\s*(.+?),\s*bullet\s+(?:3|three)\s*,?\s*(.+?)[.!?]?\s*$",
        r"(?i)^\s*bullet\s+one,?\s+(.+?)\.\s*bullet\s+two,?\s+(.+?)\.\s*bullet\s+three,?\s+(.+?)[.!?]?\s*$",
    ];
    PATTERNS
        .iter()
        .find_map(|p| three_items(p, text))
        .map(|items| bullets(&items))
}

fn normalize_ordinal_list(text: &str) -> Option<String> {
    const PATTERNS: &[&str] = &[
        r"(?i)^\s*first\s+(.+?),\s*second\s+(.+?),\s*third\s+(.+?)[.!?]?\s*$",
        r"(?i)^\s*first,?\s+(.+?)\.\s*second,?\s+(.+?)\.\s*third,?\s+(.+?)[.!?]?\s*$",
    ];
    PATTERNS
        .iter()
        .find_map(|p| three_items(p, text))
        .map(|items| numbered(&items))
}

fn normalize_dotted_ordinal_list(text: &str) -> Option<String> {
    let re = re!(
        r"(?i)^\s*one\s+dot\s+(.+?),\s*two\s+dot\s+(.+?)(?:,\s*three\s+dot\s+(.+?))?[.!?]?\s*$"
    );
    let c = re.captures(text).ok().flatten()?;
    let mut items = vec![
        cleanup_list_item(c.get(1)?.as_str()),
        cleanup_list_item(c.get(2)?.as_str()),
    ];
    if let Some(third) = c.get(3).filter(|m| !m.as_str().is_empty()) {
        items.push(cleanup_list_item(third.as_str()));
    }
    Some(numbered(&items))
}

fn normalize_labeled_list(text: &str) -> Option<String> {
    const PREFIXES: &[&str] = &[
        "road map",
        "roadmap",
        "checklist",
        "action items",
        "meeting notes action items",
    ];
    let lowered = text.to_lowercase();
    let prefix = PREFIXES.iter().find(|p| lowered.starts_with(*p))?;
    let mut remainder: &str = &text[prefix.len()..];
    remainder = remainder.trim_start_matches(|c: char| c == ',' || c == ':' || c.is_whitespace());
    let lowered_remainder = remainder.to_lowercase();
    if lowered_remainder.starts_with("colon ") {
        remainder = &remainder["colon ".len()..];
    } else if lowered_remainder.starts_with("colon,") {
        remainder = remainder["colon,".len()..].trim_start();
    }
    let items: Vec<String> = remainder
        .split(',')
        .map(cleanup_list_item)
        .filter(|i| !i.is_empty())
        .collect();
    if items.len() < 3 {
        return None;
    }
    let heading = if *prefix == "road map" {
        "Roadmap".to_string()
    } else {
        capitalize_leading_word(prefix)
    };
    Some(format!("{heading}:\n{}", bullets(&items)))
}

fn is_punctuation(c: char) -> bool {
    c.is_ascii_punctuation() || "…‘’“”«»–—¿¡".contains(c)
}

fn cleanup_list_item(text: &str) -> String {
    let trimmed = text.trim_matches(|c: char| c.is_whitespace() || is_punctuation(c));
    collapse_double_spaces(trimmed)
}
