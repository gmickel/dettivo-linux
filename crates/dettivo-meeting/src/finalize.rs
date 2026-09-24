//! Finalisation (FR-A2, ADR 0030): every preserved take of both tracks
//! goes through the offline chunked pipeline (`dettivo_transcribe::run`),
//! each take's segments are shifted onto the meeting clock by the take's
//! offset, a take that began after a gap carries the gap on its first
//! segment, the two sources interleave with the cross-source rule, and
//! the result is the transcript the row keeps. Progress counts chunks
//! over every take, so `job.progress` climbs once per chunk.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use dettivo_audio::takes::{Takes, read_sidecar};
use dettivo_speech::SttEngine;
use dettivo_transcribe::live::{LiveSegment, LiveSettings, Source};
use dettivo_transcribe::{
    JobError, Progress, Request, Settings, Stage, WavSource, chunker, interleave,
};

use crate::Track;
use crate::journal;

/// One take as finalisation reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakeInput {
    /// Which side.
    pub source: Source,
    /// The 16 kHz mono WAV.
    pub path: PathBuf,
    /// Milliseconds into the meeting the take began.
    pub start_offset_ms: u64,
    /// True when capture was missing before the take.
    pub gap_before: bool,
}

/// What finalisation produced.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Outcome {
    /// Both sources on the meeting clock.
    pub segments: Vec<LiveSegment>,
    /// The transcript text.
    pub text: String,
    /// The language the engine reported.
    pub language: String,
    /// The meeting's length: the latest take end.
    pub duration_ms: u64,
    /// `silent` when nothing was said.
    pub notice: Option<String>,
    /// Chunks transcribed over every take.
    pub chunks: u32,
}

/// The takes under `dir` from the sidecars both tracks wrote (the
/// microphone's `takes.json`, the system's `system-takes.json`).
/// Damaged manifests, missing inputs and unlisted audio return an error;
/// a system track that was never captured may be absent.
pub fn takes_in(dir: &Path) -> Result<Vec<TakeInput>, JobError> {
    let mut out = Vec::new();
    for (track, source) in [
        (Track::Microphone, Source::You),
        (Track::System, Source::Remote),
    ] {
        let sidecar = if track == Track::Microphone {
            dir.join(dettivo_audio::takes::SIDECAR)
        } else {
            dir.join(format!("{}-takes.json", track.prefix()))
        };
        if track == Track::System && !sidecar.exists() {
            let has_audio = std::fs::read_dir(dir)
                .map_err(|e| JobError::Audio(e.to_string()))?
                .filter_map(Result::ok)
                .any(|e| {
                    e.file_name()
                        .to_str()
                        .is_some_and(|n| n.starts_with("system") && n.ends_with(".wav"))
                });
            if !has_audio {
                continue;
            }
        }
        let takes: Takes = read_sidecar(&sidecar)
            .map_err(|e| JobError::Audio(format!("{}: {e}", sidecar.display())))?;
        for entry in std::fs::read_dir(dir).map_err(|e| JobError::Audio(e.to_string()))? {
            let entry = entry.map_err(|e| JobError::Audio(e.to_string()))?;
            let file = entry.file_name();
            if let Some(file) = file.to_str()
                && file.starts_with(track.prefix())
                && file.ends_with(".wav")
                && !takes.takes.iter().any(|take| take.file == file)
            {
                return Err(JobError::Audio(format!(
                    "unlisted take with unknown timing: {file}"
                )));
            }
        }
        for take in takes.takes {
            let path = dir.join(&take.file);
            if Path::new(&take.file).components().count() != 1
                || !path
                    .symlink_metadata()
                    .is_ok_and(|m| m.file_type().is_file())
            {
                return Err(JobError::Audio(format!(
                    "missing or invalid take: {}",
                    take.file
                )));
            }
            let reader = hound::WavReader::open(&path)
                .map_err(|e| JobError::Audio(format!("{}: {e}", take.file)))?;
            let spec = reader.spec();
            if spec.channels != 1
                || spec.sample_rate != 16000
                || spec.bits_per_sample != 16
                || spec.sample_format != hound::SampleFormat::Int
                || u64::from(reader.len()) != take.samples
            {
                return Err(JobError::Audio(format!(
                    "invalid or unclosed take: {}",
                    take.file
                )));
            }
            if take.samples > 0 {
                out.push(TakeInput {
                    source,
                    path,
                    start_offset_ms: take.start_offset_ms,
                    gap_before: take.gap_before,
                });
            }
        }
    }
    Ok(out)
}

