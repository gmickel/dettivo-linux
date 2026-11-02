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
//!
//! Words are read as [`split()`] reads them: at spaces and, in scripts
//! written without them, at every character; with the marks a term is
//! spelt with (`C++`); and never across punctuation between words. A
//! suspect stands against the prompt-free decode only when that decode
//! heard its words together and in the same place, and only the settled
//! gap's bytes change.

use std::time::Instant;

use dettivo_engine_proto::{BiasTerm, EngineError};

use crate::words::{Spoken, Token};

mod settle;
mod split;

pub use settle::settle;
use split::{Span, Spelling, Word, split};

/// The mean token log probability a listed term needs to stay without the
/// prompt-free decode agreeing (ADR 0095 has the measurement).
pub const MIN_MEAN_LOGPROB: f32 = -0.3;

/// How alike ([`alike`](dettivo_engine_proto::sound::alike)) the prompt-free decode's words must sound to an
/// unsure lone name, or one of its aliases, for the name to stay (ADR 0095
/// has the measurement).
pub const MIN_SOUND_ALIKE: f32 = 0.6;

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
    let terms: Vec<Spelling> = terms
        .iter()
        .map(|t| Spelling::new(&t.term, &t.aliases))
        .collect();
    let words = split(texts);
    let spans = split::spans(&words, texts, &terms);
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
        spellings: terms.into_iter().map(|t| t.spellings).collect(),
    }
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
/// does not. The run is the longest stretch of term words before the end
/// of the segment that starts a sentence, so an echo after a listed name
/// ending its own sentence ("Book Hyprland. Omarchy, Clef.") is a run of
/// its own. A run of two or more is echo-shaped. One term alone is a name
/// said on its own as often as an echo, so its confidence decides.
fn alone(span: Span, words: &[Word], spans: &[Span]) -> usize {
    let in_span = |w: usize| spans.iter().any(|s| s.first <= w && w <= s.last);
    let seg = words[span.first].seg;
    let same = |w: &Word| w.seg == seg;
    let closes_segment = words[span.last + 1..]
        .iter()
        .take_while(|w| same(w))
        .enumerate()
        .all(|(k, _)| in_span(span.last + 1 + k));
    let opens = |w: usize| w == 0 || !same(&words[w - 1]) || words[w - 1].closes;
    let mut start = span.first;
    let mut opening = opens(start).then_some(start);
    while start > 0 && same(&words[start - 1]) && in_span(start - 1) {
        start -= 1;
        if opens(start) {
            opening = Some(start);
        }
    }
    match opening {
        Some(start) if closes_segment => spans
            .iter()
            .filter(|s| s.first >= start && words[s.first].seg == seg)
            .count(),
        _ => 0,
    }
}

/// True when the span is a sentence of its own: it opens the take or
/// follows a word ending a sentence (`.`, `!` or `?`, a closing quote or
/// bracket after it allowed), and it ends the take or ends one itself. Segment breaks do not count, because Whisper splits
/// segments at timestamps, not at sentence ends. Only such a lone name
/// may stay by sound.
fn own_sentence(span: Span, words: &[Word]) -> bool {
    (span.first == 0 || words[span.first - 1].closes)
        && (span.last + 1 == words.len() || words[span.last].closes)
}

#[cfg(test)]
mod review_tests;
#[cfg(test)]
mod tests;
