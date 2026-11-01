//! Which runs of heard words could be a vocabulary term (stage 1 of
//! ADR 0088): a run of one to four words qualifies when it is a listed
//! alias, shares most letters with the term or an alias, is a small edit
//! away from one, or (for a term of five sounds or more) is one consonant
//! sound off. Case-only differences, inflections of the term, words inside
//! paths, URLs and addresses, and terms under five letters without an
//! alias are never candidates.

use super::sound::{common_letters, edit_distance, key, normalise};

/// The most words a candidate span covers.
pub const MAX_SPAN_WORDS: usize = 4;
/// The most candidate spans one decode scores.
pub const MAX_CANDIDATES: usize = 16;
/// The fewest letters a term without an alias needs.
pub const MIN_TERM_LETTERS: usize = 5;
/// The fewest sounds a term needs for the consonant-sound rule.
pub const MIN_PHONETIC_SOUNDS: usize = 5;

/// A term as matching reads it.
#[derive(Debug, Clone)]
pub struct Term {
    /// The spelling wanted.
    pub text: String,
    lower: String,
    norm: String,
    aliases: Vec<String>,
    key: String,
    sounds: usize,
}

impl Term {
    /// The term ready for matching; `None` for a term matching skips: one
    /// under five letters with no alias, or one with no letters at all.
    pub fn new(text: &str, aliases: &[String]) -> Option<Self> {
        let norm = normalise(text);
        let aliases: Vec<String> = aliases
            .iter()
            .map(|a| normalise(a))
            .filter(|a| !a.is_empty() && *a != norm)
            .collect();
        if norm.is_empty() || (norm.chars().count() < MIN_TERM_LETTERS && aliases.is_empty()) {
            return None;
        }
        let (key, sounds) = key(&norm);
        Some(Self {
            text: text.trim().to_string(),
            lower: text.trim().to_lowercase(),
            norm,
            aliases,
            key,
            sounds,
        })
    }

    /// True when the words already are the term: the term in any case
    /// (`as_written`, lowercased), or the term or a spelling of it with
    /// `s` or `es` (`heard`, normalised), its plural or possessive.
    fn written(&self, as_written: &str, heard: &str) -> bool {
        as_written == self.lower
            || std::iter::once(&self.norm).chain(&self.aliases).any(|s| {
                heard
                    .strip_prefix(s.as_str())
                    .is_some_and(|rest| rest == "s" || rest == "es")
            })
    }

    /// How closely `heard` (normalised) matches: lower is closer, `None`
    /// when it does not qualify.
    fn rank(&self, heard: &str) -> Option<u32> {
        let spellings = || std::iter::once(&self.norm).chain(&self.aliases);
        if self.aliases.iter().any(|a| a == heard) {
            return Some(0);
        }
        let len = heard.chars().count();
        let mut best = None::<u32>;
        for s in spellings() {
            let n = s.chars().count();
            let longer = n.max(len);
            // Lengths alone rule out both rules below, before any table.
            if 4 * n.min(len) < 3 * longer && n.abs_diff(len) > (n / 4).max(1) {
                continue;
            }
            let edits = edit_distance(heard, s);
            let similar = 4 * common_letters(heard, s) >= 3 * longer || edits <= (n / 4).max(1);
            if similar {
                let rank = 1 + 100 * edits as u32 / longer as u32;
                best = Some(best.map_or(rank, |b| b.min(rank)));
            }
        }
        let n = self.norm.chars().count();
        if best.is_none() && self.sounds >= MIN_PHONETIC_SOUNDS && 2 * len >= n && len <= 2 * n {
            let (heard_key, _) = key(heard);
            if !heard_key.is_empty()
                && heard_key.len().abs_diff(self.key.len()) <= 1
                && edit_distance(&heard_key, &self.key) <= 1
            {
                best = Some(200);
            }
        }
        best
    }

    /// The fewest edits from `heard` to the term or one of its aliases.
    fn closest(&self, heard: &str) -> usize {
        std::iter::once(&self.norm)
            .chain(&self.aliases)
            .map(|s| edit_distance(heard, s))
            .min()
            .unwrap_or(usize::MAX)
    }
}

/// A word of a path, URL or address (`src/app`, `example.com`,
/// `me@example.org`): never part of a candidate.
pub fn is_protected(word: &str) -> bool {
    if word.contains(['/', '\\', '@', '~']) || word.contains("://") {
        return true;
    }
    let chars: Vec<char> = word.chars().collect();
    chars
        .windows(3)
        .any(|w| w[1] == '.' && w[0].is_alphanumeric() && w[2].is_alphanumeric())
}

/// A heard span that could be a term: words `first..=last` and the term.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    /// First word.
    pub first: usize,
    /// Last word.
    pub last: usize,
    /// Index into the terms.
    pub term: usize,
}

