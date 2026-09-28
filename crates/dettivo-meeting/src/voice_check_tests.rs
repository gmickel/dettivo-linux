//! The voice check's margins (ADR 0076): the "diarization agrees" and
//! "voice only" branches, the per-recording scale, and a moved unit
//! through `diarize::assign`.

use dettivo_engine_proto::SpeakerTurn;
use dettivo_proto::methods::meetings::{Segment, SegmentSource};

use super::*;
use crate::diarize::{Rule, assign};

fn unit(span: (u64, u64), speaker: &str, tally: &[(&str, u64)]) -> Unit {
    Unit {
        span,
        speaker: Some(speaker.into()),
        tally: tally.iter().map(|(s, ms)| (s.to_string(), *ms)).collect(),
    }
}

fn scores(pairs: &[(&str, f64)]) -> Vec<(String, f64)> {
    pairs.iter().map(|(s, v)| (s.to_string(), *v)).collect()
}

/// Fixed scores per unit, as a stand-in for any evidence.
struct Given(Vec<Scores>);

impl Evidence for Given {
    fn scores(&mut self, _: &[Unit]) -> Vec<Scores> {
        self.0.clone()
    }
}

#[test]
fn a_unit_moves_at_any_margin_where_its_turns_agree_and_by_the_overrule_margin_alone() {
    // A holds most of the unit's time; B holds some of it.
    let shared = unit((0, 1000), "A", &[("A", 700), ("B", 300)]);
    let close = scores(&[("A", 0.50), ("B", 0.51), ("C", 0.40)]);
    assert_eq!(decide(&shared, &close, 0.0, 0.12).as_deref(), Some("B"));
    // The same voice with no B turn inside: 0.01 is under the overrule margin.
    let alone = unit((0, 1000), "A", &[("A", 1000)]);
    assert_eq!(decide(&alone, &close, 0.0, 0.12).as_deref(), Some("A"));
    // A voice far closer to a speaker no turn names moves the unit anyway.
    let far = scores(&[("A", 0.30), ("B", 0.35), ("C", 0.50)]);
    assert_eq!(decide(&alone, &far, 0.0, 0.12).as_deref(), Some("C"));
    // Exactly at the margin is not over it.
    let edge = scores(&[("A", 0.25), ("C", 0.375)]);
    assert_eq!(decide(&alone, &edge, 0.0, 0.125).as_deref(), Some("A"));
    // After the turns' speaker, the overrule margin counts from it.
    let both = scores(&[("A", 0.20), ("B", 0.30), ("C", 0.40)]);
    assert_eq!(decide(&shared, &both, 0.0, 0.12).as_deref(), Some("B"));
    // Nothing to compare the unit's own speaker with: it stays.
    let blind = scores(&[("B", 0.9)]);
    assert_eq!(decide(&shared, &blind, 0.0, 0.12).as_deref(), Some("A"));
    let unlabelled = Unit {
        speaker: None,
        ..shared.clone()
    };
    assert_eq!(decide(&unlabelled, &close, 0.0, 0.12), None);
}

#[test]
fn the_margins_shrink_with_the_recordings_typical_margin_within_the_floor() {
    let long = |s: &str| unit((0, CONFIDENT_MS), s, &[(s, CONFIDENT_MS)]);
    let units = vec![
        long("A"),
        long("B"),
        long("A"),
        unit((0, 800), "A", &[("A", 800)]),
    ];
    // Own margins 0.2, 0.4 and 0.3 on the long units; the short one's 0.9
    // does not count. The median 0.3 over 0.5 scales the margins by 0.6.
    let given = vec![
        Some(scores(&[("A", 0.6), ("B", 0.4)])),
        Some(scores(&[("A", 0.1), ("B", 0.5)])),
        Some(scores(&[("A", 0.7), ("B", 0.4)])),
        Some(scores(&[("A", 1.0), ("B", 0.1)])),
    ];
    assert!((margin_scale(&units, &given) - 0.6).abs() < 1e-9);
    // Voices that barely differ stop at the floor; clear ones at 1.
    let similar = vec![Some(scores(&[("A", 0.51), ("B", 0.5)])); 3];
    assert_eq!(margin_scale(&units[..3], &similar), 0.5);
    let clear = vec![Some(scores(&[("A", 0.9), ("B", 0.1)])); 3];
    assert_eq!(margin_scale(&units[..3], &clear), 1.0);
    // Nothing to go by keeps the margins.
    assert_eq!(margin_scale(&units[3..], &given[3..]), 1.0);

    // Under the 0.6 scale the default overrule of 0.2 needs 0.12: a voice
    // 0.15 closer to B moves a unit it would not move unscaled.
    // A 300 ms unit never moves, whatever its scores.
    let mut probe = units.clone();
    probe.push(unit((5000, 5600), "A", &[("A", 600)]));
    probe.push(unit((6000, 6300), "A", &[("A", 300)]));
    let mut all = given.clone();
    all.push(Some(scores(&[("A", 0.3), ("B", 0.45)])));
    all.push(Some(scores(&[("A", 0.0), ("B", 0.9)])));
    let (moves, report) = check(&probe, &mut Given(all), &Settings::default());
    assert!((report.scale - 0.6).abs() < 1e-9);
    assert_eq!(moves[4].as_deref(), Some("B"));
    assert_eq!(moves[5], None);
    assert_eq!((report.checked, report.moved), (5, 1));
    let off = Settings {
        on: false,
        ..Settings::default()
    };
    let (moves, report) = check(&probe, &mut Given(vec![]), &off);
    assert!(moves.iter().all(Option::is_none) && report.checked == 0);
}

