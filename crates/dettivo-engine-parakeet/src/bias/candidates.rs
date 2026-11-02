//! Which runs of heard words could be a vocabulary term (stage 1 of
//! ADR 0088): a run of one to four words qualifies when it is a listed
//! alias, shares most letters with the term or an alias, is a small edit
//! away from one, or (for a term of five sounds or more) is one consonant
//! sound off. Case-only differences, inflections of the term or of a
//! mishearing of it, words inside paths, URLs, addresses and code, runs
//! across punctuation, and terms under five letters without an alias are
//! never candidates.

use dettivo_engine_proto::sound::{common_letters, edit_distance, ends_run, key, normalise};

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
        let given: Vec<String> = aliases
            .iter()
            .map(|a| normalise(a))
            .filter(|a| !a.is_empty())
            .collect();
        // A spelling that normalises to the term itself (`A W S` for
        // `AWS`) still counts as one: the term matches it as it stands.
        if norm.is_empty() || (norm.chars().count() < MIN_TERM_LETTERS && given.is_empty()) {
            return None;
        }
        let aliases: Vec<String> = given.into_iter().filter(|a| *a != norm).collect();
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

    /// The stem of `heard` (normalised) when it is a plural or possessive
    /// of a mishearing of the term ("hyperlans" for "Hyprland's"). With a
    /// written possessive (`possessive` letters of `'s` to drop) the stem
    /// need only qualify. Otherwise `heard` must not be a listed spelling,
    /// and without its `s` or `es` it must qualify against a spelling that
    /// does not itself end in `s`, so `Cubernetes` still matches
    /// `Kubernetes`.
    fn inflected(&self, heard: &str, possessive: Option<usize>) -> Option<String> {
        if let Some(drop) = possessive {
            let stem = &heard[..heard.len().saturating_sub(drop)];
            return self.rank(stem).is_some().then(|| stem.to_string());
        }
        if self.aliases.iter().any(|a| a == heard) {
            return None;
        }
        ["s", "es"].iter().find_map(|tail| {
            let stem = heard.strip_suffix(tail)?;
            self.rank_by(stem, |s| !s.ends_with('s'))
                .is_some()
                .then(|| stem.to_string())
        })
    }

    /// How closely `heard` (normalised) matches: lower is closer, `None`
    /// when it does not qualify.
    fn rank(&self, heard: &str) -> Option<u32> {
        self.rank_by(heard, |_| true)
    }

    /// [`Term::rank`] against the spellings `keep` admits.
    fn rank_by(&self, heard: &str, keep: impl Fn(&str) -> bool) -> Option<u32> {
        let spellings = || {
            std::iter::once(&self.norm)
                .chain(&self.aliases)
                .filter(|s| keep(s))
        };
        if self.aliases.iter().any(|a| a == heard && keep(a)) {
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
        if best.is_none()
            && keep(&self.norm)
            && self.sounds >= MIN_PHONETIC_SOUNDS
            && 2 * len >= n
            && len <= 2 * n
        {
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

/// A word of a path, URL, address or code (`src/app`, `example.com`,
/// `me@example.org`, `std::io`, `` `x` ``): never part of a candidate.
pub fn is_protected(word: &str) -> bool {
    if word.contains(['/', '\\', '@', '~', '`']) || word.contains("::") {
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

/// Every word between a backtick and the next one, both included: code
/// the raw layer keeps byte for byte.
fn in_code(words: &[&str]) -> Vec<bool> {
    let mut code = vec![false; words.len()];
    let mut open = None;
    for (i, word) in words.iter().enumerate() {
        for _ in word.matches('`') {
            match open.take() {
                Some(from) => code[from..=i].iter_mut().for_each(|c| *c = true),
                None => open = Some(i),
            }
        }
    }
    code
}

/// The letters a written possessive adds to a word's normalised form:
/// one for `'s` (`XT's`), none for a bare `'` after an `s` (`Kubernetes'`).
fn possessive(word: &str) -> Option<usize> {
    let apostrophe = |c: char| matches!(c, '\'' | '\u{2019}');
    let bare = word.trim_end_matches(|c: char| !c.is_alphanumeric() && !apostrophe(c));
    let mut tail = bare.chars().rev();
    match (tail.next(), tail.next()) {
        (Some('s' | 'S'), Some(a)) if apostrophe(a) => Some(1),
        (Some(a), Some('s' | 'S')) if apostrophe(a) => Some(0),
        _ => None,
    }
}

/// Every candidate in `words` (each as the transcript spells it) whose
/// words are all `eligible`, closest first, at most `limit`.
pub fn find(words: &[&str], terms: &[Term], eligible: &[bool], limit: usize) -> Vec<Candidate> {
    // A word that opens with a mark (a quote, a bracket) or holds none of
    // its letters cannot take a term's spelling without losing it.
    let code = in_code(words);
    let usable: Vec<bool> = words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            eligible[i]
                && !code[i]
                && !is_protected(w)
                && w.chars().next().is_some_and(char::is_alphanumeric)
        })
        .collect();
    let norms: Vec<String> = words.iter().map(|w| normalise(w)).collect();
    let joined = |from: usize, to: usize| norms[from..to].concat();
    let spans = || {
        (0..words.len()).flat_map(|first| {
            (first..words.len().min(first + MAX_SPAN_WORDS)).map(move |last| (first, last))
        })
    };
    // Words that already are a term, or an inflection of a mishearing of
    // it, stay as heard, and no shorter or overlapping run rewrites them
    // into that term: `blocked[word]` lists those terms.
    let mut blocked: Vec<Vec<usize>> = vec![Vec::new(); words.len()];
    let mut inflected: Vec<(usize, usize, usize)> = Vec::new();
    for (first, last) in spans() {
        let heard = joined(first, last + 1);
        let as_written = words[first..=last]
            .join(" ")
            .trim_end_matches(|c: char| c.is_ascii_punctuation())
            .to_lowercase();
        let possessive = possessive(words[last]);
        for (index, term) in terms.iter().enumerate() {
            let block = term.written(&as_written, &heard)
                || match term.inflected(&heard, possessive) {
                    None => false,
                    // The suffix must belong to the run: its last word
                    // brings the stem closer to a spelling (`Pro's` in
                    // `Hyperland Pro's`, never `draws` after `Hyperland`).
                    Some(stem) => {
                        inflected.push((first, last, index));
                        first == last || {
                            let before = joined(first, last);
                            std::iter::once(&term.norm)
                                .chain(&term.aliases)
                                .any(|s| edit_distance(&stem, s) < edit_distance(&before, s))
                        }
                    }
                };
            if block {
                for terms in &mut blocked[first..=last] {
                    if !terms.contains(&index) {
                        terms.push(index);
                    }
                }
            }
        }
    }
    let mut found: Vec<(u32, Candidate)> = Vec::new();
    for (first, last) in spans() {
        if !usable[first..=last].iter().all(|u| *u)
            || words[first..last].iter().any(|w| ends_run(w))
        {
            continue;
        }
        let heard = joined(first, last + 1);
        for (index, term) in terms.iter().enumerate() {
            if blocked[first..=last].iter().any(|b| b.contains(&index))
                || inflected.contains(&(first, last, index))
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
    found.truncate(limit);
    found.into_iter().map(|(_, c)| c).collect()
}

#[cfg(test)]
#[path = "candidates_tests.rs"]
mod tests;