/// Every candidate in `words` (each as the transcript spells it), closest
/// first, at most [`MAX_CANDIDATES`].
pub fn find(words: &[&str], terms: &[Term]) -> Vec<Candidate> {
    // A word that opens with a mark (a quote, a bracket) or holds none of
    // its letters cannot take a term's spelling without losing it.
    let usable: Vec<bool> = words
        .iter()
        .map(|w| !is_protected(w) && w.chars().next().is_some_and(char::is_alphanumeric))
        .collect();
    let norms: Vec<String> = words.iter().map(|w| normalise(w)).collect();
    let joined = |from: usize, to: usize| norms[from..to].concat();
    let spans = || {
        (0..words.len()).flat_map(|first| {
            (first..words.len().min(first + MAX_SPAN_WORDS)).map(move |last| (first, last))
        })
    };
    // Words that already are a term stay as heard, and no shorter or
    // overlapping run rewrites them into that term.
    let mut written: Vec<(usize, usize, usize)> = Vec::new();
    for (first, last) in spans() {
        let heard = joined(first, last + 1);
        let as_written = words[first..=last]
            .join(" ")
            .trim_end_matches(|c: char| c.is_ascii_punctuation())
            .to_lowercase();
        for (index, term) in terms.iter().enumerate() {
            if term.written(&as_written, &heard) {
                written.push((first, last, index));
            }
        }
    }
    let mut found: Vec<(u32, Candidate)> = Vec::new();
    for (first, last) in spans() {
        if !usable[first..=last].iter().all(|u| *u) {
            continue;
        }
        let heard = joined(first, last + 1);
        for (index, term) in terms.iter().enumerate() {
            if written
                .iter()
                .any(|&(f, l, t)| t == index && first <= l && last >= f)
            {
                continue;
            }
            let Some(rank) = term.rank(&heard) else {
                continue;
            };
            // Each word of a longer span must bring it closer to the
            // term, so a neighbouring word is never swallowed.
            let edges_count = first == last || {
                let whole = term.closest(&heard);
                whole < term.closest(&joined(first + 1, last + 1))
                    && whole < term.closest(&joined(first, last))
            };
            if edges_count {
                found.push((
                    rank,
                    Candidate {
                        first,
                        last,
                        term: index,
                    },
                ));
            }
        }
    }
    found.sort_by_key(|(rank, c)| (*rank, c.first, c.last, c.term));
    found.truncate(MAX_CANDIDATES);
    found.into_iter().map(|(_, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(text: &str, aliases: &[&str]) -> Term {
        let aliases: Vec<String> = aliases.iter().map(|a| a.to_string()).collect();
        Term::new(text, &aliases).unwrap()
    }

    fn spans(heard: &str, terms: &[Term]) -> Vec<(usize, usize, usize)> {
        let words: Vec<&str> = heard.split_whitespace().collect();
        find(&words, terms)
            .into_iter()
            .map(|c| (c.first, c.last, c.term))
            .collect()
    }

    #[test]
    fn mishearings_of_a_term_are_candidates() {
        let terms = [
            term("Dettivo", &["detivo"]),
            term("Hyprland", &[]),
            term("SapienXT", &[]),
            term("Omarchy", &[]),
        ];
        // An edit away from an alias, letters in common, two words that
        // spell the term, one consonant sound off.
        assert!(spans("with dativo on", &terms).contains(&(1, 1, 0)));
        assert!(spans("The HyperLAN config", &terms).contains(&(1, 1, 1)));
        assert!(spans("to Zapian XT before", &terms).contains(&(1, 2, 2)));
        assert!(spans("in Omar He changed", &terms).contains(&(1, 2, 3)));
        assert!(spans("Itivo puts the text", &terms).contains(&(0, 0, 0)));
    }

    /// R4: an alias widens matching.
    #[test]
    fn an_alias_finds_a_span_the_spelling_alone_misses() {
        assert_eq!(
            spans("she shaped the clay", &[term("Clef", &["clay"])]),
            vec![(3, 3, 0)]
        );
        assert!(
            Term::new("Clef", &[]).is_none(),
            "a short term needs an alias"
        );
        assert_eq!(
            spans("I installed oh marky", &[term("Omarchy", &["oh marky"])]),
            vec![(2, 3, 0)]
        );
        assert_eq!(
            spans("I installed oh marky", &[term("Omarchy", &[])]),
            vec![]
        );
    }

    /// R5: case-only differences, inflections, paths, URLs and addresses
    /// are never rewritten, and short terms without an alias are skipped.
    #[test]
    fn what_must_stay_as_heard_is_never_a_candidate() {
        let terms = [term("Dettivo", &["detivo"])];
        for heard in [
            "open dettivo now",
            "open DETTIVO now",
            "two Dettivos here",
            "the Dettivo's window",
            "the detivos window",
            "see src/detivo please",
            "see https://dativo.com please",
            "see dativo.com please",
            "mail me@dativo.org please",
        ] {
            assert_eq!(spans(heard, &terms), vec![], "{heard}");
        }
        assert!(Term::new("Jev", &[]).is_none());
        assert!(Term::new("Jev", &["jeff".into()]).is_some());
    }

    /// R5: words that already are a multi-word term are never rewritten
    /// in part, so no word of the term is written twice.
    #[test]
    fn a_term_already_heard_is_never_rewritten_in_part() {
        for (heard, written) in [
            ("moved because of the holiday", "because of the"),
            ("O Markey ships with", "o Markey"),
            ("Rain is expected over", "Rain Is"),
            ("two Sapien XTs here", "Sapien XT"),
        ] {
            assert_eq!(spans(heard, &[term(written, &[])]), vec![], "{heard}");
        }
    }

    #[test]
    fn a_span_never_swallows_a_neighbouring_word() {
        let found = spans("invoice to Sapien XT before", &[term("SapienXT", &[])]);
        assert!(found.contains(&(2, 3, 0)), "{found:?}");
        assert!(!found.contains(&(1, 3, 0)), "{found:?}");
    }

    #[test]
    fn unrelated_words_are_not_candidates_and_the_count_is_capped() {
        let terms = [
            term("Hyprland", &[]),
            term("SapienXT", &[]),
            term("Detective", &[]),
        ];
        assert_eq!(spans("We walked across the highland", &terms[..2]), vec![]);
        let many = "detectiv ".repeat(20);
        assert_eq!(spans(&many, &terms).len(), MAX_CANDIDATES);
    }
}
