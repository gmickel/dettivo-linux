//! Reconciling consecutive chunks by timestamp. Inside an overlap the
//! later chunk's words win from the midpoint: the earlier chunk keeps the
//! words that start before it, the later chunk the words that start at or
//! after it; a word the later chunk repeats within `DEDUPE_MS` of one the
//! earlier chunk kept is dropped, and so is a run of words at the seam
//! that the earlier chunk's tail already said, when both sit in the audio
//! the two chunks shared (within `SEAM_TOLERANCE_MS`). Chunks that share
//! no audio are not reconciled at all: a word said twice with a gap
//! between is two words. Segment boundaries then snap to the words that
//! remain. An engine without word timestamps gets
//! words spread evenly over each segment; their times are estimates, so
//! for those the seam run decides first and the midpoint rule applies
//! only when no run anchors the seam. Synthetic words never leave the
//! merger. The meeting spec reuses this for two sources.

use dettivo_engine_proto::{Segment, Word};

/// A repeated word this close to the earlier chunk's copy is a duplicate.
pub const DEDUPE_MS: u64 = 250;

/// The longest run of words compared at a seam.
const SEAM_RUN: usize = 12;

/// How far outside the shared audio a word may start and still be a seam
/// candidate: the engines' timestamp precision (ADR 0018) plus the
/// spread of words estimated over a segment.
pub const SEAM_TOLERANCE_MS: u64 = 1_500;

/// One chunk's engine result on the audio's clock.
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkResult {
    /// Where the chunk starts on the audio's clock.
    pub start_ms: u64,
    /// Where it ends.
    pub end_ms: u64,
    /// The engine's segments, timed relative to the chunk.
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone)]
struct Piece {
    start_ms: u64,
    end_ms: u64,
    words: Vec<Word>,
    synthesised: bool,
}

