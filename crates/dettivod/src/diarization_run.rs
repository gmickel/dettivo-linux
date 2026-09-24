//! The speaker pass thread (ADR 0035): reads the diarized track, sends it
//! through the engine, labels the segments under the coverage and share
//! rule and commits the result through the meeting's job entry. The
//! service that starts and tracks passes is `diarization.rs`.

use dettivo_meeting::diarize;
use dettivo_proto::methods::speakers::DiarizationStatus;
use dettivo_speech::EngineError;
use dettivo_speech::diarize::{DiarizeRequest, timeout_for};
use dettivo_speech::engines::DIARIZE_BINARY;

use crate::diarization::{
    Diarization, Pass, STAGE, model_missing, publish_progress, publish_state,
};

/// The engine's last redacted stderr line, for the row.
fn last_line(tail: &str) -> String {
    tail.lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("engine crashed")
        .to_string()
}

/// The pass thread: the track, the engine, the assignment, the row.
pub(crate) fn run(service: &Diarization, pass: Pass) {
    let Pass {
        job_id,
        mut row,
        audio_dir,
        engine,
        model_id,
        rule,
        speakers,
        clustering_threshold,
        cancel,
        history,
        bus,
    } = pass;
    let mut block = row.diarization.clone().unwrap_or_default();
    block.status = DiarizationStatus::Running;
    block.error = None;
    block.engine = Some(DIARIZE_BINARY.into());
    block.model = Some(format!("diarize/{model_id}"));
    row.diarization = Some(block.clone());
    if let Err(e) = history.with_store(|s| s.update_meeting(&row)) {
        tracing::warn!(error = %e, "history: diarization block not stored");
    }
    publish_state(&bus, &row, DiarizationStatus::Running);
    publish_progress(&bus, &job_id, (0, 0), STAGE);
    let track = diarize::track_for(row.system_audio);
    let outcome = match audio_dir.as_deref() {
        None => Err("the meeting keeps no audio".to_string()),
        Some(dir) => diarize::read_track(std::path::Path::new(dir), track),
    }
    .map_err(|e| (format!("audio: {e}"), false))
    .and_then(|pcm| {
        let audio_ms = pcm.len() as u64 * 1000 / 16_000;
        let request = DiarizeRequest {
            pcm,
            speakers,
            clustering_threshold: Some(clustering_threshold),
        };
        let mut progress = |done: u32, total: u32| {
            service.note_progress(&row.id, (done, total));
            publish_progress(&bus, &job_id, (done, total), STAGE);
        };
        engine
            .diarize(&request, timeout_for(audio_ms), &cancel, &mut progress)
            .map(|r| (r, audio_ms))
            .map_err(|e| match e {
                EngineError::ModelMissing(m) => (m, true),
                EngineError::Crashed(tail) => (last_line(&tail), false),
                EngineError::Cancelled => ("cancelled".into(), false),
                other => (other.to_string(), false),
            })
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
            let room_audio = !row.system_audio;
            let out = diarize::assign(
                &mut row.segments,
                &result.turns,
                room_audio,
                audio_ms,
                &rule,
            );
            let mut speakers = out.speakers;
            diarize::carry_names(&mut speakers, &mut row.segments, &previous);
            row.speakers = speakers;
            block.status = DiarizationStatus::Ready;
            block.coverage = Some(out.coverage);
            block.error = None;
            tracing::info!(
                job = %job_id,
                speakers = row.speakers.len(),
                turns = result.turns.len(),
                coverage = out.coverage,
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
    let stored = service.commit(&row.id, &job_id, || {
        if let Err(e) = history.with_store(|s| s.update_meeting(&row)) {
            tracing::warn!(error = %e, "history: diarization result not stored");
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
