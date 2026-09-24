//! Timestamp precision against a golden alignment (the S-13 spike, ADR
//! 0018): hypothesis words are paired with golden words by text, per-word
//! start and end offsets are summarised as median and p95, segment
//! boundaries are measured against the nearest golden word boundary, and
//! the written-down rule turns the numbers into a verdict.

use serde::{Deserialize, Serialize};

/// The rule, fixed before any run: every measured Parakeet model must
/// keep its p95 word offset within the merger's interleaving tolerance
/// and its median within half of it, with at least this share of the
/// golden words matched.
pub const P95_TOLERANCE_MS: u64 = 200;
/// See [`P95_TOLERANCE_MS`].
pub const MEDIAN_TOLERANCE_MS: u64 = 100;
/// See [`P95_TOLERANCE_MS`].
pub const MIN_MATCHED_SHARE: f64 = 0.9;

/// One word of the golden alignment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoldenWord {
    /// Lower-case text.
    pub text: String,
    /// Start in milliseconds.
    pub start_ms: u64,
    /// End in milliseconds.
    pub end_ms: u64,
}

/// The golden fixture file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Golden {
    /// The audio it aligns, relative to the models directory.
    pub fixture: String,
    /// How it was made.
    pub aligner: String,
    /// The aligner's frame stride.
    pub frame_ms: f64,
    /// The words.
    pub words: Vec<GoldenWord>,
}

/// A hypothesis word or segment boundary pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// Normalised text (empty for a bare segment).
    pub text: String,
    /// Start in milliseconds.
    pub start_ms: u64,
    /// End in milliseconds.
    pub end_ms: u64,
}

/// Median and p95 of absolute offsets, in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Offsets {
    /// How many offsets went in.
    pub count: usize,
    /// The median.
    pub median_ms: u64,
    /// The 95th percentile (nearest rank).
    pub p95_ms: u64,
    /// The largest.
    pub max_ms: u64,
}

/// Word-level precision of one engine against the golden words.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WordPrecision {
    /// Golden words.
    pub golden_words: usize,
    /// Hypothesis words paired with a golden word of the same text.
    pub matched_words: usize,
    /// Start offsets over the matched pairs.
    pub start: Offsets,
    /// End offsets over the matched pairs.
    pub end: Offsets,
    /// Start and end offsets pooled.
    pub combined: Offsets,
}

/// Lower-case alphanumerics only, so `Americans,` pairs with `americans`.
pub fn normalise(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

/// Nearest-rank percentiles over absolute offsets.
pub fn offsets(mut values: Vec<u64>) -> Offsets {
    if values.is_empty() {
        return Offsets::default();
    }
    values.sort_unstable();
    let rank = |p: f64| {
        let i = ((p * values.len() as f64).ceil() as usize).clamp(1, values.len()) - 1;
        values[i]
    };
    Offsets {
        count: values.len(),
        median_ms: rank(0.5),
        p95_ms: rank(0.95),
        max_ms: values[values.len() - 1],
    }
}

/// Pairs hypothesis words with golden words by the longest common
/// subsequence of their normalised texts, and measures the pairs.
pub fn word_precision(golden: &[GoldenWord], hypothesis: &[Span]) -> WordPrecision {
    let g: Vec<String> = golden.iter().map(|w| normalise(&w.text)).collect();
    let h: Vec<String> = hypothesis.iter().map(|w| normalise(&w.text)).collect();
    let (n, m) = (g.len(), h.len());
    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if g[i] == h[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let (mut starts, mut ends) = (Vec::new(), Vec::new());
    while i < n && j < m {
        if g[i] == h[j] {
            starts.push(golden[i].start_ms.abs_diff(hypothesis[j].start_ms));
            ends.push(golden[i].end_ms.abs_diff(hypothesis[j].end_ms));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    let mut combined = starts.clone();
    combined.extend_from_slice(&ends);
    WordPrecision {
        golden_words: n,
        matched_words: starts.len(),
        start: offsets(starts),
        end: offsets(ends),
        combined: offsets(combined),
    }
}

/// Each segment start against the nearest golden word start and each
/// segment end against the nearest golden word end, pooled.
pub fn boundary_precision(golden: &[GoldenWord], segments: &[Span]) -> Offsets {
    let nearest = |value: u64, candidates: &mut dyn Iterator<Item = u64>| {
        candidates.map(|c| c.abs_diff(value)).min().unwrap_or(value)
    };
    let mut values = Vec::new();
    for s in segments {
        values.push(nearest(s.start_ms, &mut golden.iter().map(|w| w.start_ms)));
        values.push(nearest(s.end_ms, &mut golden.iter().map(|w| w.end_ms)));
    }
    offsets(values)
}

/// The verdict for one Parakeet model under the rule above.
pub fn passes(p: &WordPrecision) -> bool {
    p.golden_words > 0
        && p.matched_words as f64 >= MIN_MATCHED_SHARE * p.golden_words as f64
        && p.combined.p95_ms <= P95_TOLERANCE_MS
        && p.combined.median_ms <= MEDIAN_TOLERANCE_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn golden(spec: &[(&str, u64, u64)]) -> Vec<GoldenWord> {
        spec.iter()
            .map(|(t, s, e)| GoldenWord {
                text: (*t).into(),
                start_ms: *s,
                end_ms: *e,
            })
            .collect()
    }

    fn spans(spec: &[(&str, u64, u64)]) -> Vec<Span> {
        spec.iter()
            .map(|(t, s, e)| Span {
                text: (*t).into(),
                start_ms: *s,
                end_ms: *e,
            })
            .collect()
    }

    #[test]
    fn percentiles_are_nearest_rank() {
        let o = offsets(vec![10, 20, 30, 40, 1000]);
        assert_eq!(
            (o.median_ms, o.p95_ms, o.max_ms, o.count),
            (30, 1000, 1000, 5)
        );
        let o = offsets((1..=100).collect());
        assert_eq!((o.median_ms, o.p95_ms), (50, 95));
        assert_eq!(offsets(vec![]), Offsets::default());
    }

    #[test]
    fn words_pair_by_text_through_punctuation_and_insertions() {
        let g = golden(&[("ask", 0, 100), ("not", 200, 300), ("what", 400, 500)]);
        let h = spans(&[
            ("Ask", 10, 110),
            ("uh", 150, 190),
            ("not,", 230, 330),
            ("what", 400, 600),
        ]);
        let p = word_precision(&g, &h);
        assert_eq!((p.golden_words, p.matched_words), (3, 3));
        assert_eq!(p.start.max_ms, 30);
        assert_eq!(p.end.max_ms, 100);
        assert_eq!(p.combined.count, 6);
        assert!(passes(&p));
        let late = spans(&[("ask", 300, 400), ("not", 500, 600), ("what", 700, 800)]);
        let p = word_precision(&g, &late);
        assert_eq!(p.combined.median_ms, 300);
        assert!(!passes(&p));
        let missing = spans(&[("ask", 0, 100)]);
        assert!(!passes(&word_precision(&g, &missing)));
    }

    #[test]
    fn segment_boundaries_measure_against_the_nearest_golden_boundary() {
        let g = golden(&[("a", 0, 100), ("b", 500, 700), ("c", 900, 1000)]);
        let s = spans(&[("", 20, 720), ("", 880, 1100)]);
        let o = boundary_precision(&g, &s);
        assert_eq!(o.count, 4);
        assert_eq!(o.max_ms, 100);
        assert_eq!(o.median_ms, 20);
    }
}
