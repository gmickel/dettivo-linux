//! The sentence-unit speaker rule (ADR 0072, amending ADR 0035). The
//! assigned segments are cut into pieces at sentence ends and, where the
//! words carry times, at pauses of at least `pause_ms`; consecutive
//! pieces join into a unit unless a sentence ends between them or such a
//! pause separates two different diarized speakers. Each unit takes the
//! speaker holding most of its time; a unit no turn overlaps takes the
//! nearest turn within `nearest_turn_ms` and otherwise stays unlabelled.
//! A segment whose pieces took different speakers becomes one stored
//! segment per speaker run.

use dettivo_engine_proto::SpeakerTurn;
use dettivo_proto::methods::meetings::Segment;

use crate::diarize::Rule;

/// A speaker and the winner's share of the unit's diarized speech (0
/// when the unit took the nearest turn).
pub(crate) type Label = Option<(String, f64)>;

/// A part of one segment between two cut points.
#[derive(Debug, Clone)]
struct Piece {
    /// The segment's position in the list.
    seg: usize,
    /// The token range `[first, last]` inside the segment.
    tokens: (usize, usize),
    span: (u64, u64),
    sentence_end: bool,
}

/// The whitespace-separated tokens of `text` as byte ranges.
fn tokens(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        match (c.is_whitespace(), start) {
            (true, Some(s)) => {
                out.push((s, i));
                start = None;
            }
            (false, None) => start = Some(i),
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, text.len()));
    }
    out
}

/// True when a token closes a sentence, closing quotes and brackets aside.
fn ends_sentence(token: &str) -> bool {
    token
        .trim_end_matches(['"', '\'', ')', ']', '»', '“', '”', '’'])
        .ends_with(['.', '?', '!', '…', '。', '？', '！'])
}

/// True when the engine aligned one word per token of the text.
fn aligned(s: &Segment, toks: &[(usize, usize)]) -> bool {
    !s.words.is_empty() && s.words.len() == toks.len()
}

/// Each token's time span: the aligned word's when the engine aligned one
/// word per token, else interpolated by character offset over the segment.
fn token_spans(s: &Segment, toks: &[(usize, usize)]) -> (Vec<(u64, u64)>, bool) {
    if aligned(s, toks) {
        let spans = s
            .words
            .iter()
            .map(|w| (w.start_ms, w.end_ms.max(w.start_ms)))
            .collect();
        return (spans, true);
    }
    let chars = s.text.chars().count().max(1) as u64;
    let length = s.end_ms.saturating_sub(s.start_ms);
    let at = |byte: usize| s.start_ms + length * s.text[..byte].chars().count() as u64 / chars;
    (toks.iter().map(|&(a, b)| (at(a), at(b))).collect(), false)
}

/// The pieces of segment `seg`: cuts after a sentence end, and at a gap
/// of at least `pause_ms` between aligned words.
fn pieces(s: &Segment, seg: usize, rule: &Rule) -> Vec<Piece> {
    let toks = tokens(&s.text);
    if toks.is_empty() {
        return vec![Piece {
            seg,
            tokens: (0, 0),
            span: (s.start_ms, s.end_ms),
            sentence_end: false,
        }];
    }
    let (spans, timed) = token_spans(s, &toks);
    let mut out = Vec::new();
    let mut first = 0;
    for i in 0..toks.len() {
        let last = i + 1 == toks.len();
        let sentence_end = ends_sentence(&s.text[toks[i].0..toks[i].1]);
        let pause = !last && timed && spans[i + 1].0.saturating_sub(spans[i].1) >= rule.pause_ms;
        if last || sentence_end || pause {
            let start = if first == 0 {
                s.start_ms
            } else {
                spans[first].0
            };
            let end = if last { s.end_ms } else { spans[i].1 };
            out.push(Piece {
                seg,
                tokens: (first, i),
                span: (start.min(end), end),
                sentence_end,
            });
            first = i + 1;
        }
    }
    out
}

fn overlap(a: (u64, u64), b: (u64, u64)) -> u64 {
    a.1.min(b.1).saturating_sub(a.0.max(b.0))
}

/// Milliseconds per speaker over `spans`, in order of first appearance.
fn tally(spans: impl Iterator<Item = (u64, u64)>, turns: &[SpeakerTurn]) -> Vec<(&str, u64)> {
    let mut out: Vec<(&str, u64)> = Vec::new();
    for span in spans {
        for t in turns {
            let o = overlap(span, (t.start_ms, t.end_ms));
            if o == 0 {
                continue;
            }
            match out.iter_mut().find(|(s, _)| *s == t.speaker) {
                Some((_, sum)) => *sum += o,
                None => out.push((&t.speaker, o)),
            }
        }
    }
    out
}

/// The speaker holding most of the tally (the earlier one on a tie).
fn best<'a>(tally: &[(&'a str, u64)]) -> Option<(&'a str, u64)> {
    let mut winner: Option<(&str, u64)> = None;
    for &(s, o) in tally {
        if winner.is_none_or(|(_, w)| o > w) {
            winner = Some((s, o));
        }
    }
    winner
}

/// The speaker of one unit: the most overlap when the winner holds at
/// least `min_speaker_share` of it, else (no overlap at all) the nearest
/// turn within `nearest_turn_ms`.
fn vote(unit: &[&Piece], turns: &[SpeakerTurn], rule: &Rule) -> Label {
    let counts = tally(unit.iter().map(|p| p.span), turns);
    if let Some((speaker, most)) = best(&counts) {
        let total: u64 = counts.iter().map(|(_, o)| o).sum();
        let share = most as f64 / total.max(1) as f64;
        return (share >= rule.min_speaker_share).then(|| (speaker.to_string(), share));
    }
    let span = (
        unit.iter().map(|p| p.span.0).min().unwrap_or(0),
        unit.iter().map(|p| p.span.1).max().unwrap_or(0),
    );
    turns
        .iter()
        .map(|t| {
            let distance = if t.end_ms <= span.0 {
                span.0 - t.end_ms
            } else {
                t.start_ms.saturating_sub(span.1)
            };
            (distance, t)
        })
        .filter(|(d, _)| *d <= rule.nearest_turn_ms)
        .min_by_key(|(d, _)| *d)
        .map(|(_, t)| (t.speaker.clone(), 0.0))
}

