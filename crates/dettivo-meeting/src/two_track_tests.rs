//! The two-track rules' cases (ADR 0075): the bleed gate, the single-remote
//! shortcut, the shared-microphone switch on a
//! fixture meeting (and no switch on a normal one), and the user's voice.

use super::*;
use crate::diarize::read_track;

fn turn(start: u64, end: u64, speaker: &str) -> SpeakerTurn {
    SpeakerTurn {
        start_ms: start,
        end_ms: end,
        speaker: speaker.into(),
    }
}

fn seg(start: u64, end: u64, source: SegmentSource, text: &str) -> Segment {
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
        polished_text: None,
    }
}

use SegmentSource::{Microphone as Mic, System as Sys};

/// Levels with the microphone voiced over `voiced` and the system track
/// loud everywhere else, 10 ms frames over `ms`.
fn levels(ms: u64, voiced: &[(u64, u64)]) -> Levels {
    let n = (ms / 10) as usize;
    let on = |i: usize| {
        voiced
            .iter()
            .any(|&(a, b)| (a / 10) as usize <= i && i < (b / 10) as usize)
    };
    Levels {
        mic: (0..n).map(|i| if on(i) { -30.0 } else { -70.0 }).collect(),
        system: (0..n).map(|i| if on(i) { -70.0 } else { -25.0 }).collect(),
    }
}

fn ids(segments: &[Segment]) -> Vec<Option<&str>> {
    segments.iter().map(|s| s.speaker_id.as_deref()).collect()
}

#[test]
fn the_bleed_gate_drops_microphone_lines_the_user_did_not_voice() {
    let lv = levels(20_000, &[(0, 2000), (10_500, 12_000)]);
    let lines = || {
        vec![
            seg(0, 2000, Mic, "I think we should ship it."),
            // Whisper heard words while the microphone stayed quiet.
            seg(3000, 6000, Mic, "Thank you."),
            seg(7000, 10_000, Sys, "The numbers look good."),
            // Voiced over the remote side's quiet: the user.
            seg(10_500, 12_000, Mic, "Agreed."),
        ]
    };
    let evidence = Evidence {
        levels: Some(&lv),
        ..Default::default()
    };
    let turns = [turn(7000, 10_000, "A")];
    let rules = Rules::default();
    let run = |rules: &Rules, evidence: &Evidence<'_>| {
        let mut segments = lines();
        let (_, report) = label(
            &mut segments,
            &turns,
            false,
            20_000,
            &Rule::default(),
            rules,
            evidence,
        );
        (segments, report)
    };
    let (segments, report) = run(&rules, &evidence);
    assert_eq!(report.dropped_bleed, 1);
    let texts: Vec<&str> = segments.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "I think we should ship it.",
            "The numbers look good.",
            "Agreed."
        ]
    );
    assert_eq!(
        ids(&segments),
        [Some("you"), Some("speaker_00"), Some("you")]
    );
    assert_eq!(
        segments.iter().map(|s| s.index).collect::<Vec<_>>(),
        [0, 1, 2]
    );

    // Off, or without levels, nothing is dropped.
    let off = Rules {
        bleed_min_voiced: 0.0,
        ..rules
    };
    assert_eq!(run(&off, &evidence).0.len(), 4);
    assert_eq!(run(&rules, &Evidence::default()).0.len(), 4);
}

#[test]
fn a_single_remote_speaker_labels_every_remote_line() {
    let lines = || {
        vec![
            seg(0, 3000, Sys, "First."),
            // Beyond nearest_turn_ms of every turn: unlabelled by the rule.
            seg(40_000, 43_000, Sys, "Much later."),
            seg(1000, 2000, Mic, "Yes."),
        ]
    };
    let rule = Rule::default();
    let one = [turn(0, 3000, "A")];
    let mut plain = lines();
    diarize::assign(&mut plain, &one, false, 43_000, &rule);
    assert_eq!(
        plain[1].speaker_id, None,
        "the sentence rule leaves it blank"
    );

    let mut segments = lines();
    let (out, report) = label(
        &mut segments,
        &one,
        false,
        43_000,
        &rule,
        &Rules::default(),
        &Evidence::default(),
    );
    assert!(report.single_remote);
    assert_eq!(
        ids(&segments),
        [Some("speaker_00"), Some("speaker_00"), Some("you")]
    );
    assert_eq!(segments[1].speaker.as_deref(), Some("Speaker 1"));
    assert_eq!(out.speakers[1].talk_ms, 6000);

    // Two remote speakers: the sentence rule alone.
    let two = [turn(0, 3000, "A"), turn(40_000, 43_000, "B")];
    let mut segments = lines();
    let (out, report) = label(
        &mut segments,
        &two,
        false,
        43_000,
        &rule,
        &Rules::default(),
        &Evidence::default(),
    );
    assert!(!report.single_remote);
    assert_eq!(out.speakers.len(), 3);
    assert_eq!(segments[1].speaker_id.as_deref(), Some("speaker_01"));
    let off = Rules {
        single_remote: false,
        ..Rules::default()
    };
    let mut segments = lines();
    let (_, report) = label(
        &mut segments,
        &one,
        false,
        43_000,
        &rule,
        &off,
        &Evidence::default(),
    );
    assert!(!report.single_remote);
    assert_eq!(segments[1].speaker_id, None);
}

