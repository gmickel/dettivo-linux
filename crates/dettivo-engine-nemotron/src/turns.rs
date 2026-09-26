//! Turns from frame probabilities, by the post-processing fn-64 measured
//! (omarchy-meeting-recorder v1.1.0, ported in
//! `scripts/qa/nemotron3/nemotron3_diarize.py`): a channel speaks where its
//! probability is over 0.5, same-channel pauses under 500 ms are bridged,
//! runs under 300 ms are dropped, then the channels kept as speakers are
//! the expected count's largest or every channel over max(4 s, 4 % of the
//! speech), and the runs of the others go to the nearest kept run. Turns
//! are labelled `SPEAKER_00`, `SPEAKER_01`, ... in start order.

use std::collections::BTreeMap;

use dettivo_engine_proto::SpeakerTurn;

const THRESHOLD: f32 = 0.5;
const BRIDGE_MS: u64 = 500;
const MIN_MS: u64 = 300;
const FLOOR_MS: u64 = 4_000;
const FLOOR_PERCENT: u64 = 4;

/// A run: start and end in milliseconds, and the channel.
type Run = (u64, u64, usize);

/// Each channel's runs over the threshold, bridged and filtered; channel by
/// channel, each in time order.
fn runs(probs: &[f32], speakers: usize, frame_ms: u64) -> Vec<Run> {
    let frames = probs.len() / speakers.max(1);
    let mut out = Vec::new();
    for s in 0..speakers {
        let mut merged: Vec<(u64, u64)> = Vec::new();
        let mut start = None;
        for f in 0..=frames {
            let on = f < frames && probs[f * speakers + s] > THRESHOLD;
            match (on, start) {
                (true, None) => start = Some(f as u64 * frame_ms),
                (false, Some(a)) => {
                    let b = f as u64 * frame_ms;
                    match merged.last_mut() {
                        Some(last) if a - last.1 < BRIDGE_MS => last.1 = b,
                        _ => merged.push((a, b)),
                    }
                    start = None;
                }
                _ => {}
            }
        }
        out.extend(
            merged
                .into_iter()
                .filter(|(a, b)| b - a >= MIN_MS)
                .map(|(a, b)| (a, b, s)),
        );
    }
    out
}

/// The channels kept as speakers.
fn kept(raw: &[Run], expected: Option<u32>) -> Vec<usize> {
    let mut totals: BTreeMap<usize, u64> = BTreeMap::new();
    for (a, b, s) in raw {
        *totals.entry(*s).or_default() += b - a;
    }
    let keep: Vec<usize> = match expected {
        Some(n) => {
            let mut ranked: Vec<(usize, u64)> = totals.iter().map(|(s, t)| (*s, *t)).collect();
            ranked.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
            ranked
                .into_iter()
                .take(n.max(1) as usize)
                .map(|(s, _)| s)
                .collect()
        }
        None => {
            let floor = (totals.values().sum::<u64>() * FLOOR_PERCENT / 100).max(FLOOR_MS);
            totals
                .iter()
                .filter(|(_, t)| **t >= floor)
                .map(|(s, _)| *s)
                .collect()
        }
    };
    if keep.is_empty() {
        totals.into_keys().collect()
    } else {
        keep
    }
}

/// How far `mid` lies from a run: zero inside it.
fn distance(mid: u64, run: &Run) -> u64 {
    if mid < run.0 {
        run.0 - mid
    } else {
        mid.saturating_sub(run.1)
    }
}

