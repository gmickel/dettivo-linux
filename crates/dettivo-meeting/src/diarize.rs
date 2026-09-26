//! The post-meeting speaker pass (FR-G4, ADR 0035): the track the pass
//! reads (the system track, or every microphone take of a room-audio
//! meeting, on the meeting clock with the gaps as silence), and the rule
//! that turns the engine's speaker turns into labels on the finalised
//! segments. The rule (ADR 0072, `sentences.rs`) gives every sentence
//! the speaker holding most of it, so a remote line is unlabelled only
//! when no turn lies within `nearest_turn_ms` of it. Microphone segments
//! of a two-track meeting are `You`. The engine's labels are renumbered
//! in order of first appearance (`speaker_00` is whoever spoke first), so
//! a re-run maps onto the same ids, and the speakers come back with their
//! talk time and a swatch index in that order, `you` first.

use std::path::Path;

use dettivo_audio::takes::{Takes, read_sidecar};
use dettivo_engine_proto::SpeakerTurn;
use dettivo_proto::methods::meetings::{Segment, SegmentSource};
use dettivo_proto::methods::speakers::{Speaker, default_label};

use crate::{Track, sentences};

/// The speaker id of the microphone.
pub const YOU: &str = "you";

/// The assignment rule (`[meetings.diarization]`, ADR 0072).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rule {
    /// A pause between two aligned words at least this long ends a unit
    /// where the diarized speaker differs across it.
    pub pause_ms: u64,
    /// A unit no turn overlaps takes the nearest turn within this.
    pub nearest_turn_ms: u64,
    /// The least share of a unit's diarized speech its winner must hold;
    /// below it the unit stays unlabelled.
    pub min_speaker_share: f64,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            pause_ms: 250,
            nearest_turn_ms: 10_000,
            min_speaker_share: 0.0,
        }
    }
}

/// What the pass produced for a meeting.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Outcome {
    /// The speakers, `you` first, then in order of first appearance.
    pub speakers: Vec<Speaker>,
    /// The share of the track inside a speaker turn.
    pub coverage: f64,
    /// How many segments the rule split in two or more.
    pub split: usize,
}

/// The track a meeting diarizes: the system track when one recorded, the
/// microphone (room audio) otherwise.
pub fn track_for(system_audio: bool) -> Track {
    if system_audio {
        Track::System
    } else {
        Track::Microphone
    }
}

/// The sidecar a track's takes are listed in.
fn sidecar(dir: &Path, track: Track) -> std::path::PathBuf {
    match track {
        Track::Microphone => dir.join(dettivo_audio::takes::SIDECAR),
        Track::System => dir.join(format!("{}-takes.json", track.prefix())),
    }
}

/// Every take of `track` under `dir` on the meeting clock, the gaps as
/// silence: one 16 kHz mono buffer from the meeting's start to the last
/// take's end. A meeting without a sidecar (an import) has its one file
/// under the track's name.
pub fn read_track(dir: &Path, track: Track) -> Result<Vec<i16>, String> {
    let sidecar_path = sidecar(dir, track);
    let single = dir.join(format!("{}.wav", track.prefix()));
    let takes: Takes = if !sidecar_path.is_file() && single.is_file() {
        Takes {
            takes: vec![dettivo_audio::takes::Take {
                index: 1,
                file: format!("{}.wav", track.prefix()),
                start_offset_ms: 0,
                start_offset_ns: 0,
                gap_before: false,
                samples: 1,
            }],
        }
    } else {
        read_sidecar(&sidecar_path).map_err(|e| {
            format!(
                "{}: {e}",
                sidecar_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            )
        })?
    };
    let mut inputs = Vec::new();
    let mut total = 0;
    for take in &takes.takes {
        if take.samples == 0 {
            continue;
        }
        let path = dir.join(&take.file);
        let reader = hound::WavReader::open(&path).map_err(|e| format!("{}: {e}", take.file))?;
        let spec = reader.spec();
        if spec.channels != 1
            || spec.sample_rate != 16_000
            || spec.bits_per_sample != 16
            || spec.sample_format != hound::SampleFormat::Int
        {
            return Err(format!(
                "{}: expected 16 kHz mono signed 16-bit PCM",
                take.file
            ));
        }
        let offset = take
            .start_offset_ms
            .checked_mul(16)
            .ok_or("diarization track offset overflow")?;
        let end = offset
            .checked_add(u64::from(reader.duration()))
            .ok_or("diarization track duration overflow")?;
        dettivo_engine_proto::validate_pcm_samples(end).map_err(|e| e.to_string())?;
        total = total.max(end);
        inputs.push((take, reader, offset as usize, end as usize));
    }
    let mut pcm = vec![0; total as usize];
    for (take, reader, offset, end) in inputs {
        for (dest, sample) in pcm[offset..end]
            .iter_mut()
            .zip(reader.into_samples::<i16>())
        {
            *dest = sample.map_err(|e| format!("{}: {e}", take.file))?;
        }
    }
    Ok(pcm)
}

