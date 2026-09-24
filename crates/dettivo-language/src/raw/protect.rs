//! The protected-span scanner (fn-39): the tokens the raw layer never
//! damages. A dot, a slash, an `@` or a `::` that joins word characters
//! with no whitespace around it is part of a token, not a sentence end,
//! so a dictated `index.ts`, `src/app/index.ts`, `v1.2.3`, `foo.bar()`,
//! `std::io`, `gordon@mickel.tech`, `https://example.com/docs` or a
//! backticked span comes back byte for byte. Trailing sentence
//! punctuation stays outside the span: "open index.ts. Then run it" keeps
//! its second dot as the sentence end because whitespace follows it.
//!
//! The scanner works over characters and is a single left-to-right pass,
//! so it is deterministic and linear in the text.

/// A protected span as character indices, `start..end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Index of the first character.
    pub start: usize,
    /// Index one past the last character.
    pub end: usize,
}

impl Span {
    fn overlaps(self, start: usize, end: usize) -> bool {
        self.start < end && start < self.end
    }

    /// Whether `start..end` may be rewritten: it must not cut into this
    /// span. A range that contains the span whole is allowed, so a rule
    /// that names a token in full still applies.
    pub fn allows(self, start: usize, end: usize) -> bool {
        !self.overlaps(start, end) || (start <= self.start && self.end <= end)
    }
}

/// Finds every protected span in `chars`.
pub fn scan(chars: &[char]) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        if chars[i] == '`' {
            if let Some(close) = chars[i + 1..]
                .iter()
                .position(|&c| c == '`')
                .filter(|&n| n > 0)
            {
                let end = i + 1 + close + 1;
                spans.push(Span { start: i, end });
                i = end;
                continue;
            }
        }
        let run_end = chars[i..]
            .iter()
            .position(|c| c.is_whitespace())
            .map_or(chars.len(), |n| i + n);
        if let Some(span) = classify(chars, i, run_end) {
            spans.push(span);
        }
        i = run_end;
    }
    spans
}

/// The protected part of the whitespace-free run `start..end`, if any.
fn classify(chars: &[char], start: usize, end: usize) -> Option<Span> {
    let start = start + leading_wrappers(&chars[start..end]);
    let end = end - trailing_punctuation(&chars[start..end]);
    if end <= start {
        return None;
    }
    let core = &chars[start..end];
    let protected = is_url(core) || is_email(core) || has_joined_separator(core);
    protected.then_some(Span { start, end })
}

/// Opening brackets and quotes before the token stay outside it.
fn leading_wrappers(core: &[char]) -> usize {
    core.iter()
        .take_while(|c| matches!(c, '(' | '[' | '{' | '"' | '\'' | '\u{201c}' | '\u{2018}'))
        .count()
}

/// Sentence punctuation and closing quotes after the token stay outside
/// it; a closing bracket is stripped only when the token did not open it.
fn trailing_punctuation(core: &[char]) -> usize {
    let mut n = 0;
    let mut opened: i64 = core.iter().filter(|&&c| c == '(').count() as i64
        - core.iter().filter(|&&c| c == ')').count() as i64;
    for &c in core.iter().rev() {
        match c {
            '.' | ',' | ';' | ':' | '!' | '?' | '"' | '\'' | '\u{201d}' | '\u{2019}' => n += 1,
            ')' | ']' | '}' if opened < 0 => {
                opened += 1;
                n += 1;
            }
            _ => break,
        }
    }
    n
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '~' | '+' | '#')
}

/// `scheme://...` or `www.` followed by anything.
fn is_url(core: &[char]) -> bool {
    if core.len() > 4
        && core[..4]
            .iter()
            .collect::<String>()
            .eq_ignore_ascii_case("www.")
    {
        return true;
    }
    let Some(colon) = core.iter().position(|&c| c == ':') else {
        return false;
    };
    colon > 0
        && core[..colon].iter().all(|c| c.is_ascii_alphabetic())
        && core.get(colon + 1) == Some(&'/')
        && core.get(colon + 2) == Some(&'/')
        && core.len() > colon + 3
}

