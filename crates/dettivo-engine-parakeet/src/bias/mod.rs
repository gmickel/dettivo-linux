//! Vocabulary rescoring (ADR 0088, stage 1): after greedy decoding, each
//! run of heard words that could be a vocabulary term is scored twice by
//! the transducer over the same frames from the same decoder state, once
//! as heard and once as the term. The term replaces the run when
//! `log P(term) + boost × pieces(term) ≥ log P(heard)`, so a term lands
//! only where the audio supports it.

mod candidates;
mod pieces;
mod sound;

use dettivo_engine_proto::VocabularyBias;

use crate::ffi::{FfiError, RawTranscription, RawWord, Take};
pub use pieces::Pieces;

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

    /// Rescores `raw`, the transcript of `take`, in place.
    pub fn rescore(
        &self,
        take: &Take<'_>,
        pieces: &Pieces,
        raw: &mut RawTranscription,
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
        let found = candidates::find(&texts, &terms);
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
            let span = &raw.words[*first..=*last];
            let word = RawWord {
                w: written(text, &span[span.len() - 1].w),
                start: span[0].start,
                end: span[span.len() - 1].end,
                conf: span.iter().map(|w| w.conf).fold(f64::INFINITY, f64::min),
            };
            raw.words.splice(*first..=*last, [word]);
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

/// The term as it replaces heard words: with the trailing marks of the
/// last heard word, unless the term ends in a mark of its own.
fn written(term: &str, heard_last: &str) -> String {
    if term.ends_with(|c: char| c.is_ascii_punctuation()) {
        return term.to_string();
    }
    let bare = heard_last.trim_end_matches(|c: char| c.is_ascii_punctuation());
    format!("{term}{}", &heard_last[bare.len()..])
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_term_takes_the_heard_marks_unless_it_has_its_own() {
        assert_eq!(super::written("Hyprland", "Hyperland."), "Hyprland.");
        assert_eq!(super::written("Hyprland", "Hyperland"), "Hyprland");
        assert_eq!(super::written("Hyperland.", "Hyperland."), "Hyperland.");
        assert_eq!(super::written("Yahoo!", "yahoo."), "Yahoo!");
    }
}