/// The gaps the journal recorded without a new take (`gap` lines with no
/// take named), as meeting-clock offsets.
pub fn journal_gaps(dir: &Path) -> Vec<u64> {
    journal::read(&dir.join(crate::JOURNAL_FILE))
        .into_iter()
        .filter(|e| e.event == "gap" && e.take.is_none())
        .map(|e| e.offset_ms)
        .collect()
}

/// Shifts a transcript's segments by `offset_ms` onto the meeting clock.
fn shifted(
    transcript: &dettivo_transcribe::Transcript,
    source: Source,
    offset_ms: u64,
) -> Vec<LiveSegment> {
    transcript
        .segments
        .iter()
        .map(|s| LiveSegment {
            source,
            id: String::new(),
            provisional: false,
            start_ms: s.start_ms + offset_ms,
            end_ms: s.end_ms + offset_ms,
            text: s.text.clone(),
            words: s
                .words
                .iter()
                .map(|w| dettivo_engine_proto::Word {
                    start_ms: w.start_ms + offset_ms,
                    end_ms: w.end_ms + offset_ms,
                    text: w.text.clone(),
                    confidence: w.confidence,
                })
                .collect(),
            gap_before_ms: None,
        })
        .collect()
}

/// Numbers the segments per source (`you-0`, `remote-0`, ...).
fn numbered(mut segments: Vec<LiveSegment>) -> Vec<LiveSegment> {
    let mut counts = [0u64; 2];
    for s in &mut segments {
        let slot = usize::from(s.source == Source::Remote);
        s.id = format!("{}-{}", s.source.as_str(), counts[slot]);
        counts[slot] += 1;
    }
    segments
}

/// Attaches the journal's take-less gaps to the first segment after each.
fn mark_journal_gaps(segments: &mut [LiveSegment], gaps: &[u64]) {
    for &at in gaps {
        if let Some(i) = segments.iter().position(|s| s.start_ms >= at)
            && segments[i].gap_before_ms.is_none()
        {
            let previous_end = segments[..i].iter().map(|s| s.end_ms).max().unwrap_or(0);
            let start = segments[i].start_ms;
            let from = at.max(previous_end).min(start);
            segments[i].gap_before_ms = Some((start - from).max(1));
        }
    }
}