/// `local@domain.tld`: word characters, one `@`, a dotted domain.
fn is_email(core: &[char]) -> bool {
    let Some(at) = core.iter().position(|&c| c == '@') else {
        return false;
    };
    let local = &core[..at];
    let domain = &core[at + 1..];
    !local.is_empty()
        && local.iter().all(|&c| is_word(c) || c == '.')
        && domain.len() >= 3
        && domain.iter().all(|&c| is_word(c) || c == '.')
        && domain.contains(&'.')
        && is_word(domain[0])
        && is_word(domain[domain.len() - 1])
}

/// A dot, a slash or a `::` with a word character on both sides: file
/// names, paths, versions, dotted identifiers and `foo.bar()` calls. A
/// leading `/`, `~/` or `./` also marks a path.
fn has_joined_separator(core: &[char]) -> bool {
    if core.len() >= 2
        && (core[0] == '/' || (matches!(core[0], '~' | '.') && core[1] == '/'))
        && core.iter().any(|&c| c.is_alphanumeric())
    {
        return true;
    }
    core.windows(3).any(|w| {
        (matches!(w[1], '.' | '/') && is_word(w[0]) && is_word(w[2]))
            || (w[1] == ':' && w[0] == ':' && is_word(w[2]))
    }) && core.iter().any(|&c| c.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans_of(text: &str) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        scan(&chars)
            .into_iter()
            .map(|s| chars[s.start..s.end].iter().collect())
            .collect()
    }

    #[test]
    fn every_token_class_is_found() {
        let cases: &[(&str, &[&str])] = &[
            ("open index.ts now", &["index.ts"]),
            (
                "see src/app/index.ts and /home/gordon",
                &["src/app/index.ts", "/home/gordon"],
            ),
            ("paths ~/work and ./run.sh", &["~/work", "./run.sh"]),
            (
                "visit https://example.com/docs?q=1 or www.example.com",
                &["https://example.com/docs?q=1", "www.example.com"],
            ),
            ("mail gordon@mickel.tech today", &["gordon@mickel.tech"]),
            ("release v1.2.3 and 1.2", &["v1.2.3", "1.2"]),
            (
                "call foo.bar() and std::io::Error",
                &["foo.bar()", "std::io::Error"],
            ),
            (
                "run `cargo build --release` now",
                &["`cargo build --release`"],
            ),
            ("die straße.txt datei", &["straße.txt"]),
        ];
        for (text, want) in cases {
            assert_eq!(spans_of(text), *want, "{text}");
        }
    }

    #[test]
    fn trailing_punctuation_and_wrappers_stay_outside() {
        assert_eq!(spans_of("open index.ts. then"), vec!["index.ts"]);
        assert_eq!(spans_of("see example.com."), vec!["example.com"]);
        assert_eq!(spans_of("(see index.ts), ok"), vec!["index.ts"]);
        assert_eq!(spans_of("\u{201c}foo.bar()\u{201d}"), vec!["foo.bar()"]);
        assert_eq!(spans_of("or bar.baz(x)?"), vec!["bar.baz(x)"]);
    }

    #[test]
    fn plain_words_and_lone_marks_are_not_tokens() {
        for text in [
            "hello world.",
            "wait... what",
            "a . b",
            "at @ sign",
            "3 / 4",
            "etc.",
            "``",
        ] {
            assert!(spans_of(text).is_empty(), "{text}");
        }
    }

    #[test]
    fn a_rule_may_cover_a_span_whole_but_never_cut_into_it() {
        let span = Span { start: 4, end: 12 };
        assert!(span.allows(0, 3));
        assert!(span.allows(12, 15));
        assert!(span.allows(4, 12));
        assert!(span.allows(4, 13));
        assert!(!span.allows(10, 12));
        assert!(!span.allows(2, 6));
    }
}
