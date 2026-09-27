//! The speaker pass thread (ADR 0035): reads both tracks, picks the one to
//! diarize (the microphone when it was shared, ADR 0075), sends it through
//! the engine, labels the segments under the sentence rule (ADR 0072) and
//! the voice check on each sentence (ADR 0076) and the two-track rules,
//! folds the user's voice into the voiceprint (both voices from the voice
//! engine, the sherpa-onnx set whichever set diarized), polishes
//! the parts it split, and commits the result through the meeting's job
//! entry. The service that starts and tracks passes is `diarization.rs`.

use std::path::Path;

use dettivo_engine_proto::SpeakerTurn;
use dettivo_meeting::Track;
use dettivo_meeting::diarize::{self, Outcome, Rule};
use dettivo_meeting::levels::Levels;
use dettivo_meeting::two_track::{self, Evidence, Report, Rules as TwoTrack};
use dettivo_meeting::voice_check::{Evidence as UnitEvidence, Voices};
use dettivo_meeting::voiceprint::Voiceprint;
use dettivo_proto::methods::meetings::{Segment, SegmentSource};
use dettivo_proto::methods::speakers::DiarizationStatus;
use dettivo_speech::EngineError;
use dettivo_speech::diarize::{DiarizeRequest, timeout_for};
use dettivo_storage::meetings::MeetingRow;

use crate::diarization::{
    Diarization, Pass, STAGE, model_missing, publish_progress, publish_state,
};
use crate::meetings_archive::MeetingArchive;

#[cfg(test)]
#[path = "diarization_run_tests.rs"]
mod tests;

/// The engine's last redacted stderr line, for the row.
fn last_line(tail: &str) -> String {
    tail.lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("engine crashed")
        .to_string()
}

/// Both tracks on the meeting clock: the system track (required when the
/// meeting recorded one) and the microphone (required for room audio,
/// optional beside a system track).
struct Tracks {
    mic: Vec<i16>,
    system: Vec<i16>,
}

fn read_tracks(dir: Option<&str>, system_audio: bool) -> Result<Tracks, String> {
    let dir = Path::new(dir.ok_or("the meeting keeps no audio")?);
    let mic = diarize::read_track(dir, Track::Microphone);
    if !system_audio {
        return Ok(Tracks {
            mic: mic?,
            system: Vec::new(),
        });
    }
    let system = diarize::read_track(dir, Track::System)?;
    Ok(Tracks {
        mic: mic.unwrap_or_default(),
        system,
    })
}

/// One embedding per span of a track: the voice engine's `embed`.
type Embed<'a> = dyn Fn(&[i16], &[(u64, u64)]) -> Result<Vec<Option<Vec<f32>>>, EngineError> + 'a;

/// One embedding per segment, each on its own track; none where the
/// engine could not embed (an older engine, a failure), which leaves the
/// voice rules idle.
fn embeddings(embed: &Embed<'_>, segments: &[Segment], tracks: &Tracks) -> Vec<Option<Vec<f32>>> {
    let spans = two_track::embed_spans(segments);
    let mut out = vec![None; segments.len()];
    for (source, pcm) in [
        (SegmentSource::Microphone, &tracks.mic),
        (SegmentSource::System, &tracks.system),
    ] {
        let mine: Vec<usize> = (0..segments.len())
            .filter(|&i| segments[i].source_type == source && spans[i].is_some())
            .collect();
        if mine.is_empty() || pcm.is_empty() {
            continue;
        }
        let asked: Vec<(u64, u64)> = mine.iter().filter_map(|&i| spans[i]).collect();
        match embed(pcm, &asked) {
            Ok(vectors) => {
                for (i, v) in mine.into_iter().zip(vectors) {
                    out[i] = v;
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "speaker embeddings unavailable; the voiceprint rule is idle");
                return Vec::new();
            }
        }
    }
    out
}

/// What the labelling reads besides the row and the voice engine.
struct Labelling<'a> {
    turns: &'a [SpeakerTurn],
    room_audio: bool,
    audio_ms: u64,
    rule: &'a Rule,
    rules: &'a TwoTrack,
    levels: Option<&'a Levels>,
    tracks: &'a Tracks,
    /// The track the engine diarized, where the voice check embeds.
    track: Track,
    data_dir: &'a Path,
}

