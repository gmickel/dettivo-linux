//! The voice check (ADR 0076): a second opinion on each sentence unit's
//! speaker, after the sentence rule of `sentences.rs` has voted. Each unit
//! is embedded on the diarized track and compared by cosine with every
//! speaker's centroid, the mean voice of that speaker's confident units.
//! A unit moves to a closer speaker at any margin above `agree` when the
//! diarized turns inside it name that speaker too, and only by more than
//! `overrule` when the voice alone points there. Both margins shrink with
//! the recording's own typical margin, so two similar voices can still be
//! told apart. The method follows noScribe PR #351, including what it
//! measured as harmful: no per-word assignment and no smoothing over
//! neighbouring units.

use crate::voiceprint::{centroid, cosine};

/// A unit shorter than this is never moved and gets no embedding (the
/// engine's own floor): too little voice to tell.
pub const MIN_UNIT_MS: u64 = 500;
/// A unit this long or longer, held wholly by its speaker, shapes that
/// speaker's centroid and the recording's margin scale.
pub const CONFIDENT_MS: u64 = 1500;
/// The typical margin at which the margins apply unscaled.
const SCALE_REF: f64 = 0.5;
/// The margins never shrink below this share of their value.
const SCALE_FLOOR: f64 = 0.5;

/// The check's settings (`[meetings.diarization] voice_check`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// Run the check at all.
    pub on: bool,
    /// The cosine margin by which a unit must favour a speaker its turns
    /// also name before it moves there.
    pub agree: f64,
    /// The cosine margin by which a unit must favour a speaker nothing
    /// else names.
    pub overrule: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            on: true,
            agree: 0.15,
            overrule: 0.2,
        }
    }
}

/// One unit as the check sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    /// The unit's time on the diarized track.
    pub span: (u64, u64),
    /// The speaker the sentence rule gave it; an unlabelled unit is left.
    pub speaker: Option<String>,
    /// Milliseconds of each speaker's turns inside the unit.
    pub tally: Vec<(String, u64)>,
}

impl Unit {
    fn ms(&self) -> u64 {
        self.span.1.saturating_sub(self.span.0)
    }

    /// True when the unit is long and its turns name its speaker alone.
    fn confident(&self) -> bool {
        self.ms() >= CONFIDENT_MS
            && self.speaker.is_some()
            && self
                .tally
                .iter()
                .all(|(s, _)| Some(s) == self.speaker.as_ref())
    }
}

/// How much a unit sounds like each speaker; None when nothing tells.
pub type Scores = Option<Vec<(String, f64)>>;

/// Where the second opinion comes from.
pub trait Evidence {
    /// One score set per unit.
    fn scores(&mut self, units: &[Unit]) -> Vec<Scores>;
}

/// One embedding per span of the diarized track, None where none came.
pub type Embedder<'a> = dyn FnMut(&[(u64, u64)]) -> Vec<Option<Vec<f32>>> + 'a;

/// The voice as evidence: each unit's embedding against the centroids of
/// the speakers' confident units (all their units when none is).
pub struct Voices<'a, 'b> {
    /// Embeds the units' spans.
    pub embed: &'a mut Embedder<'b>,
}

impl Evidence for Voices<'_, '_> {
    fn scores(&mut self, units: &[Unit]) -> Vec<Scores> {
        let asked: Vec<usize> = (0..units.len())
            .filter(|&i| units[i].speaker.is_some() && units[i].ms() >= MIN_UNIT_MS)
            .collect();
        let mut voice: Vec<Option<Vec<f32>>> = vec![None; units.len()];
        if !asked.is_empty() {
            let spans: Vec<(u64, u64)> = asked.iter().map(|&i| units[i].span).collect();
            for (i, v) in asked.into_iter().zip((self.embed)(&spans)) {
                voice[i] = v;
            }
        }
        let mut speakers: Vec<&str> = units.iter().filter_map(|u| u.speaker.as_deref()).collect();
        speakers.sort_unstable();
        speakers.dedup();
        let centroids: Vec<(&str, Vec<f32>)> = speakers
            .into_iter()
            .filter_map(|s| {
                let of = |confident: bool| {
                    centroid(
                        units
                            .iter()
                            .zip(&voice)
                            .filter(|(u, _)| u.speaker.as_deref() == Some(s))
                            .filter(|(u, _)| !confident || u.confident())
                            .filter_map(|(_, v)| v.as_deref().map(|v| (v, 1.0))),
                    )
                };
                of(true).or_else(|| of(false)).map(|c| (s, c))
            })
            .collect();
        voice
            .iter()
            .map(|v| {
                let v = v.as_deref()?;
                Some(
                    centroids
                        .iter()
                        .map(|(s, c)| (s.to_string(), cosine(v, c)))
                        .collect(),
                )
            })
            .collect()
    }
}