/// A meeting directory with the fixture's two voices on one track and
/// `other` on the other one.
fn fixture_meeting(voices_on: &str, other: impl Fn(usize) -> i16) -> tempfile::TempDir {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dettivo-qa/fixtures/diarization/two-speakers.wav");
    let pcm: Vec<i16> = hound::WavReader::open(&fixture)
        .unwrap()
        .into_samples::<i16>()
        .map(Result::unwrap)
        .collect();
    let dir = tempfile::tempdir().unwrap();
    let clock = std::time::Instant::now();
    for prefix in ["microphone", "system"] {
        let mut writer =
            dettivo_audio::takes::TakeWriter::with_prefix(dir.path(), prefix, clock).unwrap();
        writer.start_take(false).unwrap();
        if prefix == voices_on {
            writer.write(&pcm).unwrap();
        } else {
            writer
                .write(&(0..pcm.len()).map(&other).collect::<Vec<_>>())
                .unwrap();
        }
        writer.finish().unwrap();
    }
    dir
}

fn meeting_levels(dir: &std::path::Path) -> Levels {
    Levels::from_pcm(
        &read_track(dir, Track::Microphone).unwrap(),
        &read_track(dir, Track::System).unwrap(),
    )
}

#[test]
fn a_silent_system_track_switches_to_the_shared_microphone() {
    let rules = Rules::default();
    // Two people on the microphone, nothing on the system track.
    let shared = fixture_meeting("microphone", |_| 0);
    let lv = meeting_levels(shared.path());
    assert_eq!(plan(true, Some(&lv), &rules), (Track::Microphone, true));
    let turns = [
        turn(700, 3991, "SPEAKER_00"),
        turn(4200, 7000, "SPEAKER_01"),
    ];
    assert!(room_labelling(true, true, &turns));
    assert!(
        !room_labelling(true, true, &turns[..1]),
        "one voice on the microphone is you"
    );
    let mut segments = vec![seg(700, 3900, Mic, "One."), seg(4300, 6900, Mic, "Two.")];
    let (out, _) = label(
        &mut segments,
        &turns,
        true,
        7000,
        &Rule::default(),
        &rules,
        &Evidence::default(),
    );
    assert_eq!(ids(&segments), [Some("speaker_00"), Some("speaker_01")]);
    assert!(out.speakers.iter().all(|s| s.speaker_id != YOU));
    let off = Rules {
        shared_mic: false,
        ..rules
    };
    assert_eq!(plan(true, Some(&lv), &off), (Track::System, false));

    // A normal call: the remote voices on the system track, a quiet hiss
    // on the microphone. No switch.
    let normal = fixture_meeting("system", |i| if i % 2 == 0 { 20 } else { -20 });
    let lv = meeting_levels(normal.path());
    assert_eq!(plan(true, Some(&lv), &rules), (Track::System, false));
    assert!(!room_labelling(true, false, &turns));
    assert_eq!(
        plan(false, None, &rules),
        (Track::Microphone, false),
        "room audio stays"
    );
}

#[test]
fn the_users_voice_on_the_system_track_is_relabelled_you() {
    let user = Some(vec![1.0_f32, 0.0]);
    let remote = Some(vec![0.0_f32, 1.0]);
    let lines = || {
        vec![
            seg(0, 2000, Mic, "Hello there."),
            seg(3000, 5000, Mic, "Can you hear me?"),
            seg(6000, 8000, Mic, "Good."),
            seg(9000, 12_000, Sys, "Yes, loud and clear."),
            // The user's own voice came back on the system track.
            seg(13_000, 15_000, Sys, "Good."),
        ]
    };
    let turns = [turn(9000, 15_000, "A")];
    let embeddings = vec![
        user.clone(),
        user.clone(),
        user.clone(),
        remote.clone(),
        user.clone(),
    ];
    let rules = Rules {
        single_remote: false,
        ..Rules::default()
    };
    let mut segments = lines();
    let evidence = Evidence {
        embeddings: embeddings.clone(),
        ..Default::default()
    };
    let (out, report) = label(
        &mut segments,
        &turns,
        false,
        15_000,
        &Rule::default(),
        &rules,
        &evidence,
    );
    assert_eq!(report.you_relabelled, 1);
    assert_eq!(report.enrolment.as_deref(), Some([1.0, 0.0].as_slice()));
    assert_eq!(ids(&segments)[3..], [Some("speaker_00"), Some("you")]);
    assert_eq!(segments[4].speaker.as_deref(), Some("You"));
    assert_eq!(out.speakers[0].talk_ms, 8000);

    // Too few enrolling lines and no stored print: nothing to match. The
    // stored print alone is enough.
    let few = Evidence {
        embeddings: vec![user.clone(), None, None, remote.clone(), user.clone()],
        ..Default::default()
    };
    let mut segments = lines();
    let (_, report) = label(
        &mut segments,
        &turns,
        false,
        15_000,
        &Rule::default(),
        &rules,
        &few,
    );
    assert_eq!((report.you_relabelled, report.enrolment), (0, None));
    let stored = [1.0_f32, 0.0];
    let with_print = Evidence {
        voiceprint: Some(&stored),
        ..few
    };
    let mut segments = lines();
    let (_, report) = label(
        &mut segments,
        &turns,
        false,
        15_000,
        &Rule::default(),
        &rules,
        &with_print,
    );
    assert_eq!(report.you_relabelled, 1);
    let off = Rules {
        voiceprint: false,
        ..rules
    };
    let mut segments = lines();
    let (_, report) = label(
        &mut segments,
        &turns,
        false,
        15_000,
        &Rule::default(),
        &off,
        &evidence,
    );
    assert_eq!((report.you_relabelled, report.enrolment), (0, None));
    assert_eq!(
        embed_spans(&segments)[..3],
        [Some((0, 2000)), Some((3000, 5000)), Some((6000, 8000))]
    );
    assert_eq!(embed_spans(&[seg(0, 1000, Mic, "Hi.")]), [None]);
}
