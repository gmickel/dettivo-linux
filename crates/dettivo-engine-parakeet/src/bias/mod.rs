//! Vocabulary rescoring (ADR 0088, stage 1): after greedy decoding, each
//! run of heard words that could be a vocabulary term is scored twice by
//! the transducer over the same frames from the same decoder state, once
//! as heard and once as the term. The term replaces the run when
//! `log P(term) + boost × pieces(term) ≥ log P(heard)`, so a term lands
//! only where the audio supports it.

mod candidates;
mod pieces;

use dettivo_engine_proto::VocabularyBias;

use crate::ffi::{FfiError, RawTranscription, RawWord, Take};
pub use pieces::Pieces;

pub use candidates::MAX_CANDIDATES;
use candidates::Term;

/// The most token pieces a term may take; longer terms are skipped.
pub const MAX_TERM_PIECES: usize = 12;
/// Frames a scored window reaches past a span's own tokens on either side.
const PAD_FRAMES: i32 = 4;

/// The terms of one request, spelt in pieces, and the boost.
pub struct Rescorer {
    terms: Vec<(Term, Vec<i32>)>,
    boost: f32,
}

/// What rescoring one take did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    /// Candidate spans scored.
    pub candidates: usize,
    /// Spans replaced by a term.
    pub replaced: usize,
}

impl Rescorer {
    /// `None` when the boost is off or no term is usable, so the request
    /// takes the plain path.
    pub fn new(bias: &VocabularyBias, pieces: &Pieces) -> Option<Self> {
        if bias.boost.is_nan() || bias.boost <= 0.0 {
            return None;
        }
        let terms: Vec<(Term, Vec<i32>)> = bias
            .terms
            .iter()
            .filter_map(|t| {
                let term = Term::new(&t.term, &t.aliases)?;
                let tokens = pieces.encode(&term.text)?;
                (tokens.len() <= MAX_TERM_PIECES).then_some((term, tokens))
            })
            .collect();
        (!terms.is_empty()).then_some(Self {
            terms,
            boost: bias.boost,
        })
    }

    /// Rescores `raw`, the transcript of `take`, in place: at most `limit`
    /// candidates, each made only of words `eligible` admits.
    pub fn rescore(
        &self,
        take: &Take<'_>,
        pieces: &Pieces,
        raw: &mut RawTranscription,
        eligible: impl Fn(&RawWord) -> bool,
        limit: usize,
    ) -> Result<Outcome, FfiError> {
        let tokens = take.tokens();
        let ids: Vec<i32> = tokens.iter().map(|t| t.id).collect();
        let grouped = pieces.words(&ids);
        // The grouping must be the library's, word for word; a transcript
        // it cannot account for is left as it is.
        if grouped.len() != raw.words.len()
            || grouped.iter().zip(&raw.words).any(|(g, w)| g.text != w.w)
        {
            tracing::debug!("transcript words do not follow the tokens; no rescoring");
            return Ok(Outcome::default());
        }
        let texts: Vec<&str> = raw.words.iter().map(|w| w.w.as_str()).collect();
        let terms: Vec<Term> = self.terms.iter().map(|(t, _)| t.clone()).collect();
        let eligible: Vec<bool> = raw.words.iter().map(eligible).collect();
        let found = candidates::find(&texts, &terms, &eligible, limit);
        let mut outcome = Outcome {
            candidates: found.len(),
            replaced: 0,
        };
        let mut heard_scores: Vec<((usize, usize), f32)> = Vec::new();
        let mut accepted = Vec::new();
        for c in &found {
            let (start, end) = (grouped[c.first].start, grouped[c.last].core);
            let first = tokens[start];
            let last = tokens[end - 1];
            let resumed = start
                .checked_sub(1)
                .map_or(0, |p| tokens[p].frame + tokens[p].span);
            let lo = resumed
                .min(first.frame)
                .max(first.frame - PAD_FRAMES)
                .max(0);
            let next = tokens.get(end).map_or(take.frames(), |t| t.frame);
            let hi = next
                .min(last.frame + last.span + PAD_FRAMES)
                .max(last.frame + 1)
                .min(take.frames());
            let heard = match heard_scores.iter().find(|(k, _)| *k == (c.first, c.last)) {
                Some((_, score)) => *score,
                None => {
                    let score = take.score(start, lo, hi, &ids[start..end])?;
                    heard_scores.push(((c.first, c.last), score));
                    score
                }
            };
            let (term, term_tokens) = &self.terms[c.term];
            let scored = take.score(start, lo, hi, term_tokens)?;
            let pieces = term_tokens.len();
            let margin = scored + self.boost * pieces as f32 - heard;
            // Scores only: never the words.
            tracing::debug!(heard, scored, pieces, margin, "candidate scored");
            if margin >= 0.0 {
                accepted.push((margin, c.first, c.last, term.text.clone()));
            }
        }
        accepted.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut chosen: Vec<(usize, usize, String)> = Vec::new();
        for (_, first, last, text) in accepted {
            if chosen.iter().all(|(f, l, _)| last < *f || first > *l) {
                chosen.push((first, last, text));
            }
        }
        chosen.sort_by_key(|(first, _, _)| std::cmp::Reverse(*first));
        for (first, last, text) in &chosen {
            let words = replacement(text, &raw.words[*first..=*last]);
            raw.words.splice(*first..=*last, words);
        }
        outcome.replaced = chosen.len();
        if outcome.replaced > 0 {
            raw.text = raw
                .words
                .iter()
                .map(|w| w.w.as_str())
                .collect::<Vec<_>>()
                .join(" ");
        }
        Ok(outcome)
    }
}

