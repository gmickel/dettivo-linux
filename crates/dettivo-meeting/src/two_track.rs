//! The two-track speaker rules (ADR 0075) around the sentence rule of
//! `diarize::assign`. A two-track meeting records the user on the
//! microphone and the remote side on the system track, and four rules use
//! that:
//!
//! - **Bleed.** A microphone line on which the microphone is rarely both
//!   active and at least half as loud as the system track is the speakers
//!   heard through the microphone, or Whisper hearing words in silence,
//!   and is dropped.
//! - **Single remote.** When the engine found one remote speaker, every
//!   remote line takes that speaker, including the lines the sentence rule
//!   leaves unlabelled.
//! - **Shared microphone.** When the system track stayed silent while the
//!   microphone spoke, the pass diarizes the microphone instead, and
//!   labels it like room audio once the engine hears two or more voices.
//! - **Your own voice.** The microphone lines that carry the user's voice
//!   give this meeting's voiceprint, folded into the stored one; a remote
//!   line whose voice matches it is the user's.

use dettivo_engine_proto::SpeakerTurn;
use dettivo_proto::methods::meetings::{Segment, SegmentSource};

use crate::Track;
use crate::diarize::{self, Outcome, Rule, YOU};
use crate::levels::Levels;
use crate::voiceprint::{Voiceprint, centroid, cosine};

/// A line shorter than this gets no embedding: too little voice to tell.
pub const MIN_EMBED_MS: u64 = 1500;
/// A microphone line enrols the user's voice when this share of its frames
/// is voiced.
const ENROL_VOICED: f64 = 0.5;
/// The fewest enrolling lines that make a meeting's voiceprint.
const MIN_ENROL_LINES: usize = 3;

/// The rules' settings (`[meetings.diarization]`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rules {
    /// A microphone line with a smaller share of voiced frames is bleed
    /// and dropped; 0 turns the level gate off.
    pub bleed_min_voiced: f64,
    /// Label every remote line with the one remote speaker.
    pub single_remote: bool,
    /// Diarize the microphone when the system track stayed silent.
    pub shared_mic: bool,
    /// Build, store and use the user's voiceprint.
    pub voiceprint: bool,
    /// The least cosine similarity between a remote line's voice and the
    /// user's voiceprint that makes the line the user's.
    pub voice_match: f64,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            bleed_min_voiced: 0.05,
            single_remote: true,
            shared_mic: true,
            voiceprint: true,
            voice_match: 0.6,
        }
    }
}

/// What the rules read besides the segments and the turns.
#[derive(Debug, Clone, Default)]
pub struct Evidence<'a> {
    /// Both tracks' levels, for a two-track meeting.
    pub levels: Option<&'a Levels>,
    /// One embedding per input segment (the spans of [`embed_spans`]), or
    /// empty when none was computed.
    pub embeddings: Vec<Option<Vec<f32>>>,
    /// The stored voiceprint, when there is one.
    pub voiceprint: Option<&'a Voiceprint>,
}

/// What the rules did.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Report {
    /// Microphone lines the bleed gate dropped.
    pub dropped_bleed: usize,
    /// Every remote line took the one remote speaker.
    pub single_remote: bool,
    /// Remote lines relabelled as the user.
    pub you_relabelled: usize,
    /// This meeting's voiceprint, to fold into the stored one.
    pub enrolment: Option<Vec<f32>>,
}

/// The track the pass diarizes and whether that is a shared microphone.
pub fn plan(system_audio: bool, levels: Option<&Levels>, rules: &Rules) -> (Track, bool) {
    let shared = system_audio
        && rules.shared_mic
        && levels.is_some_and(Levels::system_silent_while_mic_speaks);
    let track = diarize::track_for(system_audio && !shared);
    (track, shared)
}

/// True when the diarized turns label every line like room audio: the
/// meeting has no system track, or its microphone is shared and the
/// engine heard two or more voices on it.
pub fn room_labelling(system_audio: bool, shared_mic: bool, turns: &[SpeakerTurn]) -> bool {
    let mut voices: Vec<&str> = turns.iter().map(|t| t.speaker.as_str()).collect();
    voices.sort_unstable();
    voices.dedup();
    !system_audio || (shared_mic && voices.len() >= 2)
}

/// The span each segment's embedding covers: the whole line, or none for
/// a line shorter than [`MIN_EMBED_MS`].
pub fn embed_spans(segments: &[Segment]) -> Vec<Option<(u64, u64)>> {
    segments
        .iter()
        .map(|s| {
            (s.end_ms.saturating_sub(s.start_ms) >= MIN_EMBED_MS).then_some((s.start_ms, s.end_ms))
        })
        .collect()
}

/// Drops the microphone lines the bleed gate catches, with their
/// embeddings.
fn bleed(
    segments: &mut Vec<Segment>,
    embeddings: &mut Vec<Option<Vec<f32>>>,
    levels: &Levels,
    min_voiced: f64,
) -> usize {
    let keep: Vec<bool> = segments
        .iter()
        .map(|s| {
            s.source_type != SegmentSource::Microphone
                || levels.voiced_share(s.start_ms, s.end_ms) >= min_voiced
        })
        .collect();
    let mut flags = keep.iter();
    segments.retain(|_| *flags.next().unwrap_or(&true));
    let mut flags = keep.iter();
    embeddings.retain(|_| *flags.next().unwrap_or(&true));
    keep.iter().filter(|k| !**k).count()
}

