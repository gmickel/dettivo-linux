//! The job thread: decode the upload when there is one (stage
//! `decoding`), run the chunked pipeline over the WAV (`transcribing`,
//! one `job.progress` per chunk, then `merging`), apply the raw pipeline,
//! and finish the item with its text, segments and words, or with the
//! reason it failed or was cancelled. Earlier chunks are kept either way.

use std::sync::Arc;

use dettivo_storage::item::{DictationItem, ItemStatus};
use dettivo_transcribe::{JobError, Stage, Transcript, WavSource};

use crate::events::EventBus;
use crate::history::History;
use crate::jobs::{Jobs, Work};

/// The error code an item carries after a cancel.
pub const CANCELLED: &str = "cancelled";

#[cfg(test)]
#[path = "jobs_run_tests.rs"]
mod tests;

/// The raw layer's switches a job carries.
#[derive(Clone, Copy)]
struct RawFlags {
    spoken: bool,
    protect_tokens: bool,
}

/// Fills an item from a transcript (final on success, partial otherwise).
fn fill(
    item: &mut DictationItem,
    t: &Transcript,
    replacements: &[(String, String)],
    raw: RawFlags,
) {
    item.raw_text = t.text.clone();
    item.final_text =
        dettivo_language::raw::apply(&t.text, replacements, raw.spoken, raw.protect_tokens);
    item.segments = t.contract_segments();
    item.notice = t.notice.clone().map(serde_json::Value::String);
    if !t.language.is_empty() {
        item.language = t.language.clone();
    }
    if t.duration_ms > 0 {
        item.duration_ms = t.duration_ms;
    }
    if item.title.is_empty() {
        item.title = dettivo_storage::item::title_for(&item.final_text);
    }
}

/// Decodes the upload into `work.audio`; the item's duration and audio
/// path follow the decoded file.
fn decode(jobs: &Jobs, history: &History, bus: &EventBus, work: &mut Work) -> Result<(), String> {
    let Some(d) = work.decode.as_ref() else {
        return Ok(());
    };
    jobs.publish(bus, &work.job_id, 0.0, None, Stage::Decoding);
    if let Some(parent) = work.audio.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let outcome = dettivo_audio::decode::decode_to_wav(
        &d.source,
        &d.content_type,
        &work.audio,
        Some(d.max_seconds),
    );
    let (info, samples) = outcome.map_err(|e| e.to_string())?;
    work.item.duration_ms = samples * 1000 / 16_000;
    if !work.discard_audio {
        work.item.audio_path = Some(work.audio.to_string_lossy().into_owned());
    }
    if work.meeting.is_none() {
        if let Err(e) = history.with_store(|store| store.update(&work.item)) {
            tracing::warn!(error = %e, "history: decoded duration not stored");
        }
    }
    tracing::info!(
        job = %work.job_id,
        container = %info.container,
        codec = %info.codec,
        sample_rate = info.sample_rate,
        channels = info.channels,
        seconds = work.item.duration_ms / 1000,
        "import decoded"
    );
    Ok(())
}