/// The speaker turns from `probs` (`speakers` channels per frame of
/// `frame_ms`), capped at `expected` speakers when the count is known.
pub fn turns(
    probs: &[f32],
    speakers: usize,
    frame_ms: u64,
    expected: Option<u32>,
) -> Vec<SpeakerTurn> {
    let mut raw = runs(probs, speakers, frame_ms);
    let keep = kept(&raw, expected);
    let anchors: Vec<Run> = raw
        .iter()
        .filter(|r| keep.contains(&r.2))
        .copied()
        .collect();
    if !anchors.is_empty() {
        for run in &mut raw {
            if !keep.contains(&run.2) {
                let mid = (run.0 + run.1) / 2;
                let nearest = anchors
                    .iter()
                    .min_by_key(|a| distance(mid, a))
                    .expect("anchors is not empty");
                run.2 = nearest.2;
            }
        }
    }
    raw.sort_unstable();
    let mut order: Vec<usize> = Vec::new();
    raw.into_iter()
        .map(|(a, b, s)| {
            let label = order.iter().position(|x| *x == s).unwrap_or_else(|| {
                order.push(s);
                order.len() - 1
            });
            SpeakerTurn {
                start_ms: a,
                end_ms: b,
                speaker: format!("SPEAKER_{label:02}"),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Probabilities for `speakers` channels at 10 ms, with channel `s`
    /// active over each `[start_ms, end_ms)` span it is given.
    fn frames(total_ms: u64, speakers: usize, spans: &[(usize, u64, u64)]) -> Vec<f32> {
        let n = (total_ms / 10) as usize;
        let mut out = vec![0.0; n * speakers];
        for (s, a, b) in spans {
            for f in (*a / 10) as usize..(*b / 10) as usize {
                out[f * speakers + s] = 0.9;
            }
        }
        out
    }

    fn spans(turns: &[SpeakerTurn]) -> Vec<(u64, u64, &str)> {
        turns
            .iter()
            .map(|t| (t.start_ms, t.end_ms, t.speaker.as_str()))
            .collect()
    }

    #[test]
    fn pauses_under_half_a_second_bridge_and_short_runs_drop() {
        // A 400 ms pause bridges; a 600 ms one does not; a 200 ms blip drops.
        let probs = frames(
            20_000,
            2,
            &[
                (0, 0, 5_000),
                (0, 5_400, 9_000),
                (0, 9_600, 14_000),
                (1, 15_000, 15_200),
            ],
        );
        assert_eq!(
            spans(&turns(&probs, 2, 10, None)),
            [(0, 9_000, "SPEAKER_00"), (9_600, 14_000, "SPEAKER_00")]
        );
    }

    #[test]
    fn a_small_channel_joins_the_nearest_kept_speaker() {
        // Channel 2 speaks 1 s of 30 s, under the 4 s floor: its run goes to
        // channel 1, whose run it borders, not to channel 0 far away.
        let probs = frames(
            40_000,
            3,
            &[(0, 0, 12_000), (1, 20_000, 32_000), (2, 32_500, 33_500)],
        );
        assert_eq!(
            spans(&turns(&probs, 3, 10, None)),
            [
                (0, 12_000, "SPEAKER_00"),
                (20_000, 32_000, "SPEAKER_01"),
                (32_500, 33_500, "SPEAKER_01"),
            ]
        );
    }

    #[test]
    fn a_known_count_keeps_the_largest_channels() {
        let probs = frames(
            60_000,
            3,
            &[(0, 0, 10_000), (1, 10_000, 30_000), (2, 30_000, 45_000)],
        );
        let three = turns(&probs, 3, 10, None);
        assert_eq!(three.len(), 3);
        let two = turns(&probs, 3, 10, Some(2));
        // Channel 0 (10 s) is the smallest; its run joins channel 1 beside it.
        assert_eq!(
            spans(&two),
            [
                (0, 10_000, "SPEAKER_00"),
                (10_000, 30_000, "SPEAKER_00"),
                (30_000, 45_000, "SPEAKER_01"),
            ]
        );
    }

    #[test]
    fn silence_has_no_turns_and_only_small_channels_are_all_kept() {
        assert!(turns(&frames(5_000, 8, &[]), 8, 10, None).is_empty());
        // Both channels are under the 4 s floor, so both stay.
        let probs = frames(10_000, 2, &[(0, 0, 1_000), (1, 2_000, 3_000)]);
        assert_eq!(
            spans(&turns(&probs, 2, 10, None)),
            [(0, 1_000, "SPEAKER_00"), (2_000, 3_000, "SPEAKER_01")]
        );
    }
}