/// This meeting's voiceprint: the centroid of the microphone lines that
/// carry the user's voice, when there are enough of them.
fn enrolment(
    segments: &[Segment],
    embeddings: &[Option<Vec<f32>>],
    levels: Option<&Levels>,
) -> Option<Vec<f32>> {
    let lines: Vec<(&[f32], f32)> = segments
        .iter()
        .zip(embeddings)
        .filter(|(s, _)| s.source_type == SegmentSource::Microphone)
        .filter(|(s, _)| {
            levels.is_none_or(|l| l.voiced_share(s.start_ms, s.end_ms) >= ENROL_VOICED)
        })
        .filter_map(|(_, e)| e.as_deref().map(|e| (e, 1.0)))
        .collect();
    (lines.len() >= MIN_ENROL_LINES)
        .then(|| centroid(lines))
        .flatten()
}

/// Labels a meeting: the bleed gate, the sentence rule, the single-remote
/// shortcut and the user's voice. A room-audio meeting takes the sentence
/// rule alone.
pub fn label(
    segments: &mut Vec<Segment>,
    turns: &[SpeakerTurn],
    room_audio: bool,
    track_ms: u64,
    rule: &Rule,
    rules: &Rules,
    evidence: &Evidence<'_>,
) -> (Outcome, Report) {
    let mut report = Report::default();
    if room_audio {
        return (
            diarize::assign(segments, turns, true, track_ms, rule),
            report,
        );
    }
    let mut embeddings = if evidence.embeddings.len() == segments.len() {
        evidence.embeddings.clone()
    } else {
        vec![None; segments.len()]
    };
    if let Some(levels) = evidence.levels
        && rules.bleed_min_voiced > 0.0
    {
        report.dropped_bleed = bleed(segments, &mut embeddings, levels, rules.bleed_min_voiced);
    }
    if rules.voiceprint {
        report.enrolment = enrolment(segments, &embeddings, evidence.levels);
    }
    let mut out = diarize::assign(segments, turns, false, track_ms, rule);
    let embedding_of = |j: usize| {
        out.origins
            .get(j)
            .and_then(|&k| embeddings.get(k))
            .and_then(Option::as_deref)
    };
    let voice_of: Vec<Option<&[f32]>> = (0..segments.len()).map(embedding_of).collect();
    if rules.single_remote {
        report.single_remote = single_remote(segments, &mut out);
    }
    if rules.voiceprint {
        let reference = match (evidence.voiceprint, report.enrolment.as_deref()) {
            (Some(stored), Some(meeting)) => stored.blend(meeting),
            (Some(stored), None) => Some(stored.vector.clone()),
            (None, meeting) => meeting.map(<[f32]>::to_vec),
        };
        if let Some(reference) = reference {
            for (s, voice) in segments.iter_mut().zip(&voice_of) {
                let similarity = voice.map_or(0.0, |v| cosine(v, &reference));
                if s.source_type == SegmentSource::System && similarity >= rules.voice_match {
                    s.speaker_id = Some(YOU.into());
                    s.speaker_confidence = Some((similarity * 1000.0).round() / 1000.0);
                    report.you_relabelled += 1;
                }
            }
        }
    }
    retally(segments, &mut out);
    (out, report)
}

/// The shortcut: when the engine found one remote speaker, every remote
/// line takes that speaker. Line embeddings cannot second the engine here:
/// one voice's lines split as far apart as two voices' (ADR 0075).
fn single_remote(segments: &mut [Segment], out: &mut Outcome) -> bool {
    let mut remote = out.speakers.iter().filter(|sp| sp.speaker_id != YOU);
    let (Some(only), None) = (remote.next(), remote.next()) else {
        return false;
    };
    let only = only.speaker_id.clone();
    for s in segments
        .iter_mut()
        .filter(|s| s.source_type == SegmentSource::System && s.speaker_id.is_none())
    {
        s.speaker_id = Some(only.clone());
        s.speaker_confidence = Some(0.0);
    }
    true
}

/// Names every segment after its speaker, recounts the talk time and
/// numbers the segments again.
fn retally(segments: &mut [Segment], out: &mut Outcome) {
    for sp in &mut out.speakers {
        sp.talk_ms = 0;
    }
    for (index, s) in segments.iter_mut().enumerate() {
        s.index = index as u32;
        let speaker = s
            .speaker_id
            .as_deref()
            .and_then(|id| out.speakers.iter_mut().find(|sp| sp.speaker_id == id));
        match speaker {
            Some(sp) => {
                sp.talk_ms += s.end_ms.saturating_sub(s.start_ms);
                s.speaker = Some(sp.name.clone());
            }
            None => s.speaker = None,
        }
    }
}

#[cfg(test)]
#[path = "two_track_tests.rs"]
mod tests;