#[test]
fn centroids_come_from_confident_units_and_short_units_go_unembedded() {
    let units = vec![
        unit((0, 2000), "A", &[("A", 2000)]),
        unit((2000, 4000), "B", &[("B", 2000)]),
        // Long but shared with A: not confident, so B's centroid ignores it.
        unit((4000, 6000), "B", &[("B", 1200), ("A", 800)]),
        unit((6000, 6300), "A", &[("A", 300)]),
    ];
    let mut asked = Vec::new();
    let mut embed = |spans: &[(u64, u64)]| {
        asked.extend_from_slice(spans);
        vec![
            Some(vec![1.0, 0.0]),
            Some(vec![0.0, 1.0]),
            Some(vec![1.0, 0.0]),
        ]
    };
    let s = Voices { embed: &mut embed }.scores(&units);
    assert_eq!(
        asked,
        [(0, 2000), (2000, 4000), (4000, 6000)],
        "not the 300 ms unit"
    );
    assert_eq!(s[3], None);
    let third = s[2].as_ref().unwrap();
    assert!((score(third, "A").unwrap() - 1.0).abs() < 1e-6);
    assert!(score(third, "B").unwrap().abs() < 1e-6);
}

#[test]
fn a_unit_no_turn_overlaps_is_never_confident() {
    // Labelled by the nearest turn: long, but nothing inside names A.
    let nearest = unit((0, 2000), "A", &[]);
    assert!(!nearest.confident());
    assert!(unit((0, 2000), "A", &[("A", 2000)]).confident());
    // So it shapes neither the margin scale nor A's centroid.
    let given = vec![Some(scores(&[("A", 0.9), ("B", 0.1)]))];
    assert_eq!(margin_scale(std::slice::from_ref(&nearest), &given), 1.0);
    let units = vec![nearest, unit((2000, 4000), "A", &[("A", 2000)])];
    let mut embed = |_: &[(u64, u64)]| vec![Some(vec![1.0, 0.0]), Some(vec![0.0, 1.0])];
    let s = Voices { embed: &mut embed }.scores(&units);
    assert!((score(s[1].as_ref().unwrap(), "A").unwrap() - 1.0).abs() < 1e-6);
}

fn remote(start: u64, end: u64, text: &str) -> Segment {
    Segment {
        index: 0,
        start_ms: start,
        end_ms: end,
        text: text.into(),
        speaker: None,
        speaker_id: None,
        speaker_confidence: None,
        source_type: SegmentSource::System,
        words: Vec::new(),
        gap_before_ms: None,
        polished_text: None,
    }
}

fn turn(start: u64, end: u64, speaker: &str) -> SpeakerTurn {
    SpeakerTurn {
        start_ms: start,
        end_ms: end,
        speaker: speaker.into(),
    }
}

#[test]
fn a_sentence_that_sounds_like_the_other_speaker_is_split_off_to_them() {
    // The engine gives the whole stretch to A, with a sliver of B inside
    // the second sentence; that sentence's voice is B's.
    let turns = [
        turn(0, 9000, "A"),
        turn(3500, 3700, "B"),
        turn(9000, 12_000, "B"),
    ];
    let mut segments = vec![
        remote(0, 6000, "The first thought. A second one."),
        remote(9000, 12_000, "And then the other voice."),
    ];
    let mut embed = |spans: &[(u64, u64)]| {
        spans
            .iter()
            .map(|&(start, _)| {
                Some(if start < 3000 {
                    vec![1.0, 0.0]
                } else {
                    vec![0.1, 1.0]
                })
            })
            .collect()
    };
    let out = assign(
        &mut segments,
        &turns,
        false,
        12_000,
        &Rule::default(),
        Some(&mut Voices { embed: &mut embed }),
    );
    let ids: Vec<Option<&str>> = segments.iter().map(|s| s.speaker_id.as_deref()).collect();
    assert_eq!(
        ids,
        [Some("speaker_00"), Some("speaker_01"), Some("speaker_01")]
    );
    assert_eq!(segments[1].text, "A second one.");
    assert_eq!(out.split, 1);
    assert_eq!(out.voice.moved, 1);
    assert!(
        segments[1].speaker_confidence.unwrap() < 0.2,
        "B held a sliver"
    );
    let mut plain = vec![remote(0, 6000, "The first thought. A second one.")];
    let off = Rule {
        voice_check: Settings {
            on: false,
            ..Settings::default()
        },
        ..Rule::default()
    };
    let out = assign(
        &mut plain,
        &turns,
        false,
        12_000,
        &off,
        Some(&mut Voices { embed: &mut embed }),
    );
    assert_eq!(
        (plain.len(), out.voice.checked),
        (1, 0),
        "off leaves the vote"
    );
}