/// The words of `term` in place of the heard `span`, one timed word each:
/// word for word when the counts match, otherwise the span's time shared
/// out by letters. Every word takes the span's lowest confidence.
fn replacement(term: &str, span: &[RawWord]) -> Vec<RawWord> {
    let parts: Vec<&str> = term.split_whitespace().collect();
    let conf = span.iter().map(|w| w.conf).fold(f64::INFINITY, f64::min);
    let (start, end) = (span[0].start, span[span.len() - 1].end);
    let letters: usize = parts.iter().map(|p| p.chars().count()).sum();
    let mut at = start;
    let last = parts.len() - 1;
    parts
        .iter()
        .enumerate()
        .map(|(i, part)| {
            let (from, to) = if parts.len() == span.len() {
                (span[i].start, span[i].end)
            } else {
                let to = at + (end - start) * part.chars().count() as f64 / letters as f64;
                (std::mem::replace(&mut at, to), to)
            };
            RawWord {
                w: if i == last {
                    written(part, &span[span.len() - 1].w)
                } else {
                    part.to_string()
                },
                start: from,
                end: if i == last { end } else { to },
                conf,
            }
        })
        .collect()
}

/// The term as it replaces heard words: followed by the marks that close
/// the last heard word (a comma, a full stop, a quote, a bracket), except
/// the term's own marks when the heard word already spells them (`C++.`)
/// and a sentence mark when the term ends in one of its own.
fn written(term: &str, heard_last: &str) -> String {
    let bare = heard_last.trim_end_matches(|c: char| !c.is_alphanumeric());
    let own = &term[term.trim_end_matches(|c: char| !c.is_alphanumeric()).len()..];
    let mut marks = &heard_last[bare.len()..];
    marks = marks.strip_prefix(own).unwrap_or(marks);
    if term.ends_with(['.', '!', '?']) {
        marks = marks.trim_start_matches(['.', '!', '?']);
    }
    format!("{term}{marks}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(w: &str, start: f64, end: f64, conf: f64) -> RawWord {
        RawWord {
            w: w.into(),
            start,
            end,
            conf,
        }
    }

    #[test]
    fn the_term_takes_the_heard_marks_unless_it_has_its_own() {
        assert_eq!(written("Hyprland", "Hyperland."), "Hyprland.");
        assert_eq!(written("Hyprland", "Hyperland"), "Hyprland");
        assert_eq!(written("Hyperland.", "Hyperland."), "Hyperland.");
        assert_eq!(written("Yahoo!", "yahoo."), "Yahoo!");
    }

    /// Marks around a term that ends in its own (`C++`, `C#`) are kept.
    #[test]
    fn a_term_ending_in_marks_keeps_the_heard_comma_quote_and_bracket() {
        assert_eq!(written("C++", "plus,"), "C++,");
        assert_eq!(written("C++", "plus."), "C++.");
        assert_eq!(written("C#", "sharp)"), "C#)");
        assert_eq!(written("C#", "sharp\u{201d}."), "C#\u{201d}.");
        assert_eq!(written("Yahoo!", "yahoo!\""), "Yahoo!\"");
    }

    /// A heard word that already spells the term's own marks (`C++.`)
    /// gives only its closing marks, never a second copy of the term's.
    #[test]
    fn a_heard_word_spelling_the_terms_marks_adds_none_of_them() {
        assert_eq!(written("C++", "C++."), "C++.");
        assert_eq!(written("C++", "c++,"), "C++,");
        assert_eq!(written("C#", "C#)"), "C#)");
    }

    /// A multi-word term comes back as timed words, never one word.
    #[test]
    fn a_multi_word_term_is_one_timed_word_per_word() {
        let heard = [word("Gnu", 1.0, 1.2, 0.9), word("Yolk.", 1.3, 1.6, 0.5)];
        let out = replacement("New York", &heard);
        let as_tuples: Vec<(&str, f64, f64, f64)> = out
            .iter()
            .map(|w| (w.w.as_str(), w.start, w.end, w.conf))
            .collect();
        assert_eq!(
            as_tuples,
            [("New", 1.0, 1.2, 0.5), ("York.", 1.3, 1.6, 0.5)]
        );
        // One heard word, two term words: the time is shared by letters.
        let out = replacement("New York", &[word("Newark", 2.0, 2.7, 0.8)]);
        assert_eq!(out.len(), 2);
        assert_eq!((out[0].w.as_str(), out[0].start), ("New", 2.0));
        assert_eq!((out[1].w.as_str(), out[1].end), ("York", 2.7));
        assert!((out[0].end - 2.3).abs() < 1e-9 && out[1].start == out[0].end);
        // Several heard words, one term word: the span's whole time.
        let out = replacement(
            "SapienXT",
            &[word("Zapian", 3.0, 3.4, 0.7), word("XT", 3.4, 3.8, 0.6)],
        );
        assert_eq!(out, [word("SapienXT", 3.0, 3.8, 0.6)]);
    }
}
