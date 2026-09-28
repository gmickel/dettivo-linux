//! The voiceprint step under a Nemotron pass (ADR 0075): the embeddings
//! come from the sherpa-onnx `diarization-en` set when it is on disk, and
//! without it the pass labels the meeting with no voiceprint and no error.
//! Labelling never writes the print: the pass stores a meeting's voice only
//! in its ownership-checked commit, so a dropped result leaves it untouched.

use super::*;
use crate::diarization_choice::embedding_set;
use dettivo_meeting::voiceprint::FILE;
use dettivo_speech::diarize::{FALLBACK_MODEL, NEMOTRON_MODEL};

fn seg(start: u64, end: u64, source: SegmentSource) -> Segment {
    Segment {
        index: 0,
        start_ms: start,
        end_ms: end,
        text: "A line long enough to embed.".into(),
        speaker: None,
        speaker_id: None,
        speaker_confidence: None,
        source_type: source,
        words: Vec::new(),
        gap_before_ms: None,
        polished_text: None,
    }
}

/// A two-track meeting: three microphone lines that enrol the user and
/// one remote line.
fn meeting() -> MeetingRow {
    use SegmentSource::{Microphone as Mic, System as Sys};
    let mut row = MeetingRow::new();
    row.system_audio = true;
    row.segments = vec![
        seg(0, 2000, Mic),
        seg(2500, 4500, Mic),
        seg(5000, 7000, Mic),
        seg(8000, 10_000, Sys),
    ];
    row
}

/// Labels `meeting()` with `voice`, returning the row's speakers and the
/// stored print.
fn label(voice: Result<(&str, &Embed<'_>), &str>) -> (usize, Option<Voiceprint>) {
    let data = tempfile::tempdir().unwrap();
    let turns = [SpeakerTurn {
        start_ms: 8000,
        end_ms: 10_000,
        speaker: "A".into(),
    }];
    let tracks = Tracks {
        mic: vec![0; 16_000 * 10],
        system: vec![0; 16_000 * 10],
    };
    let labelling = Labelling {
        turns: &turns,
        room_audio: false,
        audio_ms: 10_000,
        rule: &Rule::default(),
        rules: &TwoTrack::default(),
        levels: None,
        tracks: &tracks,
        track: Track::System,
        data_dir: data.path(),
    };
    let mut row = meeting();
    let (out, _, pending) = label_row(&mut row, &labelling, voice);
    assert!(
        !data.path().join(FILE).exists(),
        "labelling alone never writes the voiceprint"
    );
    // The pass's ownership-checked commit stores the enrolment.
    if let Some(pending) = pending {
        pending.store(data.path(), &row.id);
    }
    let print =
        data.path().join(FILE).is_file().then(|| {
            Voiceprint::load(data.path(), FALLBACK_MODEL).expect("a diarization-en print")
        });
    (out.speakers.len(), print)
}

#[test]
fn nemotron_with_the_sherpa_set_on_disk_still_builds_the_voiceprint() {
    let set = embedding_set(NEMOTRON_MODEL, |_| true).unwrap();
    let embed = |_: &[i16], spans: &[(u64, u64)]| Ok(vec![Some(vec![1.0, 0.0]); spans.len()]);
    let (speakers, print) = label(Ok((set.as_str(), &embed)));
    let print = print.expect("the voiceprint is stored");
    assert_eq!(print.model, FALLBACK_MODEL);
    assert_eq!(print.meetings, 1);
    assert!(speakers >= 2, "the user and the remote speaker");
}

#[test]
fn nemotron_without_the_sherpa_set_labels_the_meeting_with_no_voiceprint() {
    let why = embedding_set(NEMOTRON_MODEL, |id| id == NEMOTRON_MODEL).unwrap_err();
    let (speakers, print) = label(Err(why.as_str()));
    assert!(print.is_none(), "no voiceprint without an embedding model");
    assert!(
        speakers >= 2,
        "the pass still labels the user and the remote speaker"
    );
}