/// The units over `all` (pieces in stream order), as index runs: a
/// piece joins the unit before it unless a sentence ended there, or a
/// pause of at least `pause_ms` separates two different speakers.
fn units(all: &[Piece], turns: &[SpeakerTurn], rule: &Rule) -> Vec<Vec<usize>> {
    let alone: Vec<Option<&str>> = all
        .iter()
        .map(|p| best(&tally(std::iter::once(p.span), turns)).map(|(s, _)| s))
        .collect();
    let mut runs: Vec<Vec<usize>> = Vec::new();
    for (i, p) in all.iter().enumerate() {
        if let Some(run) = runs.last_mut() {
            let k = run[run.len() - 1];
            let paused = p.span.0.saturating_sub(all[k].span.1) >= rule.pause_ms;
            let changed = matches!((alone[k], alone[i]), (Some(a), Some(b)) if a != b);
            let cut = all[k].sentence_end || (paused && changed);
            if !cut {
                run.push(i);
                continue;
            }
        }
        runs.push(vec![i]);
    }
    runs
}

/// Labels the segments at `assigned` positions under the rule and
/// returns every segment with its label, split where a segment's pieces
/// took different speakers, plus how many segments were split. Segments
/// outside `assigned` keep the label `fixed` gives them.
pub(crate) fn label(
    segments: Vec<Segment>,
    assigned: &[bool],
    turns: &[SpeakerTurn],
    rule: &Rule,
    fixed: impl Fn(&Segment) -> Label,
) -> (Vec<(Segment, Label)>, usize) {
    let mut order: Vec<usize> = (0..segments.len()).filter(|&i| assigned[i]).collect();
    order.sort_by_key(|&i| (segments[i].start_ms, segments[i].end_ms, i));
    let all: Vec<Piece> = order
        .iter()
        .flat_map(|&i| pieces(&segments[i], i, rule))
        .collect();
    let mut labels: Vec<Label> = vec![None; all.len()];
    for unit in units(&all, turns, rule) {
        let label = vote(
            &unit.iter().map(|&k| &all[k]).collect::<Vec<_>>(),
            turns,
            rule,
        );
        for k in unit {
            labels[k] = label.clone();
        }
    }
    let mut own: Vec<Vec<usize>> = vec![Vec::new(); segments.len()];
    for (k, p) in all.iter().enumerate() {
        own[p.seg].push(k);
    }
    let mut out = Vec::with_capacity(segments.len());
    let mut split = 0;
    for (i, s) in segments.into_iter().enumerate() {
        if !assigned[i] {
            let l = fixed(&s);
            out.push((s, l));
            continue;
        }
        let runs = runs_of(own[i].iter().map(|&k| (&all[k], &labels[k])));
        if runs.len() > 1 {
            split += 1;
        }
        out.extend(split_segment(s, &runs));
    }
    (out, split)
}

/// Consecutive pieces with the same speaker, as `(first, last, label)`;
/// a run keeps its first piece's share.
fn runs_of<'a>(
    pieces: impl Iterator<Item = (&'a Piece, &'a Label)>,
) -> Vec<(&'a Piece, &'a Piece, Label)> {
    let speaker = |l: &Label| l.as_ref().map(|(s, _)| s.clone());
    let mut out: Vec<(&Piece, &Piece, Label)> = Vec::new();
    for (p, l) in pieces {
        match out.last_mut() {
            Some(run) if speaker(&run.2) == speaker(l) => run.1 = p,
            _ => out.push((p, p, l.clone())),
        }
    }
    out
}

/// One stored segment per run: the run's text, time and words; only the
/// first keeps the gap before it, and a split segment drops its polished
/// text for the caller to polish the parts again.
fn split_segment(s: Segment, runs: &[(&Piece, &Piece, Label)]) -> Vec<(Segment, Label)> {
    if runs.len() <= 1 {
        let label = runs.first().and_then(|r| r.2.clone());
        return vec![(s, label)];
    }
    let toks = tokens(&s.text);
    let timed = aligned(&s, &toks);
    let mut out = Vec::with_capacity(runs.len());
    for (r, (first, last, label)) in runs.iter().enumerate() {
        let start = if r == 0 { s.start_ms } else { first.span.0 };
        let end = if r + 1 == runs.len() {
            s.end_ms
        } else {
            last.span.1
        };
        let next_start = runs.get(r + 1).map_or(u64::MAX, |n| n.0.span.0);
        let words = if timed {
            s.words[first.tokens.0..=last.tokens.1].to_vec()
        } else {
            s.words
                .iter()
                .filter(|w| {
                    let mid = (w.start_ms + w.end_ms) / 2;
                    (r == 0 || mid >= start) && mid < next_start
                })
                .cloned()
                .collect()
        };
        let mut part = s.clone();
        part.start_ms = start;
        part.end_ms = end.max(start);
        part.text = s.text[toks[first.tokens.0].0..toks[last.tokens.1].1].to_string();
        part.words = words;
        part.gap_before_ms = if r == 0 { s.gap_before_ms } else { None };
        part.polished_text = None;
        out.push((part, label.clone()));
    }
    out
}

#[cfg(test)]
#[path = "sentences_tests.rs"]
mod tests;
