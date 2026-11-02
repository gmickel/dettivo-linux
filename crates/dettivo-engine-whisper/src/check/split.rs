//! The words of a decode and the runs of them that spell a listed term.
//! Words split at whitespace and, in scripts written without spaces
//! (Chinese, Japanese, Thai, Lao, Khmer, Burmese), at every character, so
//! `纽约` is found in `我在纽约工作`. Words compare by their marked
//! spelling ([`marked`]), so `C++` is never `C`, and a run of words never
//! crosses a mark between them ([`ends_run`]) unless the term is spelt
//! with it.

use dettivo_engine_proto::sound::{ends_run, ends_sentence, is_term_mark, marked};

/// The most words one term span covers ("Sapien X T" is three), unless a
/// term written without spaces has more characters.
const MAX_SPAN_WORDS: usize = 4;

/// One word of a segment.
#[derive(Debug, Clone)]
pub(super) struct Word {
    pub seg: usize,
    /// Byte range in the segment's text.
    pub start: usize,
    pub end: usize,
    /// The marked spelling.
    pub norm: String,
    /// Ends a sentence ([`ends_sentence`]).
    pub closes: bool,
    /// Ends in a mark ([`ends_run`]).
    pub breaks: bool,
}

/// A run of words that spells a listed term: first and last word,
/// inclusive, counted over the whole take, and the term's index.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Span {
    pub first: usize,
    pub last: usize,
    pub term: usize,
}

/// A character of a script written without spaces between words.
fn unspaced(c: char) -> bool {
    matches!(c as u32,
        0x0E00..=0x0EFF // Thai, Lao
        | 0x1000..=0x109F // Myanmar
        | 0x1780..=0x17FF // Khmer
        | 0x3040..=0x30FF // Hiragana, Katakana
        | 0x31F0..=0x31FF
        | 0x3400..=0x4DBF // CJK ideographs
        | 0x4E00..=0x9FFF
        | 0xF900..=0xFAFF
        | 0xFF66..=0xFF9D // halfwidth Katakana
        | 0x20000..=0x2FA1F)
}

/// Byte ranges of the words of `text`: whitespace splits them, and so
/// does each character of a script without spaces. Marks stay with the
/// word they follow (`作。`), and opening marks with the one they open.
fn pieces(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for chunk in text.split_whitespace() {
        let base = chunk.as_ptr() as usize - text.as_ptr() as usize;
        let mut from = 0;
        let mut has_letter = false;
        let mut prev_unspaced = false;
        for (i, c) in chunk.char_indices() {
            let cut = (unspaced(c) && has_letter) || (prev_unspaced && c.is_alphanumeric());
            if cut && i > from {
                out.push((base + from, base + i));
                from = i;
                has_letter = false;
            }
            has_letter |= c.is_alphanumeric();
            prev_unspaced = unspaced(c);
        }
        out.push((base + from, base + chunk.len()));
    }
    out
}

/// The words of every segment of `texts`, in order.
pub(super) fn split(texts: &[String]) -> Vec<Word> {
    let mut words = Vec::new();
    for (seg, text) in texts.iter().enumerate() {
        for (start, end) in pieces(text) {
            let w = &text[start..end];
            words.push(Word {
                seg,
                start,
                end,
                norm: marked(w),
                closes: ends_sentence(w),
                breaks: ends_run(w),
            });
        }
    }
    words
}

/// How a span compares with a term: its spelling (the term, then its
/// aliases, marked), the term as written without spaces (lowercase, its
/// own closing marks off), and how many words it splits into.
pub(super) struct Spelling {
    pub spellings: Vec<String>,
    written: String,
    words: usize,
}

impl Spelling {
    pub fn new(term: &str, aliases: &[String]) -> Self {
        // The term keeps the first place even when it spells nothing,
        // so an alias never stands in for it as a span.
        let spellings = std::iter::once(marked(term))
            .chain(aliases.iter().map(|a| marked(a)).filter(|a| !a.is_empty()))
            .collect();
        Self {
            spellings,
            written: written(term),
            words: pieces(term).len(),
        }
    }
}

/// Lowercase, without whitespace, without the marks that close it.
fn written(text: &str) -> String {
    let joined: String = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    joined
        .trim_end_matches(|c: char| !c.is_alphanumeric() && !is_term_mark(c))
        .to_string()
}

/// The term spans in `words` of `texts`, left to right, none overlapping:
/// one to [`MAX_SPAN_WORDS`] words of one segment whose marked letters
/// spell a term, or the term with `s` (a plural, or a possessive once the
/// apostrophe goes). Only the term itself (the first of its spellings) is
/// looked for, and a run stops at a word ending in a mark unless the term
/// is written with it. The longest run wins.
pub(super) fn spans(words: &[Word], texts: &[String], terms: &[Spelling]) -> Vec<Span> {
    let longest = terms
        .iter()
        .map(|t| t.words)
        .fold(MAX_SPAN_WORDS, usize::max);
    let mut found = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let mut hit = None;
        let mut spelt = String::new();
        for last in i..words.len().min(i + longest) {
            if words[last].seg != words[i].seg {
                break;
            }
            spelt.push_str(&words[last].norm);
            let spells = |t: &String| spelt == *t || spelt.strip_suffix('s') == Some(t.as_str());
            let crosses = words[i..last].iter().any(|w| w.breaks);
            let as_written = || written(&texts[words[i].seg][words[i].start..words[last].end]);
            let term = terms.iter().position(|t| {
                t.spellings
                    .first()
                    .is_some_and(|s| !s.is_empty() && spells(s))
                    && (!crosses || as_written() == t.written)
            });
            if let (false, Some(term)) = (words[i].norm.is_empty(), term) {
                hit = Some((last, term));
            }
        }
        match hit {
            Some((last, term)) => {
                found.push(Span {
                    first: i,
                    last,
                    term,
                });
                i = last + 1;
            }
            None => i += 1,
        }
    }
    found
}