/// The speaker id of the `n`th speaker to appear: `speaker_00`, ...
pub fn speaker_id(n: usize) -> String {
    format!("speaker_{n:02}")
}

/// The engine's turns with their labels renumbered in order of first
/// appearance, so `speaker_00` is whoever spoke first.
pub fn renumbered(turns: &[SpeakerTurn]) -> Vec<SpeakerTurn> {
    let mut sorted: Vec<&SpeakerTurn> = turns.iter().collect();
    sorted.sort_by_key(|t| (t.start_ms, t.end_ms));
    let mut order: Vec<String> = Vec::new();
    for t in &sorted {
        if !order.contains(&t.speaker) {
            order.push(t.speaker.clone());
        }
    }
    sorted
        .into_iter()
        .map(|t| SpeakerTurn {
            start_ms: t.start_ms,
            end_ms: t.end_ms,
            speaker: speaker_id(order.iter().position(|s| s == &t.speaker).unwrap_or(0)),
        })
        .collect()
}

/// Milliseconds of `span` inside any turn (overlapping turns count once).
fn covered(span: (u64, u64), turns: &[SpeakerTurn]) -> u64 {
    let mut pieces: Vec<(u64, u64)> = turns
        .iter()
        .map(|t| (t.start_ms.max(span.0), t.end_ms.min(span.1)))
        .filter(|(s, e)| e > s)
        .collect();
    pieces.sort_unstable();
    let mut total = 0;
    let mut end = 0;
    for (s, e) in pieces {
        let s = s.max(end);
        if e > s {
            total += e - s;
            end = e;
        }
    }
    total
}

/// Labels `segments` from the engine's `turns` under the rule, splitting
/// a segment whose sentences took different speakers into one segment
/// per speaker (renumbered in order; a split part carries no polished
/// text until the caller polishes it). Every segment is assigned from the
/// turns in a room-audio meeting; otherwise the microphone segments are
/// `you` and the system segments are assigned. `track_ms` is the length
/// of the diarized track, for the coverage figure.
pub fn assign(
    segments: &mut Vec<Segment>,
    turns: &[SpeakerTurn],
    room_audio: bool,
    track_ms: u64,
    rule: &Rule,
) -> Outcome {
    let turns = &renumbered(turns);
    let mut speakers: Vec<Speaker> = Vec::new();
    // Every diarized speaker is listed in order of first appearance, even
    // one whose turns cover no segment, so the swatches stay stable.
    let mut ordered: Vec<(u64, String)> = Vec::new();
    for t in turns {
        match ordered.iter_mut().find(|(_, s)| s == &t.speaker) {
            Some((start, _)) => *start = (*start).min(t.start_ms),
            None => ordered.push((t.start_ms, t.speaker.clone())),
        }
    }
    ordered.sort();
    if !room_audio {
        speakers.push(Speaker {
            speaker_id: YOU.into(),
            name: default_label(YOU),
            color_index: 0,
            talk_ms: 0,
        });
    }
    for (_, id) in ordered {
        let color_index = speakers.len() as u32;
        speakers.push(Speaker {
            speaker_id: id.clone(),
            name: default_label(&id),
            color_index,
            talk_ms: 0,
        });
    }
    let is_you = |s: &Segment| !room_audio && s.source_type == SegmentSource::Microphone;
    let assigned: Vec<bool> = segments.iter().map(|s| !is_you(s)).collect();
    let (labelled, split) =
        sentences::label(std::mem::take(segments), &assigned, turns, rule, |_| {
            Some((YOU.to_string(), 1.0))
        });
    for (index, (mut s, label)) in labelled.into_iter().enumerate() {
        s.index = index as u32;
        match label {
            Some((id, share)) => {
                if let Some(sp) = speakers.iter_mut().find(|sp| sp.speaker_id == id) {
                    sp.talk_ms += s.end_ms.saturating_sub(s.start_ms);
                    s.speaker = Some(sp.name.clone());
                }
                s.speaker_id = Some(id);
                s.speaker_confidence = Some((share * 1000.0).round() / 1000.0);
            }
            None => {
                s.speaker = None;
                s.speaker_id = None;
                s.speaker_confidence = None;
            }
        }
        segments.push(s);
    }
    let coverage = if track_ms == 0 {
        0.0
    } else {
        (covered((0, track_ms), turns) as f64 / track_ms as f64).min(1.0)
    };
    Outcome {
        speakers,
        coverage: (coverage * 1000.0).round() / 1000.0,
        split,
    }
}