/// Labels the row's segments under the voice check and the two-track
/// rules and folds this meeting into the stored voiceprint. `voice` is the
/// embedding set's id (which keys the print) with its embedder, or why this
/// meeting has none: the voice rules then stay idle and the labelling goes
/// on without them.
fn label_row(
    row: &mut MeetingRow,
    l: &Labelling<'_>,
    voice: Result<(&str, &Embed<'_>), &str>,
) -> (Outcome, Report, Option<Enrolment>) {
    let voice = match voice {
        Ok(voice) => Some(voice),
        Err(why) => {
            tracing::debug!(reason = %why, "voiceprint and voice check skipped for this meeting");
            None
        }
    };
    let print = voice.filter(|_| l.rules.voiceprint && !l.room_audio);
    let stored = print.and_then(|(model, _)| Voiceprint::load(l.data_dir, model));
    let evidence = Evidence {
        levels: l.levels,
        embeddings: print.map_or_else(Vec::new, |(_, embed)| {
            embeddings(embed, &row.segments, l.tracks)
        }),
        voiceprint: stored.as_ref(),
    };
    let diarized = match l.track {
        Track::System => &l.tracks.system,
        Track::Microphone => &l.tracks.mic,
    };
    let mut unit_voices = |spans: &[(u64, u64)]| match voice {
        Some((_, embed)) => embed(diarized, spans).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "sentence embeddings unavailable; the voice check is idle");
            vec![None; spans.len()]
        }),
        None => vec![None; spans.len()],
    };
    let mut voices = Voices {
        embed: &mut unit_voices,
    };
    let check: Option<&mut dyn UnitEvidence> =
        (voice.is_some() && l.rule.voice_check.on).then_some(&mut voices);
    let (out, report) = two_track::label(
        &mut row.segments,
        l.turns,
        l.room_audio,
        l.audio_ms,
        l.rule,
        l.rules,
        &evidence,
        check,
    );
    let pending = match (print, &report.enrolment) {
        (Some((model, _)), Some(meeting)) => Some(Enrolment {
            model: model.to_string(),
            meeting: meeting.clone(),
        }),
        _ => None,
    };
    (out, report, pending)
}

/// A meeting's voice, folded into the user's print only once the pass
/// that measured it still owns the meeting (see `run`'s commit).
struct Enrolment {
    model: String,
    meeting: Vec<f32>,
}

impl Enrolment {
    fn store(&self, data_dir: &Path, meeting_id: &str) {
        if let Err(e) = Voiceprint::enrol(data_dir, &self.model, meeting_id, &self.meeting) {
            tracing::warn!(error = %e, "voiceprint not stored");
        }
    }
}

