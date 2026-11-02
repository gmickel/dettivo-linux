//! Settling suspects against the take decoded without the prompt: the
//! two decodes are lined up on the words they share, and a suspect's gap
//! between shared words becomes the prompt-free decode's bytes there.

use dettivo_engine_proto::sound::alike;

use super::split::{Word, split};
use super::{Finding, MIN_SOUND_ALIKE, Suspect};

/// The longest common subsequence of `a` and `b` as matched index pairs,
/// matching as few of the `soft` words of `a` as a longest one allows: a
/// suspect lines up with a word of `b` only when no other word of `a`
/// could take that place, so a repeated word confirms the occurrence
/// the prompt-free decode heard, not the suspect.
fn lcs(a: &[&str], b: &[&str], soft: &[bool]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    // Each match scores n + 1, less one when its word is soft: more
    // matches always win, fewer soft ones break a tie.
    let weight = n as u64 + 1;
    let mut score = vec![vec![0u64; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let skip = score[i + 1][j].max(score[i][j + 1]);
            score[i][j] = if a[i] == b[j] {
                skip.max(score[i + 1][j + 1] + weight - u64::from(soft[i]))
            } else {
                skip
            };
        }
    }
    let (mut i, mut j, mut pairs) = (0, 0, Vec::new());
    while i < n && j < m {
        if a[i] == b[j] && score[i][j] == score[i + 1][j + 1] + weight - u64::from(soft[i]) {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if score[i][j] == score[i + 1][j] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

/// The bytes of `texts` under `words[from..to]`: each segment's run as
/// written, runs of different segments joined by a space.
fn bytes_of(texts: &[String], words: &[Word], from: usize, to: usize) -> String {
    let mut out = String::new();
    let mut w = from;
    while w < to {
        let seg = words[w].seg;
        let mut last = w;
        while last + 1 < to && words[last + 1].seg == seg {
            last += 1;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&texts[seg][words[w].start..words[last].end]);
        w = last + 1;
    }
    out
}

/// The segment texts once every suspect is settled against `plain`, the
/// segment texts of the prompt-free decode; `None` when every suspect
/// stands. Words both decodes share anchor the two; a suspect stands when
/// its words anchor one after another. Otherwise its gap between anchors
/// becomes the prompt-free decode's words in that gap, unless both gaps
/// spell the same letters, or the suspect is a lone name and their gap
/// sounds like it. Only the gaps change: every other byte stays, and the
/// texts come back trimmed, a segment left with nothing empty.
pub fn settle(finding: &Finding, plain: &[String]) -> Option<Vec<String>> {
    let words = &finding.words;
    let theirs = split(plain);
    let ours: Vec<&str> = words.iter().map(|w| w.norm.as_str()).collect();
    let their_refs: Vec<&str> = theirs.iter().map(|w| w.norm.as_str()).collect();
    let mut soft = vec![false; words.len()];
    for s in &finding.suspects {
        soft[s.span.first..=s.span.last].fill(true);
    }
    let mut anchor = vec![None; words.len()];
    for (i, j) in lcs(&ours, &their_refs, &soft) {
        anchor[i] = Some(j);
    }
    // (our gap, their gap), each a half-open word range.
    let mut gaps: Vec<((usize, usize), (usize, usize))> = Vec::new();
    for &Suspect { span: s, lone } in &finding.suspects {
        let heard_whole = anchor[s.first]
            .is_some_and(|j0| (s.first..=s.last).all(|w| anchor[w] == Some(j0 + (w - s.first))));
        if heard_whole {
            continue;
        }
        let before = (0..s.first).rev().find_map(|w| anchor[w].map(|j| (w, j)));
        let after = (s.last + 1..words.len()).find_map(|w| anchor[w].map(|j| (w, j)));
        let ours = (
            before.map_or(0, |b| b.0 + 1),
            after.map_or(words.len(), |a| a.0),
        );
        let their = (
            before.map_or(0, |b| b.1 + 1),
            after.map_or(theirs.len(), |a| a.1),
        );
        let letters = |w: &[Word]| w.iter().map(|w| w.norm.as_str()).collect::<String>();
        let their_letters = letters(&theirs[their.0..their.1]);
        if letters(&words[ours.0..ours.1]) == their_letters {
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
        gaps.push((ours, their));
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
    // Byte edits per segment: (start, end, replacement). A gap across
    // segments puts its words in the first and empties the rest.
    let mut edits: Vec<Vec<(usize, usize, String)>> = vec![Vec::new(); finding.texts.len()];
    for (ours, their) in merged {
        let mut replacement = Some(bytes_of(plain, &theirs, their.0, their.1));
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
                let (start, end) = if with.is_empty() {
                    swallow_space(&text, start, end)
                } else {
                    (start, end)
                };
                text.replace_range(start..end, &with);
            }
            text.trim().to_string()
        })
        .collect();
    Some(texts)
}

/// The range `start..end` of `text` widened over the whitespace before it,
/// or after it at the start of the text, so removing it leaves one space
/// between its neighbours, not two.
fn swallow_space(text: &str, start: usize, end: usize) -> (usize, usize) {
    let before = text[..start].trim_end_matches(char::is_whitespace).len();
    if before < start && before > 0 {
        return (before, end);
    }
    let after = text.len() - text[end..].trim_start_matches(char::is_whitespace).len();
    (start, after)
}
