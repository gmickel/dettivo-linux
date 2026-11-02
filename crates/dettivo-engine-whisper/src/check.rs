//! The vocabulary check (ADR 0095): a listed term in Whisper's output has
//! to earn its place, because the vocabulary reaches Whisper as a decoder
//! prompt and the model may write a term nobody said. Only spans that
//! spell a listed term are looked at, so a take without one is untouched.
//!
//! A term span is a suspect when it is echo-shaped (a segment, or the
//! sentences closing one, made of listed terms alone) or when the mean log
//! probability of its tokens is under [`MIN_MEAN_LOGPROB`]. Suspects are
//! settled against the same take decoded without the prompt: a span that
//! decode also spells stays, and so does a lone name (one term that is a
//! sentence of its own) when that decode heard something that sounds like it or
//! one of its aliases ([`MIN_SOUND_ALIKE`]). Any other gives way to what
//! that decode heard in its place, which is nothing when it heard nothing
//! there.

use std::time::Instant;

use dettivo_engine_proto::sound::{alike, normalise};
use dettivo_engine_proto::{BiasTerm, EngineError};

use crate::words::{Spoken, Token};

/// The mean token log probability a listed term needs to stay without the
/// prompt-free decode agreeing (ADR 0095 has the measurement).
pub const MIN_MEAN_LOGPROB: f32 = -0.3;

/// How alike ([`alike`]) the prompt-free decode's words must sound to an
/// unsure lone name, or one of its aliases, for the name to stay (ADR 0095
/// has the measurement).
pub const MIN_SOUND_ALIKE: f32 = 0.6;

/// The most words one term span covers ("Sapien X T" is three).
const MAX_SPAN_WORDS: usize = 4;

/// One whitespace-separated word of a segment.
#[derive(Debug, Clone)]
struct Word {
    seg: usize,
    /// Byte range in the segment's text.
    start: usize,
    end: usize,
    norm: String,
    /// Ends in `.`, `!` or `?`.
    closes: bool,
}

/// A run of words that spells a listed term: first and last word,
/// inclusive, counted over the whole take, and the term's index.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Span {
    first: usize,
    last: usize,
    term: usize,
}

/// A span the prompt-free decode settles, and whether it is a lone name.
#[derive(Debug, Clone, Copy)]
struct Suspect {
    span: Span,
    lone: bool,
}

/// What the check found in the prompted decode.
#[derive(Debug)]
pub struct Finding {
    texts: Vec<String>,
    words: Vec<Word>,
    spans: Vec<Span>,
    suspects: Vec<Suspect>,
    /// Each term's spellings, normalised: the term, then its aliases.
    spellings: Vec<Vec<String>>,
}

impl Finding {
    /// Term spans in the take.
    pub fn candidates(&self) -> usize {
        self.spans.len()
    }

    /// True when a span needs the prompt-free decode to settle it.
    pub fn needs_plain_decode(&self) -> bool {
        !self.suspects.is_empty()
    }
}

/// The segment texts of a prompted decode once its listed `terms` are
/// checked; `None` when they stand as decoded. `plain` decodes the take
/// without the prompt and runs only when a suspect needs it. Logs the
/// counts and the time the check added, never a word.
pub fn run(
    texts: &[String],
    spoken: &[Spoken],
    terms: &[BiasTerm],
    plain: impl FnOnce() -> Result<Vec<String>, EngineError>,
) -> Result<Option<Vec<String>>, EngineError> {
    let started = Instant::now();
    let tokens: Vec<Vec<Token>> = spoken.iter().map(|s| s.tokens.clone()).collect();
    let finding = find(texts, &tokens, terms);
    let revised = if finding.needs_plain_decode() {
        settle(&finding, &plain()?)
    } else {
        None
    };
    let replaced = revised.as_ref().map_or(0, |r| {
        r.iter()
            .zip(texts)
            .filter(|(new, old)| new.as_str() != old.trim())
            .count()
    });
    let check_ms = (started.elapsed().as_secs_f64() * 10_000.0).round() / 10.0;
    tracing::info!(
        candidates = finding.candidates(),
        suspects = finding.suspects.len(),
        replaced,
        check_ms,
        "vocabulary check"
    );
    Ok(revised)
}

