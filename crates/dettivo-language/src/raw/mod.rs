//! The raw layer (FR-M1): replacement pairs, spoken punctuation and
//! whitespace discipline applied to what the engine returned. Every mode
//! starts here; `deterministic_polish` and `enhanced` build on its output.
//!
//! With `protect_tokens` on, the `protect` scanner marks file names,
//! paths, URLs, email addresses, versions, dotted identifiers and
//! backticked spans before each pass: a replacement never cuts into one,
//! the sentence-end rule never fires on a dot inside one, and every
//! span comes back byte for byte.

mod protect;
#[cfg(test)]
mod tests;

pub use protect::{Span, scan};

/// Spoken punctuation phrases and what they become, longest first.
const SPOKEN: &[(&str, &str)] = &[
    ("new paragraph", "\n\n"),
    ("new line", "\n"),
    ("newline", "\n"),
    ("question mark", "?"),
    ("exclamation mark", "!"),
    ("exclamation point", "!"),
    ("full stop", "."),
    ("period", "."),
    ("comma", ","),
    ("colon", ":"),
    ("semicolon", ";"),
    ("open quote", "\u{201c}"),
    ("close quote", "\u{201d}"),
    ("open paren", "("),
    ("close paren", ")"),
    ("dash", " - "),
    ("ellipsis", "..."),
];

/// The first private-use code point; a hidden span stands in the text as
/// one of these while the passes run.
const HIDDEN_BASE: u32 = 0xE000;
/// How many spans can be hidden at once (the private-use block).
const HIDDEN_LIMIT: u32 = 0x1900;

/// Applies the raw pipeline.
pub fn apply(
    text: &str,
    replacements: &[(String, String)],
    spoken_punctuation: bool,
    protect_tokens: bool,
) -> String {
    let mut out = text.trim().to_string();
    for (from, to) in replacements {
        if from.is_empty() {
            continue;
        }
        let chars: Vec<char> = out.chars().collect();
        let spans = if protect_tokens {
            scan(&chars)
        } else {
            Vec::new()
        };
        out = replace_word(&chars, from, to, &spans);
    }
    // A spoken model identifier (`qwen three colon 4b`) is a token, not a
    // colon in prose: the fixed pass every rewrite gets reads it before
    // the spoken marks are consumed, so the later stages see `qwen3:4b`.
    if spoken_punctuation {
        out = crate::polish::normalize_spoken_model_identifiers(&out);
    }
    let hidden = if protect_tokens {
        hide(&out)
    } else {
        Hidden::none(out)
    };
    let mut out = hidden.text.clone();
    if spoken_punctuation {
        out = punctuate(&out);
    }
    hidden.restore(&tidy(&out))
}

/// Text with its protected spans swapped for private-use placeholders.
/// The Polish pass hides the same spans, so its prose rules never reach
/// inside a token the raw layer protected.
pub(crate) struct Hidden {
    /// The text with a placeholder where each span stood.
    pub(crate) text: String,
    spans: Vec<String>,
}

impl Hidden {
    pub(crate) fn none(text: String) -> Self {
        Self {
            text,
            spans: Vec::new(),
        }
    }

    /// Puts the spans back where their placeholders stand.
    pub(crate) fn restore(&self, text: &str) -> String {
        if self.spans.is_empty() {
            return text.to_string();
        }
        text.chars()
            .fold(String::with_capacity(text.len()), |mut acc, c| {
                match placeholder_index(c) {
                    Some(n) if n < self.spans.len() => acc.push_str(&self.spans[n]),
                    _ => acc.push(c),
                }
                acc
            })
    }
}

fn placeholder_index(c: char) -> Option<usize> {
    let code = c as u32;
    (HIDDEN_BASE..HIDDEN_BASE + HIDDEN_LIMIT)
        .contains(&code)
        .then(|| (code - HIDDEN_BASE) as usize)
}

/// Hides every protected span behind a placeholder. Text that already
/// carries private-use characters is left as it is, so a restore can
/// never confuse them with a placeholder.
pub(crate) fn hide(text: &str) -> Hidden {
    let chars: Vec<char> = text.chars().collect();
    if chars.iter().any(|&c| placeholder_index(c).is_some()) {
        return Hidden::none(text.to_string());
    }
    let spans = scan(&chars);
    let mut out = String::with_capacity(text.len());
    let mut hidden = Vec::with_capacity(spans.len());
    let mut cursor = 0;
    for span in spans.iter().take(HIDDEN_LIMIT as usize) {
        out.extend(&chars[cursor..span.start]);
        let placeholder = char::from_u32(HIDDEN_BASE + hidden.len() as u32).expect("private use");
        out.push(placeholder);
        hidden.push(chars[span.start..span.end].iter().collect());
        cursor = span.end;
    }
    out.extend(&chars[cursor..]);
    Hidden {
        text: out,
        spans: hidden,
    }
}

