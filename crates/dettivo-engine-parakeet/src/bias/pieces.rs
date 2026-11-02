//! The tokenizer's pieces as rescoring reads them: a term spelt in the
//! fewest pieces, and greedy tokens grouped into words the way
//! parakeet.cpp groups them, with each word's trailing punctuation apart.

use std::collections::HashMap;

/// SentencePiece's word-start mark.
pub const META: char = '\u{2581}';

/// The piece table.
pub struct Pieces {
    pieces: Vec<String>,
    ids: HashMap<String, i32>,
    longest: usize,
}

/// A word of the greedy transcript as token indices: `start..end` are its
/// tokens, `start..core` the ones before its trailing punctuation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenWord {
    /// First token.
    pub start: usize,
    /// One past the last token before trailing punctuation.
    pub core: usize,
    /// One past the last token.
    pub end: usize,
    /// The word as the transcript spells it.
    pub text: String,
}

fn is_punctuation(piece: &str) -> bool {
    !piece.is_empty() && piece.chars().all(|c| c.is_ascii_punctuation())
}

impl Pieces {
    /// The table, indexed by token id.
    pub fn new(pieces: Vec<String>) -> Self {
        let mut ids = HashMap::new();
        let mut longest = 0;
        for (id, piece) in pieces.iter().enumerate() {
            let special = piece.starts_with('<') && piece.ends_with('>');
            if piece.is_empty() || special {
                continue;
            }
            longest = longest.max(piece.chars().count());
            ids.entry(piece.clone()).or_insert(id as i32);
        }
        Self {
            pieces,
            ids,
            longest,
        }
    }

    fn piece(&self, id: i32) -> &str {
        usize::try_from(id)
            .ok()
            .and_then(|i| self.pieces.get(i))
            .map_or("", String::as_str)
    }

    /// `text` as the fewest pieces, each of its words starting with the
    /// word-start mark; `None` when the pieces cannot spell it.
    pub fn encode(&self, text: &str) -> Option<Vec<i32>> {
        let marked: Vec<char> = text
            .split_whitespace()
            .flat_map(|w| std::iter::once(META).chain(w.chars()))
            .collect();
        let n = marked.len();
        if n == 0 {
            return None;
        }
        // best[i]: the fewest pieces spelling marked[..i], and the last one.
        let mut best: Vec<Option<(usize, usize, i32)>> = vec![None; n + 1];
        best[0] = Some((0, 0, -1));
        for i in 0..n {
            let Some((count, _, _)) = best[i] else {
                continue;
            };
            let mut piece = String::new();
            for j in i..n.min(i + self.longest) {
                piece.push(marked[j]);
                if let Some(&id) = self.ids.get(&piece) {
                    let better = best[j + 1].is_none_or(|(c, _, _)| count + 1 < c);
                    if better {
                        best[j + 1] = Some((count + 1, i, id));
                    }
                }
            }
        }
        let mut out = Vec::new();
        let mut at = n;
        while at > 0 {
            let (_, from, id) = best[at]?;
            out.push(id);
            at = from;
        }
        out.reverse();
        Some(out)
    }

    /// `tokens` grouped into words: a piece with the word-start mark opens
    /// a word unless it is punctuation, which stays with the word before.
    pub fn words(&self, tokens: &[i32]) -> Vec<TokenWord> {
        let mut words: Vec<TokenWord> = Vec::new();
        for (i, &id) in tokens.iter().enumerate() {
            let piece = self.piece(id);
            let bare = piece.trim_start_matches(META);
            let opens = piece.starts_with(META) && !is_punctuation(bare);
            match words.last_mut() {
                Some(word) if !opens => {
                    word.end = i + 1;
                    word.text.push_str(bare);
                    if !is_punctuation(bare) {
                        word.core = i + 1;
                    }
                }
                _ => words.push(TokenWord {
                    start: i,
                    core: i + 1,
                    end: i + 1,
                    text: bare.to_string(),
                }),
            }
        }
        words
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Pieces {
        let p = |s: &str| s.replace('_', "\u{2581}");
        Pieces::new(
            [
                "<unk>", "_Hy", "per", "land", "_Hyper", "pr", "_The", ".", ",", "_", "H", "y",
                "p", "r", "l", "a", "n", "d", "_XT", "_Sap", "ien", "XT",
            ]
            .iter()
            .map(|s| p(s))
            .collect(),
        )
    }

    #[test]
    fn a_term_is_spelt_in_the_fewest_pieces() {
        let t = table();
        assert_eq!(t.encode("Hyprland"), Some(vec![1, 5, 3]));
        assert_eq!(t.encode("Hyperland"), Some(vec![4, 3]));
        assert_eq!(t.encode("Sapien XT"), Some(vec![19, 20, 18]));
        assert_eq!(t.encode("Hyprland!"), None, "no piece spells the mark");
        assert_eq!(t.encode("  "), None);
    }

    #[test]
    fn tokens_group_into_words_with_trailing_punctuation_apart() {
        let t = table();
        // "The Hyperland, XT."
        let words = t.words(&[6, 4, 3, 8, 18, 7]);
        let spans: Vec<(usize, usize, usize, &str)> = words
            .iter()
            .map(|w| (w.start, w.core, w.end, w.text.as_str()))
            .collect();
        assert_eq!(
            spans,
            vec![(0, 1, 1, "The"), (1, 3, 4, "Hyperland,"), (4, 5, 6, "XT.")]
        );
    }
}