/// The term spans and the suspects among them in a decode whose segments
/// have `texts` (as whisper.cpp returned them, untrimmed) and `tokens`.
pub fn find(texts: &[String], tokens: &[Vec<Token>], terms: &[BiasTerm]) -> Finding {
    let spellings: Vec<Vec<String>> = terms
        .iter()
        .map(|t| {
            // The term keeps the first place even when it spells nothing,
            // so an alias never stands in for it as a span.
            std::iter::once(normalise(&t.term))
                .chain(
                    t.aliases
                        .iter()
                        .map(|a| normalise(a))
                        .filter(|a| !a.is_empty()),
                )
                .collect()
        })
        .collect();
    let words = split(texts);
    let spans = spans(&words, &spellings);
    let suspects = spans
        .iter()
        .copied()
        .filter_map(|span| {
            let run = alone(span, &words, &spans);
            let echo = run >= 2;
            let mean_logprob = confidence(span, &words, texts, tokens);
            tracing::debug!(
                first_word = span.first,
                echo,
                mean_logprob,
                "vocabulary candidate"
            );
            (echo || mean_logprob < MIN_MEAN_LOGPROB).then_some(Suspect {
                span,
                lone: !echo && own_sentence(span, &words),
            })
        })
        .collect();
    Finding {
        texts: texts.to_vec(),
        words,
        spans,
        suspects,
        spellings,
    }
}

fn split(texts: &[String]) -> Vec<Word> {
    let mut words = Vec::new();
    for (seg, text) in texts.iter().enumerate() {
        for w in text.split_whitespace() {
            let start = w.as_ptr() as usize - text.as_ptr() as usize;
            words.push(Word {
                seg,
                start,
                end: start + w.len(),
                norm: normalise(w),
                closes: w.ends_with(['.', '!', '?']),
            });
        }
    }
    words
}