/// The pass thread: the track, the engine, the assignment, the row.
pub(crate) fn run(service: &Diarization, pass: Pass) {
    let Pass {
        job_id,
        mut row,
        audio_dir,
        engine,
        voice,
        model_id,
        fallback_reason,
        rule,
        two_track: rules,
        data_dir,
        polish,
        speakers,
        clustering_threshold,
        cancel,
        history,
        bus,
    } = pass;
    let mut block = row.diarization.clone().unwrap_or_default();
    block.status = DiarizationStatus::Running;
    block.error = None;
    block.engine = Some(engine.binary().into());
    block.model = Some(format!("diarize/{model_id}"));
    block.fallback_reason = fallback_reason;
    row.diarization = Some(block.clone());
    if let Err(e) = history.with_store(|s| s.update_meeting(&row)) {
        tracing::warn!(error = %e, "history: diarization block not stored");
    }
    publish_state(&bus, &row, DiarizationStatus::Running);
    publish_progress(&bus, &job_id, (0, 0), STAGE);
    let mut tracks = read_tracks(audio_dir.as_deref(), row.system_audio);
    let levels = match &tracks {
        Ok(t) if row.system_audio && !t.mic.is_empty() => Some(Levels::from_pcm(&t.mic, &t.system)),
        _ => None,
    };
    let (track, shared_mic) = two_track::plan(row.system_audio, levels.as_ref(), &rules);
    let mut enrolment: Option<Enrolment> = None;
    let outcome = match &mut tracks {
        Err(e) => Err((format!("audio: {e}"), false)),
        Ok(t) => {
            let chosen = match track {
                Track::System => &mut t.system,
                Track::Microphone => &mut t.mic,
            };
            let audio_ms = chosen.len() as u64 * 1000 / 16_000;
            let mut request = DiarizeRequest {
                pcm: std::mem::take(chosen),
                speakers,
                clustering_threshold: Some(clustering_threshold),
            };
            let mut progress = |done: u32, total: u32| {
                service.note_progress(&row.id, (done, total));
                publish_progress(&bus, &job_id, (done, total), STAGE);
            };
            let outcome = engine
                .diarize(&request, timeout_for(audio_ms), &cancel, &mut progress)
                .map(|r| (r, audio_ms))
                .map_err(|e| match e {
                    EngineError::ModelMissing(m) => (m, true),
                    EngineError::Crashed(tail) => (last_line(&tail), false),
                    EngineError::Cancelled => ("cancelled".into(), false),
                    other => (other.to_string(), false),
                });
            *chosen = std::mem::take(&mut request.pcm);
            outcome
        }
    };
    let tracks = tracks.unwrap_or(Tracks {
        mic: Vec::new(),
        system: Vec::new(),
    });
    // The row may have changed (a rename, a delete) while the pass ran.
    let current = match history.with_store(|s| s.get_meeting(&row.id)) {
        Ok(Some(current)) => current,
        _ => {
            tracing::info!(job = %job_id, "meeting gone while diarizing; result dropped");
            publish_progress(&bus, &job_id, (0, 0), "cancelled");
            return;
        }
    };
    row = current;
    let previous = row.speakers.clone();
    let (status, stage, chunks) = match outcome {
        Ok((result, audio_ms)) => {
            let room_audio = two_track::room_labelling(row.system_audio, shared_mic, &result.turns);
            // One voice on a shared microphone is the user: no turns name
            // the (silent) system track's lines.
            let turns = if shared_mic && !room_audio {
                Vec::new()
            } else {
                result.turns
            };
            let labelling = Labelling {
                turns: &turns,
                room_audio,
                audio_ms,
                rule: &rule,
                rules: &rules,
                levels: levels.as_ref(),
                tracks: &tracks,
                track,
                data_dir: &data_dir,
            };
            let embed;
            let voice_in: Result<(&str, &Embed<'_>), &str> = match &voice {
                Ok(v) => {
                    embed = |pcm: &[i16], spans: &[(u64, u64)]| {
                        v.engine.embed(pcm, spans, timeout_for(audio_ms))
                    };
                    Ok((v.model_id.as_str(), &embed))
                }
                Err(why) => Err(why.as_str()),
            };
            let (out, report, pending) = label_row(&mut row, &labelling, voice_in);
            enrolment = pending;
            if report.dropped_bleed > 0 {
                row.raw_text = row
                    .segments
                    .iter()
                    .map(|s| s.text.trim())
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                row.final_text = row.polished_join();
            }
            if out.split > 0 {
                MeetingArchive::polish_missing(&polish, &mut row);
            }
            let mut speakers = out.speakers;
            diarize::carry_names(&mut speakers, &mut row.segments, &previous);
            row.speakers = speakers;
            block.status = DiarizationStatus::Ready;
            block.coverage = Some(out.coverage);
            block.error = None;
            tracing::info!(
                job = %job_id,
                speakers = row.speakers.len(),
                turns = turns.len(),
                split = out.split,
                coverage = out.coverage,
                shared_mic,
                bleed_dropped = report.dropped_bleed,
                single_remote = report.single_remote,
                you_relabelled = report.you_relabelled,
                voice_checked = out.voice.checked,
                voice_moved = out.voice.moved,
                "diarization done"
            );
            (DiarizationStatus::Ready, "done", (1, 1))
        }
        Err((message, missing)) => {
            block.status = if missing {
                DiarizationStatus::Unavailable
            } else {
                DiarizationStatus::Failed
            };
            block.error = Some(if missing {
                model_missing(&model_id).message
            } else {
                message
            });
            tracing::warn!(job = %job_id, error = %block.error.as_deref().unwrap_or(""), "diarization did not complete");
            (block.status, "failed", (0, 0))
        }
    };
    block.ran_at = Some(dettivo_storage::time::now_iso());
    row.diarization = Some(block);
    // The result is written only by the pass that still owns the
    // meeting: a delete that invalidated it drops the speakers and the
    // block instead of recreating what it cleared.
    // The voiceprint takes this meeting's voice under the same ownership
    // check, so a meeting deleted or replaced while the pass ran never
    // reaches the user's print.
    let stored = service.commit(&row.id, &job_id, || {
        if let Err(e) = history.with_store(|s| s.update_meeting(&row)) {
            tracing::warn!(error = %e, "history: diarization result not stored");
        }
        if let Some(enrolment) = &enrolment {
            enrolment.store(&data_dir, &row.id);
        }
    });
    if stored.is_none() {
        tracing::info!(job = %job_id, "diarization invalidated while it ran; result dropped");
        publish_progress(&bus, &job_id, (0, 0), "cancelled");
        return;
    }
    publish_progress(&bus, &job_id, chunks, stage);
    publish_state(&bus, &row, status);
}