/// Replaces whole-word, case-insensitive occurrences of `from` outside
/// the protected `spans` (a match that covers a span whole still counts).
/// Matching runs over characters (each lowered to its first lowercase
/// form) so a non-ASCII text never lands on a byte that is not a boundary.
fn replace_word(chars: &[char], from: &str, to: &str, spans: &[Span]) -> String {
    let lower: Vec<char> = chars
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let needle: Vec<char> = from
        .chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect();
    if needle.is_empty() {
        return chars.iter().collect();
    }
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let end = i + needle.len();
        if end <= lower.len()
            && lower[i..end] == needle[..]
            && at_boundary(chars, i)
            && at_boundary(chars, end)
            && spans.iter().all(|s| s.allows(i, end))
        {
            out.push_str(to);
            i = end;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn at_boundary(chars: &[char], i: usize) -> bool {
    if i == 0 || i >= chars.len() {
        return true;
    }
    !(chars[i - 1].is_alphanumeric() && chars[i].is_alphanumeric())
}

/// Words after which a punctuation word is a noun ("a period of three
/// weeks", "add a dash of salt"): articles and determiners that cannot
/// end a sentence, so the word cannot be a spoken mark closing one.
const DETERMINERS: &[&str] = &[
    "the", "a", "an", "each", "every", "my", "your", "our", "their", "its",
];

/// The start of the word before `at` when it is one of [`DETERMINERS`].
fn determiner_before(chars: &[char], at: usize) -> Option<usize> {
    let end = chars[..at].iter().rposition(|c| !c.is_whitespace())? + 1;
    let start = chars[..end]
        .iter()
        .rposition(|c| !c.is_alphanumeric())
        .map_or(0, |i| i + 1);
    let word: String = chars[start..end]
        .iter()
        .flat_map(|c| c.to_lowercase())
        .collect();
    DETERMINERS.contains(&word.as_str()).then_some(start)
}

/// The whole-word, case-insensitive occurrences of `needle` in `chars`.
fn occurrences(chars: &[char], needle: &str) -> Vec<Span> {
    let lower: Vec<char> = chars
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let needle: Vec<char> = needle.chars().collect();
    let mut spans = Vec::new();
    let mut i = 0;
    while i + needle.len() <= lower.len() {
        let end = i + needle.len();
        if lower[i..end] == needle[..] && at_boundary(chars, i) && at_boundary(chars, end) {
            spans.push(Span { start: i, end });
            i = end;
        } else {
            i += 1;
        }
    }
    spans
}

/// Turns spoken punctuation into marks and glues them to the word before.
/// A phrase after a determiner stays a word, and a spoken `dash dash` is a
/// flag (`--model`), which the code preset's technical pass owns, never
/// two marks of prose punctuation.
fn punctuate(text: &str) -> String {
    let mut out = text.to_string();
    for (phrase, mark) in SPOKEN {
        let chars: Vec<char> = out.chars().collect();
        let mut keep: Vec<Span> = occurrences(&chars, phrase)
            .into_iter()
            .filter_map(|s| {
                determiner_before(&chars, s.start).map(|start| Span { start, end: s.end })
            })
            .collect();
        if *phrase == "dash" {
            keep.extend(occurrences(&chars, "dash dash"));
        }
        let replaced = replace_word(&chars, phrase, mark, &keep);
        if replaced != out {
            out = replaced;
        }
    }
    let mut result = String::with_capacity(out.len());
    let chars: Vec<char> = out.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if matches!(c, '.' | ',' | '?' | '!' | ':' | ';' | ')' | '\u{201d}') {
            while result.ends_with(' ') {
                result.pop();
            }
            // A spoken stop beside the engine's own ("report period."):
            // the sentence ends once, over a comma before it, and a run
            // of exactly two dots is one; an ellipsis keeps three.
            let ends = matches!(c, '.' | '?' | '!');
            if ends && result.ends_with([',', ';', ':']) {
                result.pop();
            }
            let doubled = c == '.'
                && result.ends_with(['.', '?', '!'])
                && !result.ends_with("..")
                && chars.get(i + 1) != Some(&'.');
            if doubled {
                i += 1;
                continue;
            }
            result.push(c);
            // Capitalise after a sentence end.
            if matches!(c, '.' | '?' | '!') {
                let mut j = i + 1;
                while j < chars.len() && chars[j] == ' ' {
                    j += 1;
                }
                if j < chars.len() && chars[j].is_lowercase() {
                    result.push(' ');
                    result.extend(chars[j].to_uppercase());
                    i = j + 1;
                    continue;
                }
            }
        } else if c == '(' || c == '\u{201c}' {
            result.push(c);
            let mut j = i + 1;
            while j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            i = j;
            continue;
        } else {
            result.push(c);
        }
        i += 1;
    }
    result
}

/// Collapses runs of spaces, trims line ends and capitalises the start.
fn tidy(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in text.split('\n') {
        let words: Vec<&str> = line.split(' ').filter(|w| !w.is_empty()).collect();
        lines.push(words.join(" "));
    }
    let mut out = lines.join("\n");
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    let mut chars = out.chars();
    match chars.next() {
        Some(first) if first.is_lowercase() => first.to_uppercase().chain(chars).collect(),
        _ => out,
    }
}