/// The thread body.
pub(crate) fn run(jobs: &Arc<Jobs>, history: &History, bus: &EventBus, mut work: Work) {
    let job_id = work.job_id.clone();
    let replacements = work.replacements.clone();
    let raw = RawFlags {
        spoken: work.spoken_punctuation,
        protect_tokens: work.protect_tokens,
    };
    let mut last = (0.0f64, None::<(u32, u32)>);
    let outcome: Result<Transcript, (String, String, Option<Transcript>)> =
        match decode(jobs, history, bus, &mut work) {
            Err(e) => Err(("decode".to_string(), e, None)),
            Ok(()) => match WavSource::open(&work.audio) {
                Err(e) => Err(("audio".to_string(), e, None)),
                Ok(mut source) => {
                    let request = dettivo_transcribe::Request {
                        language: work.language.clone(),
                        prompt: work.prompt.clone(),
                        from_system: false,
                    };
                    let mut on_progress = |p: dettivo_transcribe::Progress| {
                        last = (p.fraction(), Some((p.chunks_done, p.chunks_total)));
                        jobs.publish(bus, &job_id, p.fraction(), last.1, p.stage);
                    };
                    dettivo_transcribe::run(
                        &mut source,
                        work.engine.as_ref(),
                        &request,
                        &work.settings,
                        &work.cancel,
                        &mut on_progress,
                    )
                    .map_err(|e| {
                        let message = e.to_string();
                        match e {
                            JobError::Cancelled { partial } => (
                                CANCELLED.to_string(),
                                "cancelled by the client".to_string(),
                                Some(*partial),
                            ),
                            JobError::Chunk { partial, .. } => {
                                ("engine".to_string(), message, Some(*partial))
                            }
                            JobError::NoTimestamps { .. } => ("engine".to_string(), message, None),
                            JobError::Audio(m) => ("audio".to_string(), m, None),
                        }
                    })
                }
            },
        };
    let cancel_requested = jobs
        .begin_settlement(&job_id)
        .unwrap_or_else(|| work.cancel.load(std::sync::atomic::Ordering::SeqCst));
    let outcome = if cancel_requested {
        let partial = match outcome {
            Ok(transcript) => Some(transcript),
            Err((_, _, partial)) => partial,
        };
        Err((
            CANCELLED.to_string(),
            "cancelled by the client".to_string(),
            partial,
        ))
    } else {
        outcome
    };
    let mut item = work.item.clone();
    let mut stage = match &outcome {
        Ok(t) => {
            fill(&mut item, t, &replacements, raw);
            Stage::Done
        }
        Err((code, message, partial)) => {
            if let Some(t) = partial {
                fill(&mut item, t, &replacements, raw);
            }
            tracing::warn!(job = %job_id, code, message, "job did not complete");
            if code == CANCELLED {
                Stage::Cancelled
            } else {
                Stage::Failed
            }
        }
    };
    let result = match outcome {
        Ok(_) => Ok(()),
        Err((code, message, _)) => Err((code, message)),
    };
    let persisted = if let Some(row) = work.meeting.as_mut() {
        let persisted = history.finish_meeting(row, &item, result, work.archive.as_deref());
        if persisted.is_ok()
            && row.status == dettivo_storage::meetings::MeetingStatus::Completed
            && let (Some(archive), Some((keep_audio, artifacts))) =
                (&work.archive, work.meeting_retention)
        {
            archive.settled(row, keep_audio, artifacts);
        }
        persisted
    } else if item.status == ItemStatus::Transcribing {
        history.finish_item(&mut item, result)
    } else {
        Ok(())
    };
    if let Err(e) = &persisted {
        stage = Stage::Failed;
        retain_after_commit_failure(history, &mut work, &mut item, e.to_string());
    }
    if work.discard_audio && persisted.is_ok() {
        let _ = std::fs::remove_file(&work.audio);
    }
    if persisted.is_ok()
        && let Some(decode) = &work.decode
    {
        let _ = std::fs::remove_file(&decode.source);
    }
    history.clear_busy(&item.id);
    if let Some(original) = &item.rerun_of_item_id {
        history.clear_busy(original);
    }
    let progress = if stage == Stage::Done { 1.0 } else { last.0 };
    jobs.publish(bus, &job_id, progress, last.1, stage);
    jobs.finished(&job_id);
    tracing::info!(job = %job_id, stage = stage.as_str(), "job finished");
}

fn retain_after_commit_failure(
    history: &History,
    work: &mut Work,
    item: &mut DictationItem,
    message: String,
) {
    tracing::warn!(job = %work.job_id, error = %message, "job result not stored; retaining input");
    let dir = if work.meeting.is_some() {
        history.meeting_artifacts().dir(&item.id)
    } else {
        history.artifacts().dir(&item.id)
    };
    let target = dir.join(dettivo_storage::retention::AUDIO_FILE);
    let retained = if work.audio == target {
        work.audio.is_file()
    } else {
        std::fs::create_dir_all(&dir).and_then(|_| {
            let mut source = std::fs::File::open(&work.audio)?;
            let mut output = std::fs::OpenOptions::new().write(true).create_new(true).open(&target)?;
            std::io::copy(&mut source, &mut output)?;
            output.sync_all()
        }).map_err(|e| tracing::warn!(error = %e, audio = %work.audio.display(), "recovery copy failed; original input retained")).is_ok()
    };
    let failed = Err(("storage".to_string(), message));
    let persisted = if let Some(row) = work.meeting.as_mut() {
        if retained {
            row.audio_dir = Some(dir.to_string_lossy().into_owned());
        }
        history.finish_meeting(row, item, failed, None)
    } else {
        if retained {
            item.audio_path = Some(target.to_string_lossy().into_owned());
        }
        history.finish_item(item, failed)
    };
    if let Err(e) = persisted {
        tracing::warn!(error = %e, "failed job status not stored; restart will reconcile the row");
    }
}