/// Transcribes every take and interleaves the sources. A cancel between
/// chunks or a failed take hands back what was finished as a partial
/// outcome inside the error.
#[allow(clippy::too_many_arguments)]
pub fn run(
    takes: &[TakeInput],
    journal_gaps: &[u64],
    engine: &dyn SttEngine,
    request: &Request,
    settings: &Settings,
    live: &LiveSettings,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Outcome, JobError> {
    // Plan every take first so the chunk total is known from the start.
    let mut sources = Vec::with_capacity(takes.len());
    let mut total = 0u32;
    for take in takes {
        let mut source = WavSource::open(&take.path).map_err(JobError::Audio)?;
        let chunks = chunker::plan(&mut source, settings).map_err(JobError::Audio)?;
        total += chunks.len() as u32;
        sources.push(source);
    }
    progress(Progress {
        stage: Stage::Transcribing,
        chunks_done: 0,
        chunks_total: total,
    });
    let mut segments: Vec<LiveSegment> = Vec::new();
    let mut done = 0u32;
    let mut language = String::new();
    let mut duration_ms = 0u64;
    let mut all_silent = true;
    let mut previous_end: [u64; 2] = [0, 0];
    let mut reported = 0u32;
    for (take, source) in takes.iter().zip(sources.iter_mut()) {
        let take_request = Request {
            from_system: take.source == Source::Remote,
            ..request.clone()
        };
        // Every take's job reports from its own zero; only a new count
        // over the whole meeting is passed on.
        let mut relay = |p: Progress| {
            let over_all = done + p.chunks_done;
            if p.stage == Stage::Transcribing && over_all > reported {
                reported = over_all;
                progress(Progress {
                    stage: Stage::Transcribing,
                    chunks_done: over_all,
                    chunks_total: total,
                });
            }
        };
        let planned = chunker::plan(source, settings)
            .map(|c| c.len() as u32)
            .unwrap_or(0);
        let outcome =
            dettivo_transcribe::run(source, engine, &take_request, settings, cancel, &mut relay);
        let transcript = match outcome {
            Ok(t) => t,
            Err(JobError::Cancelled { .. }) => {
                return Err(JobError::Cancelled {
                    partial: Box::new(partial_of(&segments, &language, duration_ms)),
                });
            }
            Err(JobError::Chunk {
                index,
                total: _,
                message,
                ..
            }) => {
                return Err(JobError::Chunk {
                    index: done + index,
                    total,
                    message,
                    partial: Box::new(partial_of(&segments, &language, duration_ms)),
                });
            }
            Err(other) => return Err(other),
        };
        done += planned;
        if transcript.notice.as_deref() != Some("silent") || !transcript.segments.is_empty() {
            all_silent &= transcript.text.is_empty();
        }
        if language.is_empty() && !transcript.language.is_empty() {
            language = transcript.language.clone();
        }
        let take_end = take.start_offset_ms + transcript.duration_ms;
        duration_ms = duration_ms.max(take_end);
        let mut shifted = shifted(&transcript, take.source, take.start_offset_ms);
        let slot = usize::from(take.source == Source::Remote);
        if take.gap_before
            && let Some(first) = shifted.first_mut()
        {
            first.gap_before_ms = Some(
                take.start_offset_ms
                    .saturating_sub(previous_end[slot])
                    .max(1),
            );
        }
        previous_end[slot] = take_end;
        segments.extend(shifted);
    }
    progress(Progress {
        stage: Stage::Merging,
        chunks_done: total,
        chunks_total: total,
    });
    let mut merged = interleave::interleave(segments, live.cross_source_padding_ms);
    mark_journal_gaps(&mut merged, journal_gaps);
    let merged = numbered(merged);
    let text = interleave::text_of(&merged);
    Ok(Outcome {
        notice: (text.is_empty() && all_silent).then(|| "silent".to_string()),
        text,
        segments: merged,
        language,
        duration_ms,
        chunks: total,
    })
}

fn partial_of(
    segments: &[LiveSegment],
    language: &str,
    duration_ms: u64,
) -> dettivo_transcribe::Transcript {
    let contract = dettivo_transcribe::live::contract_segments(segments);
    dettivo_transcribe::Transcript {
        text: interleave::text_of(segments),
        segments: contract
            .iter()
            .map(|s| dettivo_engine_proto::Segment {
                start_ms: s.start_ms,
                end_ms: s.end_ms,
                text: s.text.clone(),
                words: Vec::new(),
            })
            .collect(),
        language: language.to_string(),
        notice: None,
        duration_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(source: Source, start: u64, end: u64, text: &str) -> LiveSegment {
        LiveSegment {
            source,
            id: String::new(),
            provisional: false,
            start_ms: start,
            end_ms: end,
            text: text.into(),
            words: Vec::new(),
            gap_before_ms: None,
        }
    }

    #[test]
    fn journal_gaps_and_numbering_land_on_the_segments() {
        let mut segments = vec![
            seg(Source::You, 0, 1000, "one"),
            seg(Source::Remote, 500, 1400, "two"),
            seg(Source::You, 5000, 6000, "three"),
        ];
        mark_journal_gaps(&mut segments, &[2000]);
        assert_eq!(segments[2].gap_before_ms, Some(3000));
        let numbered = numbered(segments);
        let ids: Vec<&str> = numbered.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["you-0", "remote-0", "you-1"]);
    }

    #[test]
    fn takes_are_read_from_both_sidecars_and_empty_ones_are_left_out() {
        let dir = tempfile::tempdir().unwrap();
        let clock = std::time::Instant::now();
        let mut mic =
            dettivo_audio::takes::TakeWriter::with_prefix(dir.path(), "microphone", clock).unwrap();
        mic.start_take(false).unwrap();
        mic.write(&[3; 1600]).unwrap();
        mic.finish_take().unwrap();
        mic.start_take(true).unwrap();
        mic.finish().unwrap();
        let mut sys =
            dettivo_audio::takes::TakeWriter::with_prefix(dir.path(), "system", clock).unwrap();
        sys.start_take(false).unwrap();
        sys.write(&[4; 800]).unwrap();
        sys.finish().unwrap();
        let takes = takes_in(dir.path()).unwrap();
        assert_eq!(takes.len(), 2, "{takes:?}");
        assert_eq!(takes[0].source, Source::You);
        assert!(takes[0].path.ends_with("microphone.wav"));
        assert_eq!(takes[1].source, Source::Remote);
        assert!(journal_gaps(dir.path()).is_empty());
    }
}
