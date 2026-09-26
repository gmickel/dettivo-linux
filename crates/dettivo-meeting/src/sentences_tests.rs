//! The sentence rule's cases (ADR 0072), through `diarize::assign`.

use dettivo_engine_proto::SpeakerTurn;
use dettivo_proto::methods::meetings::{Segment, SegmentSource, Word};

use crate::diarize::{Rule, assign};

fn turn(start: u64, end: u64, speaker: &str) -> SpeakerTurn {
    SpeakerTurn {
        start_ms: start,
        end_ms: end,
        speaker: speaker.into(),
    }
}

fn seg(start: u64, end: u64, text: &str, source: SegmentSource) -> Segment {
    Segment {
        index: 0,
        start_ms: start,
        end_ms: end,
        text: text.into(),
        speaker: None,
        speaker_id: None,
        speaker_confidence: None,
        source_type: source,
        words: Vec::new(),
        gap_before_ms: None,
        polished_text: Some(format!("{text} (polished)")),
    }
}

fn remote(start: u64, end: u64, text: &str) -> Segment {
    seg(start, end, text, SegmentSource::System)
}

/// A remote segment with one aligned word per token at the given times.
fn aligned(text: &str, times: &[(u64, u64)]) -> Segment {
    let mut s = remote(times[0].0, times[times.len() - 1].1, text);
    s.words = text
        .split_whitespace()
        .zip(times)
        .map(|(w, &(a, b))| Word {
            start_ms: a,
            end_ms: b,
            text: w.into(),
            confidence: 0.9,
        })
        .collect();
    s
}

fn ids(segments: &[Segment]) -> Vec<Option<&str>> {
    segments.iter().map(|s| s.speaker_id.as_deref()).collect()
}

fn texts(segments: &[Segment]) -> Vec<&str> {
    segments.iter().map(|s| s.text.as_str()).collect()
}

fn run(segments: &mut Vec<Segment>, turns: &[SpeakerTurn], rule: &Rule) -> usize {
    assign(segments, turns, false, 20_000, rule).split
}

#[test]
fn a_sentence_end_splits_a_segment_between_speakers_and_a_sentence_spans_segments() {
    let turns = vec![
        turn(0, 600, "A"),
        turn(600, 4000, "B"),
        turn(5000, 6200, "A"),
        turn(6200, 12_500, "B"),
    ];
    let mut first = remote(0, 4000, "Yes. I think we should go.");
    first.gap_before_ms = Some(300);
    let mut segments = vec![
        first,
        // One sentence over two segments: its speaker is the sentence's.
        remote(5000, 6000, "and then we"),
        remote(6000, 9000, "leave together."),
        // Two sentences by one speaker stay one segment.
        remote(10_000, 12_000, "Right. Okay."),
    ];
    assert_eq!(run(&mut segments, &turns, &Rule::default()), 1);
    assert_eq!(
        texts(&segments),
        [
            "Yes.",
            "I think we should go.",
            "and then we",
            "leave together.",
            "Right. Okay."
        ]
    );
    let a = Some("speaker_00");
    let b = Some("speaker_01");
    assert_eq!(ids(&segments), [a, b, b, b, b]);
    let indexes: Vec<u32> = segments.iter().map(|s| s.index).collect();
    assert_eq!(indexes, [0, 1, 2, 3, 4]);
    // The parts share the segment's span at the interpolated cut.
    assert_eq!((segments[0].start_ms, segments[0].end_ms), (0, 615));
    assert_eq!((segments[1].start_ms, segments[1].end_ms), (769, 4000));
    assert_eq!(segments[0].gap_before_ms, Some(300));
    assert_eq!(segments[1].gap_before_ms, None);
    assert!(segments[0].polished_text.is_none() && segments[1].polished_text.is_none());
    assert_eq!(
        segments[4].polished_text.as_deref(),
        Some("Right. Okay. (polished)")
    );
}

#[test]
fn a_pause_cuts_only_where_the_speaker_changes_across_it() {
    let times = [(0, 300), (300, 700), (1100, 1500), (1500, 3000)];
    let changed = [turn(0, 900, "A"), turn(900, 3000, "B")];
    let mut segments = vec![aligned("we go then stop", &times)];
    assert_eq!(run(&mut segments, &changed, &Rule::default()), 1);
    assert_eq!(texts(&segments), ["we go", "then stop"]);
    assert_eq!(ids(&segments), [Some("speaker_00"), Some("speaker_01")]);
    assert_eq!((segments[0].start_ms, segments[0].end_ms), (0, 700));
    assert_eq!((segments[1].start_ms, segments[1].end_ms), (1100, 3000));
    assert_eq!(segments[0].words.len(), 2);
    assert_eq!(segments[1].words[0].text, "then");

    // The same pause with one speaker on both sides cuts nothing.
    let mut segments = vec![aligned("we go then stop", &times)];
    assert_eq!(
        run(&mut segments, &[turn(0, 3000, "A")], &Rule::default()),
        0
    );
    assert_eq!(texts(&segments), ["we go then stop"]);

    // A speaker change without a pause or a sentence end never splits the
    // sentence: the whole of it takes the speaker holding most of it.
    let close = [(0, 300), (300, 700), (800, 1100), (1100, 3000)];
    let mut segments = vec![aligned("we go then stop", &close)];
    assert_eq!(run(&mut segments, &changed, &Rule::default()), 0);
    assert_eq!(ids(&segments), [Some("speaker_01")]);
}