fn normalise(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

/// Segments on the audio's clock, with words spread evenly when the
/// engine gave none.
fn pieces(chunk: &ChunkResult) -> Vec<Piece> {
    chunk
        .segments
        .iter()
        .filter(|s| !s.text.trim().is_empty())
        .map(|s| {
            let start_ms = chunk.start_ms + s.start_ms;
            let end_ms = chunk.start_ms + s.end_ms.max(s.start_ms);
            if !s.words.is_empty() {
                let words = s
                    .words
                    .iter()
                    .map(|w| Word {
                        start_ms: chunk.start_ms + w.start_ms,
                        end_ms: chunk.start_ms + w.end_ms,
                        text: w.text.clone(),
                        confidence: w.confidence,
                    })
                    .collect();
                return Piece {
                    start_ms,
                    end_ms,
                    words,
                    synthesised: false,
                };
            }
            let tokens: Vec<&str> = s.text.split_whitespace().collect();
            let span = (end_ms - start_ms) as f64 / tokens.len().max(1) as f64;
            let words = tokens
                .iter()
                .enumerate()
                .map(|(i, t)| Word {
                    start_ms: start_ms + (i as f64 * span) as u64,
                    end_ms: start_ms + ((i + 1) as f64 * span) as u64,
                    text: (*t).to_string(),
                    confidence: 1.0,
                })
                .collect();
            Piece {
                start_ms,
                end_ms,
                words,
                synthesised: true,
            }
        })
        .collect()
}

/// The last `n` words of `pieces` that start at or after `from_ms`.
fn last_words(pieces: &[Piece], n: usize, from_ms: u64) -> Vec<String> {
    let mut out: Vec<String> = pieces
        .iter()
        .rev()
        .flat_map(|p| p.words.iter().rev())
        .take_while(|w| w.start_ms >= from_ms)
        .map(|w| normalise(&w.text))
        .take(n)
        .collect();
    out.reverse();
    out
}

/// The longest run of words at the seam the earlier chunk's tail already
/// said, dropped from the head of `later`; how many were dropped. Only
/// words inside the shared audio `overlap` (widened by
/// `SEAM_TOLERANCE_MS`) are candidates, so a phrase repeated on either
/// side of the seam stays.
fn drop_seam_run(earlier: &[Piece], later: &mut Vec<Piece>, overlap: (u64, u64)) -> usize {
    let tail = last_words(
        earlier,
        SEAM_RUN,
        overlap.0.saturating_sub(SEAM_TOLERANCE_MS),
    );
    let head: Vec<String> = later
        .iter()
        .flat_map(|p| p.words.iter())
        .take_while(|w| w.start_ms <= overlap.1 + SEAM_TOLERANCE_MS)
        .map(|w| normalise(&w.text))
        .take(SEAM_RUN)
        .collect();
    let mut run = 0;
    for k in (1..=tail.len().min(head.len())).rev() {
        if tail[tail.len() - k..] == head[..k] {
            run = k;
            break;
        }
    }
    let mut to_drop = run;
    for p in later.iter_mut() {
        let n = to_drop.min(p.words.len());
        p.words.drain(..n);
        to_drop -= n;
        if to_drop == 0 {
            break;
        }
    }
    later.retain(|p| !p.words.is_empty());
    run
}

/// The midpoint rule: the earlier chunk keeps the words that start before
/// `mid_ms`, the later chunk those at or after it, less the ones repeated
/// within `DEDUPE_MS` of a kept word.
fn split_at_midpoint(earlier: &mut Vec<Piece>, later: &mut Vec<Piece>, mid_ms: u64) {
    for p in earlier.iter_mut() {
        p.words.retain(|w| w.start_ms < mid_ms);
    }
    earlier.retain(|p| !p.words.is_empty());
    let kept: Vec<(String, u64)> = earlier
        .iter()
        .flat_map(|p| p.words.iter().map(|w| (normalise(&w.text), w.start_ms)))
        .collect();
    for p in later.iter_mut() {
        p.words.retain(|w| {
            w.start_ms >= mid_ms
                && !kept.iter().any(|(text, start)| {
                    *text == normalise(&w.text) && w.start_ms.abs_diff(*start) <= DEDUPE_MS
                })
        });
    }
    later.retain(|p| !p.words.is_empty());
}

/// Drops from `later` what the seam rules say `earlier` already covers
/// inside the shared audio `overlap`.
fn reconcile(earlier: &mut Vec<Piece>, mut later: Vec<Piece>, overlap: (u64, u64)) -> Vec<Piece> {
    let mid_ms = overlap.0 + (overlap.1 - overlap.0) / 2;
    let precise = earlier.iter().chain(later.iter()).all(|p| !p.synthesised);
    if precise {
        split_at_midpoint(earlier, &mut later, mid_ms);
        drop_seam_run(earlier, &mut later, overlap);
    } else if drop_seam_run(earlier, &mut later, overlap) == 0 {
        split_at_midpoint(earlier, &mut later, mid_ms);
    }
    later
}

/// Merges chunk results into segments on the audio's clock.
pub fn merge(chunks: &[ChunkResult]) -> Vec<Segment> {
    let mut acc: Vec<Piece> = Vec::new();
    let mut previous_end = 0u64;
    for (i, chunk) in chunks.iter().enumerate() {
        // A chunk without segments (a skipped silent window) heard nothing
        // the next chunk could repeat, so it never opens an overlap.
        if chunk.segments.is_empty() {
            continue;
        }
        let later = pieces(chunk);
        if i == 0 {
            acc = later;
        } else if chunk.start_ms >= previous_end {
            // No shared audio: nothing the earlier chunk heard was heard
            // again, so a repeated word is a repetition, not a duplicate.
            acc.extend(later);
        } else {
            let reconciled = reconcile(&mut acc, later, (chunk.start_ms, previous_end));
            acc.extend(reconciled);
        }
        previous_end = chunk.end_ms;
    }
    acc.sort_by_key(|p| p.words.first().map(|w| w.start_ms).unwrap_or(p.start_ms));
    acc.into_iter()
        .map(|p| {
            let start_ms = p.words.first().map(|w| w.start_ms).unwrap_or(p.start_ms);
            let end_ms = p.words.last().map(|w| w.end_ms).unwrap_or(p.end_ms);
            let text = p
                .words
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            Segment {
                start_ms,
                end_ms,
                text,
                words: if p.synthesised { Vec::new() } else { p.words },
            }
        })
        .collect()
}

/// The merged text: segments joined by one space.
pub fn text_of(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|s| s.text.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(start: u64, end: u64, text: &str) -> Segment {
        Segment {
            start_ms: start,
            end_ms: end,
            text: text.into(),
            words: Vec::new(),
        }
    }

    #[test]
    fn segments_without_words_are_spread_evenly_and_a_seam_run_is_dropped_once() {
        let a = ChunkResult {
            start_ms: 0,
            end_ms: 299_000,
            segments: vec![seg(
                288_000,
                299_000,
                "ask not what your country can do for you ask what you can do for your country",
            )],
        };
        let b = ChunkResult {
            start_ms: 297_500,
            end_ms: 400_000,
            segments: vec![
                seg(0, 6_000, "for your country. And so my fellow Americans"),
                seg(6_500, 9_000, "the end."),
            ],
        };
        let merged = merge(&[a, b]);
        assert_eq!(
            text_of(&merged),
            "ask not what your country can do for you ask what you can do for your country And so my fellow Americans the end."
        );
        assert!(
            merged.iter().all(|s| s.words.is_empty()),
            "no synthesised words leave"
        );
        assert_eq!(merged[0].start_ms, 288_000);
        assert!(merged[1].start_ms >= 298_250, "{}", merged[1].start_ms);
    }

    /// A word said again after a gap is two words: chunks that share no
    /// audio are not reconciled, and inside an overlap only the shared
    /// audio's words are seam candidates.
    #[test]
    fn a_repetition_across_a_gap_or_outside_the_shared_audio_stays() {
        let a = ChunkResult {
            start_ms: 0,
            end_ms: 3_000,
            segments: vec![seg(500, 1_000, "yes")],
        };
        let b = ChunkResult {
            start_ms: 3_000,
            end_ms: 6_000,
            segments: vec![seg(500, 1_000, "yes")],
        };
        let c = ChunkResult {
            start_ms: 9_000,
            end_ms: 12_000,
            segments: vec![seg(500, 1_000, "yes")],
        };
        assert_eq!(text_of(&merge(&[a, b, c])), "yes yes yes");

        let a = ChunkResult {
            start_ms: 0,
            end_ms: 10_000,
            segments: vec![seg(1_000, 2_000, "yes")],
        };
        let b = ChunkResult {
            start_ms: 8_000,
            end_ms: 18_000,
            segments: vec![seg(1_000, 2_000, "yes")],
        };
        let merged = merge(&[a, b]);
        assert_eq!(text_of(&merged), "yes yes");
        assert_eq!(merged[1].start_ms, 9_000);
    }

    /// A skipped silent window heard nothing, so the speech that opens
    /// the next window inside their overlap is kept.
    #[test]
    fn speech_after_a_silent_window_keeps_its_onset() {
        let a = ChunkResult {
            start_ms: 0,
            end_ms: 30_000,
            segments: vec![seg(1_000, 2_000, "hello")],
        };
        let silent = ChunkResult {
            start_ms: 28_000,
            end_ms: 58_000,
            segments: Vec::new(),
        };
        let b = ChunkResult {
            start_ms: 56_000,
            end_ms: 86_000,
            segments: vec![seg(0, 800, "wait"), seg(900, 2_000, "there")],
        };
        let merged = merge(&[a, silent, b]);
        assert_eq!(text_of(&merged), "hello wait there");
        assert_eq!(merged[1].start_ms, 56_000);
    }

    /// The same words heard by both chunks in the audio they share are
    /// still one run.
    #[test]
    fn a_duplicate_inside_the_shared_audio_is_dropped_once() {
        let a = ChunkResult {
            start_ms: 0,
            end_ms: 10_000,
            segments: vec![seg(7_000, 9_500, "and then we ship it")],
        };
        let b = ChunkResult {
            start_ms: 8_000,
            end_ms: 18_000,
            segments: vec![seg(0, 1_500, "we ship it"), seg(2_000, 3_000, "next week")],
        };
        assert_eq!(text_of(&merge(&[a, b])), "and then we ship it next week");
    }

    #[test]
    fn an_overlap_with_no_words_on_either_side_is_a_no_op() {
        let a = ChunkResult {
            start_ms: 0,
            end_ms: 10_000,
            segments: vec![seg(1_000, 3_000, "one two")],
        };
        let b = ChunkResult {
            start_ms: 8_000,
            end_ms: 18_000,
            segments: vec![seg(4_000, 6_000, "three four")],
        };
        let merged = merge(&[a, b]);
        assert_eq!(text_of(&merged), "one two three four");
        assert_eq!((merged[1].start_ms, merged[1].end_ms), (12_000, 14_000));
    }
}