/// Carries the names of `previous` speakers onto `speakers` by id when
/// the diarized speaker count matches (a re-run after renames); the
/// segments' `speaker` text follows. Returns true when the names carried.
pub fn carry_names(
    speakers: &mut [Speaker],
    segments: &mut [Segment],
    previous: &[Speaker],
) -> bool {
    let count = |list: &[Speaker]| list.iter().filter(|s| s.speaker_id != YOU).count();
    if previous.is_empty() || count(previous) != count(speakers) {
        return false;
    }
    for sp in speakers.iter_mut() {
        if let Some(old) = previous.iter().find(|o| o.speaker_id == sp.speaker_id) {
            sp.name = old.name.clone();
        }
    }
    for s in segments.iter_mut() {
        if let Some(id) = &s.speaker_id
            && let Some(sp) = speakers.iter().find(|sp| &sp.speaker_id == id)
        {
            s.speaker = Some(sp.name.clone());
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(start: u64, end: u64, speaker: &str) -> SpeakerTurn {
        SpeakerTurn {
            start_ms: start,
            end_ms: end,
            speaker: speaker.into(),
        }
    }

    fn seg(index: u32, start: u64, end: u64, source: SegmentSource) -> Segment {
        Segment {
            index,
            start_ms: start,
            end_ms: end,
            text: format!("segment {index}"),
            speaker: None,
            speaker_id: None,
            speaker_confidence: None,
            source_type: source,
            words: Vec::new(),
            gap_before_ms: None,
            polished_text: None,
        }
    }

    #[test]
    fn every_remote_segment_gets_a_speaker_and_the_microphone_is_you() {
        let turns = vec![
            turn(0, 4000, "SPEAKER_01"),
            turn(4300, 7000, "SPEAKER_00"),
            turn(7300, 11000, "SPEAKER_01"),
        ];
        let mut segments = vec![
            seg(0, 100, 3900, SegmentSource::System),
            // Straddles two turns evenly: the earlier speaker, half share.
            seg(1, 2500, 5800, SegmentSource::System),
            // Outside every turn, 500 ms after one: the nearest turn.
            seg(2, 11500, 20000, SegmentSource::System),
            seg(3, 500, 1500, SegmentSource::Microphone),
            // Seven tenths inside one speaker's turn.
            seg(4, 6500, 8500, SegmentSource::System),
        ];
        for s in &mut segments {
            s.text.push('.');
        }
        let out = assign(&mut segments, &turns, false, 20_000, &Rule::default());
        let ids: Vec<&str> = out.speakers.iter().map(|s| s.speaker_id.as_str()).collect();
        assert_eq!(
            ids,
            ["you", "speaker_00", "speaker_01"],
            "first appearance order"
        );
        assert_eq!(out.speakers[0].color_index, 0);
        assert_eq!(out.speakers[2].color_index, 2);
        assert_eq!(segments.len(), 5, "one sentence each: nothing split");
        assert_eq!(out.split, 0);
        assert_eq!(segments[0].speaker_id.as_deref(), Some("speaker_00"));
        assert_eq!(segments[0].speaker.as_deref(), Some("Speaker 1"));
        assert_eq!(segments[0].speaker_confidence, Some(1.0));
        assert_eq!(segments[1].speaker_id.as_deref(), Some("speaker_00"));
        assert_eq!(segments[1].speaker_confidence, Some(0.5));
        assert_eq!(segments[2].speaker_id.as_deref(), Some("speaker_00"));
        assert_eq!(segments[2].speaker_confidence, Some(0.0), "no overlap");
        assert_eq!(segments[3].speaker_id.as_deref(), Some("you"));
        assert_eq!(segments[3].speaker.as_deref(), Some("You"));
        assert_eq!(segments[4].speaker_id.as_deref(), Some("speaker_00"));
        assert_eq!(segments[4].speaker_confidence, Some(0.706));
        assert_eq!(out.speakers[0].talk_ms, 1000);
        assert_eq!(out.speakers[1].talk_ms, 3800 + 3300 + 8500 + 2000);
        assert_eq!(out.speakers[2].talk_ms, 0);
        assert!((out.coverage - 0.52).abs() < 0.001, "{}", out.coverage);
    }

    #[test]
    fn room_audio_assigns_every_segment_and_names_carry_over_by_count() {
        let turns = vec![turn(0, 5000, "SPEAKER_00"), turn(5000, 9000, "SPEAKER_01")];
        let mut segments = vec![
            seg(0, 0, 4000, SegmentSource::Microphone),
            seg(1, 5500, 8500, SegmentSource::Microphone),
        ];
        let mut out = assign(&mut segments, &turns, true, 9000, &Rule::default());
        assert_eq!(out.speakers.len(), 2);
        assert_eq!(out.speakers[0].speaker_id, "speaker_00");
        assert_eq!(segments[0].speaker_id.as_deref(), Some("speaker_00"));
        assert_eq!(segments[1].speaker.as_deref(), Some("Speaker 2"));
        assert_eq!(out.coverage, 1.0);
        let previous = vec![
            Speaker {
                speaker_id: "speaker_00".into(),
                name: "Ada".into(),
                color_index: 0,
                talk_ms: 1,
            },
            Speaker {
                speaker_id: "speaker_01".into(),
                name: "Grace".into(),
                color_index: 1,
                talk_ms: 1,
            },
        ];
        assert!(carry_names(&mut out.speakers, &mut segments, &previous));
        assert_eq!(segments[1].speaker.as_deref(), Some("Grace"));
        assert_eq!(out.speakers[0].name, "Ada");
        let one = vec![previous[0].clone()];
        assert!(!carry_names(&mut out.speakers, &mut segments, &one));
        let renum = renumbered(&[turn(500, 900, "SPEAKER_07"), turn(0, 400, "SPEAKER_02")]);
        assert_eq!(renum[0].speaker, "speaker_00");
        assert_eq!(renum[1].speaker, "speaker_01");
        assert_eq!(speaker_id(7), "speaker_07");
        assert_eq!(track_for(true), Track::System);
        assert_eq!(track_for(false), Track::Microphone);
    }

    #[test]
    fn a_track_is_read_onto_the_meeting_clock_with_silent_gaps() {
        let dir = tempfile::tempdir().unwrap();
        let clock = std::time::Instant::now();
        let mut sys =
            dettivo_audio::takes::TakeWriter::with_prefix(dir.path(), "system", clock).unwrap();
        sys.start_take(false).unwrap();
        sys.write(&[7; 1600]).unwrap();
        sys.finish().unwrap();
        let pcm = read_track(dir.path(), Track::System).unwrap();
        assert!(pcm.len() >= 1600, "{}", pcm.len());
        assert_eq!(pcm[pcm.len() - 1], 7);
        assert!(pcm.iter().take(pcm.len() - 1600).all(|s| *s == 0));
        assert!(read_track(dir.path(), Track::Microphone).is_err());
        // An import keeps one file under the track's name and no sidecar.
        std::fs::copy(
            dir.path().join("system.wav"),
            dir.path().join("microphone.wav"),
        )
        .unwrap();
        assert_eq!(
            read_track(dir.path(), Track::Microphone).unwrap().len(),
            pcm.len()
        );
    }
}