#[test]
fn a_line_outside_every_turn_takes_the_nearest_within_the_tolerance() {
    let turns = [turn(0, 2000, "A")];
    let lines = || {
        vec![
            remote(11_000, 12_000, "Still there?"),
            remote(13_000, 14_000, "Anyone?"),
        ]
    };
    let mut segments = lines();
    run(&mut segments, &turns, &Rule::default());
    assert_eq!(
        ids(&segments),
        [Some("speaker_00"), None],
        "9 s in, 11 s out"
    );
    assert_eq!(segments[0].speaker_confidence, Some(0.0));
    assert_eq!(segments[1].speaker_confidence, None);
    let tight = Rule {
        nearest_turn_ms: 500,
        ..Rule::default()
    };
    let mut segments = lines();
    run(&mut segments, &turns, &tight);
    assert_eq!(ids(&segments), [None, None]);
}

#[test]
fn a_one_word_sentence_keeps_its_own_speaker() {
    // ADR 0072: folding short fragments into a neighbour raised the
    // bench's error, so a fragment votes like any other sentence.
    let turns = [
        turn(0, 3000, "A"),
        turn(3000, 3400, "B"),
        turn(3400, 6000, "A"),
    ];
    let mut segments = vec![
        remote(0, 3000, "We should ship on Friday."),
        remote(3000, 3400, "Yeah."),
        remote(3400, 6000, "And tell the team."),
    ];
    run(&mut segments, &turns, &Rule::default());
    assert_eq!(
        ids(&segments),
        [Some("speaker_00"), Some("speaker_01"), Some("speaker_00")]
    );
}

#[test]
fn a_share_floor_leaves_a_mixed_sentence_unlabelled() {
    let turns = [turn(0, 1000, "A"), turn(1000, 2000, "B")];
    let mut segments = vec![remote(0, 2000, "Half and half.")];
    run(&mut segments, &turns, &Rule::default());
    assert_eq!(segments[0].speaker_confidence, Some(0.5));
    let strict = Rule {
        min_speaker_share: 0.6,
        ..Rule::default()
    };
    let mut segments = vec![remote(0, 2000, "Half and half.")];
    run(&mut segments, &turns, &strict);
    assert_eq!(ids(&segments), [None]);
}

#[test]
fn room_audio_assigns_the_microphone_and_a_microphone_only_meeting_is_you() {
    let turns = [turn(0, 2000, "A"), turn(2000, 5000, "B")];
    let mic = |start, end, text| seg(start, end, text, SegmentSource::Microphone);
    // Room audio: the microphone carries every voice and is split like
    // the system track.
    let mut segments = vec![mic(0, 5000, "Hello there. How are you doing today?")];
    let out = assign(&mut segments, &turns, true, 5000, &Rule::default());
    assert_eq!(ids(&segments), [Some("speaker_00"), Some("speaker_01")]);
    assert_eq!(out.speakers.len(), 2, "no `you` in a room");

    // A two-track meeting whose remote side said nothing: every line is
    // You, whatever the turns say, and nothing is split.
    for turns in [&turns[..], &[]] {
        let mut segments = vec![
            mic(0, 5000, "Hello there. How are you doing today?"),
            mic(6000, 7000, "Fine."),
        ];
        let out = assign(&mut segments, turns, false, 7000, &Rule::default());
        assert_eq!(ids(&segments), [Some("you"), Some("you")]);
        assert_eq!(out.split, 0);
        assert_eq!(out.speakers[0].talk_ms, 6000);
    }
}

#[test]
fn a_rerun_relabels_an_old_meeting_without_losing_text_or_words() {
    let turns = [
        turn(0, 600, "A"),
        turn(600, 4000, "B"),
        turn(5000, 5900, "A"),
        turn(5900, 8000, "B"),
    ];
    // As the coverage and share rule left it: the straddling lines blank.
    let mut old = remote(0, 4000, "Yes. I think we should go.");
    old.speaker_id = None;
    let mut labelled = remote(4000, 4500, "Sure.");
    labelled.speaker_id = Some("speaker_01".into());
    labelled.speaker = Some("Speaker 2".into());
    let mut segments = vec![
        old,
        labelled,
        aligned(
            "we go then stop",
            &[(5000, 5300), (5300, 5700), (6100, 6500), (6500, 8000)],
        ),
        seg(4200, 4800, "Mm.", SegmentSource::Microphone),
    ];
    let text_before: Vec<String> = segments.iter().map(|s| s.text.clone()).collect();
    let words_before: Vec<Word> = segments.iter().flat_map(|s| s.words.clone()).collect();
    assert_eq!(run(&mut segments, &turns, &Rule::default()), 2);
    assert!(
        segments.iter().all(|s| s.speaker_id.is_some()),
        "{segments:?}"
    );
    let joined = |t: Vec<&str>| t.join(" ");
    assert_eq!(
        joined(texts(&segments)),
        joined(text_before.iter().map(String::as_str).collect())
    );
    let words_after: Vec<Word> = segments.iter().flat_map(|s| s.words.clone()).collect();
    assert_eq!(words_after, words_before);
    // A second run over its own output changes nothing.
    let settled = segments.clone();
    assert_eq!(run(&mut segments, &turns, &Rule::default()), 0);
    assert_eq!(segments, settled);
}