/// What the check did.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Report {
    /// Units the evidence scored.
    pub checked: usize,
    /// Units that moved to another speaker.
    pub moved: usize,
    /// The recording's margin scale.
    pub scale: f64,
}

fn score(scores: &[(String, f64)], speaker: &str) -> Option<f64> {
    scores.iter().find(|(s, _)| s == speaker).map(|(_, v)| *v)
}

/// By how much this recording's margins shrink: the median margin by
/// which its confident units favour their own speaker, over `SCALE_REF`,
/// between `SCALE_FLOOR` and 1. Without such a unit the margins stay.
pub fn margin_scale(units: &[Unit], scores: &[Scores]) -> f64 {
    let mut own: Vec<f64> = units
        .iter()
        .zip(scores)
        .filter(|(u, _)| u.confident())
        .filter_map(|(u, s)| {
            let s = s.as_deref()?;
            let mine = score(s, u.speaker.as_deref()?)?;
            let other = s
                .iter()
                .filter(|(k, _)| Some(k) != u.speaker.as_ref())
                .map(|(_, v)| *v)
                .fold(f64::NEG_INFINITY, f64::max);
            (other.is_finite() && mine > other).then_some(mine - other)
        })
        .collect();
    if own.is_empty() {
        return 1.0;
    }
    own.sort_by(f64::total_cmp);
    let mid = own.len() / 2;
    let typical = if own.len() % 2 == 1 {
        own[mid]
    } else {
        (own[mid - 1] + own[mid]) / 2.0
    };
    (typical / SCALE_REF).clamp(SCALE_FLOOR, 1.0)
}

/// The speaker a unit should carry, under margins already scaled: the
/// speaker its turns name besides its own (the runner-up by time) when
/// the voice favours it by more than `agree`, then the closest voice when
/// it wins by more than `overrule`.
pub fn decide(unit: &Unit, scores: &[(String, f64)], agree: f64, overrule: f64) -> Option<String> {
    let current = unit.speaker.as_deref()?;
    let Some(mut at) = score(scores, current) else {
        return Some(current.to_string());
    };
    let mut label = current;
    let named = unit
        .tally
        .iter()
        .filter(|(s, _)| s != current)
        .max_by_key(|(_, ms)| *ms);
    if let Some((named, _)) = named
        && let Some(v) = score(scores, named)
        && v - at > agree
    {
        label = named;
        at = v;
    }
    if let Some((best, v)) = scores.iter().max_by(|a, b| a.1.total_cmp(&b.1))
        && best != label
        && v - at > overrule
    {
        label = best;
    }
    Some(label.to_string())
}

/// The check over a recording's units: for each unit, the speaker it
/// moves to (None where it stays).
pub fn check(
    units: &[Unit],
    evidence: &mut dyn Evidence,
    settings: &Settings,
) -> (Vec<Option<String>>, Report) {
    let mut report = Report {
        scale: 1.0,
        ..Report::default()
    };
    if !settings.on || units.is_empty() {
        return (vec![None; units.len()], report);
    }
    let scores = evidence.scores(units);
    report.scale = margin_scale(units, &scores);
    let (agree, overrule) = (
        settings.agree * report.scale,
        settings.overrule * report.scale,
    );
    let moves = units
        .iter()
        .zip(&scores)
        .map(|(u, s)| {
            let s = s.as_deref().filter(|_| u.ms() >= MIN_UNIT_MS)?;
            report.checked += 1;
            let to = decide(u, s, agree, overrule)?;
            (u.speaker.as_deref() != Some(to.as_str())).then(|| {
                report.moved += 1;
                to
            })
        })
        .collect();
    (moves, report)
}

#[cfg(test)]
#[path = "voice_check_tests.rs"]
mod tests;