/// The term spans in `words`, left to right, none overlapping: one to
/// [`MAX_SPAN_WORDS`] words of one segment whose letters spell a term, or
/// the term with `s` (a plural, or a possessive once the apostrophe goes).
/// Only the term itself (the first of its `spellings`) is looked for. The
/// longest run wins.
fn spans(words: &[Word], spellings: &[Vec<String>]) -> Vec<Span> {
    let mut found = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let mut hit = None;
        let mut spelt = String::new();
        for last in i..words.len().min(i + MAX_SPAN_WORDS) {
            if words[last].seg != words[i].seg {
                break;
            }
            spelt.push_str(&words[last].norm);
            let spells = |t: &String| spelt == *t || spelt.strip_suffix('s') == Some(t.as_str());
            let term = spellings
                .iter()
                .position(|s| s.first().is_some_and(|t| !t.is_empty() && spells(t)));
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

/// The mean log probability of the tokens under the span's letters; minus
/// infinity when the tokens do not spell the segment's text.
fn confidence(span: Span, words: &[Word], texts: &[String], tokens: &[Vec<Token>]) -> f32 {
    let (first, last) = (&words[span.first], &words[span.last]);
    let text = &texts[first.seg];
    let tokens = &tokens[first.seg];
    let spelt: Vec<u8> = tokens
        .iter()
        .flat_map(|t| t.bytes.iter().copied())
        .collect();
    if spelt != text.as_bytes() {
        return f32::NEG_INFINITY;
    }
    let from = first.start
        + text[first.start..first.end]
            .find(char::is_alphanumeric)
            .unwrap_or(0);
    let to = last.start
        + text[last.start..last.end]
            .char_indices()
            .rfind(|(_, c)| c.is_alphanumeric())
            .map_or(last.end - last.start, |(i, c)| i + c.len_utf8());
    let (mut at, mut sum, mut n) = (0, 0.0, 0);
    for t in tokens {
        let (start, end) = (at, at + t.bytes.len());
        at = end;
        if start < to && end > from {
            sum += t.plog;
            n += 1;
        }
    }
    if n == 0 {
        f32::NEG_INFINITY
    } else {
        sum / n as f32
    }
}

/// How many term spans make up the run the span sits in when that run
/// closes its segment and opens it or follows a sentence's end; 0 when it
/// does not. A run of two or more is echo-shaped. One term alone is a
/// name said on its own as often as an echo, so its confidence decides.
fn alone(span: Span, words: &[Word], spans: &[Span]) -> usize {
    let in_span = |w: usize| spans.iter().any(|s| s.first <= w && w <= s.last);
    let seg = words[span.first].seg;
    let same = |w: &Word| w.seg == seg;
    let closes_segment = words[span.last + 1..]
        .iter()
        .take_while(|w| same(w))
        .enumerate()
        .all(|(k, _)| in_span(span.last + 1 + k));
    let mut start = span.first;
    while start > 0 && same(&words[start - 1]) && in_span(start - 1) {
        start -= 1;
    }
    let opens = start == 0 || !same(&words[start - 1]) || words[start - 1].closes;
    let run = spans
        .iter()
        .filter(|s| s.first >= start && words[s.first].seg == seg)
        .count();
    if closes_segment && opens { run } else { 0 }
}

/// True when the span is a sentence of its own: it opens the take or
/// follows a word ending in `.`, `!` or `?`, and it ends the take or ends
/// in one itself. Segment breaks do not count, because Whisper splits
/// segments at timestamps, not at sentence ends. Only such a lone name
/// may stay by sound.
fn own_sentence(span: Span, words: &[Word]) -> bool {
    (span.first == 0 || words[span.first - 1].closes)
        && (span.last + 1 == words.len() || words[span.last].closes)
}

/// The longest common subsequence of `a` and `b`, as matched index pairs.
fn lcs(a: &[&str], b: &[&str]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    let mut len = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            len[i][j] = if a[i] == b[j] {
                len[i + 1][j + 1] + 1
            } else {
                len[i + 1][j].max(len[i][j + 1])
            };
        }
    }
    let (mut i, mut j, mut pairs) = (0, 0, Vec::new());
    while i < n && j < m {
        if a[i] == b[j] {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if len[i + 1][j] >= len[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

/// The segment texts once every suspect is settled against `plain`, the
/// segment texts of the prompt-free decode; `None` when every suspect
/// stands. Words both decodes share anchor the two; a suspect's gap
/// between anchors becomes the prompt-free decode's words in that gap,
/// unless both gaps spell the same letters, or the suspect is a lone name
/// and their gap sounds like it. Texts come back trimmed, and a segment
/// left with nothing is empty.
pub fn settle(finding: &Finding, plain: &[String]) -> Option<Vec<String>> {
    let words = &finding.words;
    let theirs: Vec<&str> = plain.iter().flat_map(|t| t.split_whitespace()).collect();
    let their_norms: Vec<String> = theirs.iter().map(|w| normalise(w)).collect();
    let ours: Vec<&str> = words.iter().map(|w| w.norm.as_str()).collect();
    let their_refs: Vec<&str> = their_norms.iter().map(String::as_str).collect();
    let mut anchor = vec![None; words.len()];
    for (i, j) in lcs(&ours, &their_refs) {
        anchor[i] = Some(j);
    }
    // (our gap, their gap), each a half-open word range.
    let mut gaps: Vec<((usize, usize), (usize, usize))> = Vec::new();
    for &Suspect { span: s, lone } in &finding.suspects {
        if (s.first..=s.last).all(|w| anchor[w].is_some()) {
            continue;
        }
        let before = (0..s.first).rev().find_map(|w| anchor[w].map(|j| (w, j)));
        let after = (s.last + 1..words.len()).find_map(|w| anchor[w].map(|j| (w, j)));
        let ours = (
            before.map_or(0, |b| b.0 + 1),
            after.map_or(words.len(), |a| a.0),
        );
        let theirs = (
            before.map_or(0, |b| b.1 + 1),
            after.map_or(theirs.len(), |a| a.1),
        );
        let our_letters: String = words[ours.0..ours.1]
            .iter()
            .map(|w| w.norm.as_str())
            .collect();
        let their_letters: String = their_norms[theirs.0..theirs.1].concat();
        if our_letters == their_letters {
            continue;
        }
        if lone {
            let sound = finding.spellings[s.term]
                .iter()
                .map(|spelling| alike(&their_letters, spelling))
                .fold(0.0, f32::max);
            tracing::debug!(first_word = s.first, sound, "vocabulary lone name");
            if sound >= MIN_SOUND_ALIKE {
                continue;
            }
        }
        gaps.push((ours, theirs));
    }
    if gaps.is_empty() {
        return None;
    }
    // A word inside a multi-word term can anchor, so two suspects' gaps can
    // overlap; anchors keep both decodes in order, so the union of the two
    // is one gap on each side.
    gaps.sort_by_key(|g| g.0);
    let mut merged: Vec<((usize, usize), (usize, usize))> = Vec::new();
    for (ours, their) in gaps {
        match merged.last_mut() {
            Some((o, t)) if ours.0 < o.1 => {
                o.1 = o.1.max(ours.1);
                *t = (t.0.min(their.0), t.1.max(their.1));
            }
            _ => merged.push((ours, their)),
        }
    }
    let gaps = merged;
    // Byte edits per segment: (start, end, replacement).
    let mut edits: Vec<Vec<(usize, usize, String)>> = vec![Vec::new(); finding.texts.len()];
    for (ours, their) in gaps {
        let mut replacement = Some(theirs[their.0..their.1].join(" "));
        let mut w = ours.0;
        while w < ours.1 {
            let seg = words[w].seg;
            let mut last = w;
            while last + 1 < ours.1 && words[last + 1].seg == seg {
                last += 1;
            }
            let text = replacement.take().unwrap_or_default();
            edits[seg].push((words[w].start, words[last].end, text));
            w = last + 1;
        }
    }
    let texts = finding
        .texts
        .iter()
        .zip(edits)
        .map(|(text, mut edits)| {
            let mut text = text.clone();
            edits.sort_by_key(|a| std::cmp::Reverse(a.0));
            for (start, end, with) in edits {
                text.replace_range(start..end, &with);
            }
            text.split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .collect();
    Some(texts)
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
