//! `meetings.delete` per the contract (the artifact policy, ADR 0036) and
//! the Linux additions `meetings.recover`, `meetings.discard`,
//! `meetings.disclosure.get` and `meetings.disclosure.acknowledge` (ADR
//! 0027): what happens to a meeting after its capture, and the gate every
//! capture passes first.

use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::methods::meetings::{
    ArtifactPolicy, DeleteParams, DeleteResult, DiscardResult, RecoverParams, RecoverResult,
};
use dettivo_proto::runtime::HistoryStatus;
use dettivo_storage::meetings::{MeetingRow, MeetingStatus};
use serde_json::Value;

use super::meetings::{conflict, json, lookup, params, reference};
use crate::daemon::Daemon;
use crate::history::store_error;

/// `meetings.delete` per the contract's `artifact_policy` (ADR 0036):
/// `transcript_only` clears the texts, the segments, the speakers, the
/// analysis and the search entry and keeps the row, the notes and the
/// audio; `transcript_and_audio` also removes every take and the take
/// sidecars and forgets the audio facts, keeping the row's other facts
/// and the notes; `all` removes the row and the directory. A request
/// that names no policy takes `[meetings] delete_artifact_policy`, so
/// no client decides one of its own. The active meeting is `CONFLICT`;
/// a running analysis or speaker pass is invalidated first, so its
/// result never restores what the delete clears.
pub fn delete(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: DeleteParams = params(params_value)?;
    let id = p.meeting_id.as_str();
    let policy = match p.artifact_policy {
        Some(policy) => policy,
        None => configured_policy(daemon)?,
    };
    refuse_while_busy(daemon, id)?;
    lookup(daemon, id)?;
    if daemon.analysis().cancel(id) {
        tracing::info!("meeting analysis cancelled for the delete");
    }
    if daemon.diarization().invalidate(id) {
        tracing::info!("diarization cancelled for the delete");
    }
    let history = daemon.history();
    let dir = history.meeting_artifacts().dir(id);
    match policy {
        ArtifactPolicy::TranscriptOnly => {
            history
                .with_store(|s| s.clear_meeting_transcript(id))
                .map_err(store_error)?;
            clear_transcript_sidecars(&dir).map_err(store_error)?;
        }
        ArtifactPolicy::TranscriptAndAudio => {
            history
                .with_store(|s| s.clear_meeting_transcript(id))
                .map_err(store_error)?;
            clear_transcript_sidecars(&dir).map_err(store_error)?;
            history
                .meeting_artifacts()
                .remove_audio_and_sidecars(id)
                .map_err(store_error)?;
            history
                .with_store(|s| s.clear_meeting_audio(id))
                .map_err(store_error)?;
        }
        ArtifactPolicy::All => {
            history.delete_meeting(id).map_err(store_error)?;
        }
    }
    tracing::info!(?policy, "meeting deleted");
    json(DeleteResult {
        reference: reference(id)?,
        deleted: true,
    })
}

/// `[meetings] delete_artifact_policy` as the contract spells it; a
/// value the contract does not know is an error, never `all`.
fn configured_policy(daemon: &Daemon) -> Result<ArtifactPolicy, JsonRpcError> {
    let configured = daemon
        .config()
        .config
        .meetings
        .delete_artifact_policy
        .clone();
    serde_json::from_value(Value::String(configured.clone())).map_err(|_| {
        JsonRpcError::new(
            AppCode::InternalError,
            format!(
                "[meetings] delete_artifact_policy is {configured:?}; it must be transcript_only, transcript_and_audio or all"
            ),
            ErrorDetails::empty(),
        )
    })
}

fn refuse_while_busy(daemon: &Daemon, id: &str) -> Result<(), JsonRpcError> {
    if daemon
        .meetings()
        .snapshot()
        .is_some_and(|s| s.meeting_id == id)
    {
        return Err(conflict(
            &format!("Meeting {id} is recording; stop or cancel it first"),
            "sessionActive",
        ));
    }
    if daemon.history().busy().contains(id) || daemon.meetings().finalizing(id).is_some() {
        return Err(conflict(
            &format!("meeting {id} is in use by a running job"),
            "jobRunning",
        ));
    }
    Ok(())
}

fn partial(daemon: &Daemon, id: &str) -> Result<MeetingRow, JsonRpcError> {
    let row = lookup(daemon, id)?;
    if row.status != MeetingStatus::Partial {
        return Err(conflict("Meeting is not partial", "meetingNotPartial"));
    }
    Ok(row)
}

pub(super) fn retryable_status(row: &MeetingRow) -> bool {
    matches!(
        row.status,
        MeetingStatus::Partial
            | MeetingStatus::Failed
            | MeetingStatus::Cancelled
            | MeetingStatus::Stopped
    )
}

pub(super) fn recovery_audio(
    daemon: &Daemon,
    row: &MeetingRow,
) -> Result<std::path::PathBuf, String> {
    recovery_audio_for(daemon, &row.id, &row.source_kind, row.system_audio)
}

pub(super) fn recovery_audio_for(
    daemon: &Daemon,
    id: &str,
    source_kind: &str,
    system_audio: bool,
) -> Result<std::path::PathBuf, String> {
    let dir = daemon.history().meeting_artifacts().dir(id);
    if source_kind == "audioImport" {
        use dettivo_transcribe::AudioSource;
        let audio = dir.join(dettivo_storage::retention::AUDIO_FILE);
        if !audio
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_file())
        {
            return Err("import audio is missing or is not a regular file".into());
        }
        let mut source = dettivo_transcribe::WavSource::open(&audio)?;
        let end = source.len();
        if end > 0 && source.read(end - 1, end)?.len() != 1 {
            return Err("import audio is truncated".into());
        }
        Ok(audio)
    } else {
        if system_audio && !dir.join("system-takes.json").is_file() {
            return Err("system audio manifest is missing".into());
        }
        dettivo_meeting::finalize::takes_in(&dir).map_err(|e| e.to_string())?;
        Ok(dir)
    }
}

/// `meetings.recover` (Linux addition): the partial meeting keeps its
/// audio and finalises from its takes (ADR 0030); the answer carries
/// `transcribing` and the duration preserved, and `meetings.status`
/// follows the job to `completed`.
pub fn recover(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: RecoverParams = params(params_value)?;
    let _reservation = daemon.capture_start_lock();
    let id = p.meeting_id.as_str();
    refuse_while_busy(daemon, id)?;
    let mut row = lookup(daemon, id)?;
    if !retryable_status(&row) {
        return Err(conflict(
            "Meeting is not partial or retryable",
            "meetingNotPartial",
        ));
    }
    if row.source_kind == "capture" && recovery_audio(daemon, &row).is_err() {
        let dir = daemon.history().meeting_artifacts().dir(id);
        let expected_system = row.system_audio;
        dettivo_meeting::recovery::promote(&dir, &mut row);
        row.system_audio |= expected_system;
    }
    let audio = recovery_audio(daemon, &row).map_err(|message| {
        conflict(
            &format!("Meeting audio cannot be recovered: {message}"),
            "audioNotRetained",
        )
    })?;
    row.audio_dir = Some(
        daemon
            .history()
            .meeting_artifacts()
            .dir(id)
            .to_string_lossy()
            .into_owned(),
    );
    let loaded = daemon.config();
    let (engine, _) =
        daemon
            .engines()
            .engine_for(&daemon.paths, &loaded, &row.stt_provider, &row.stt_model)?;
    let duration_ms = row.duration_ms;
    let id = row.id.clone();
    if row.source_kind == "audioImport" {
        daemon.jobs().retry_meeting(daemon, row, audio, engine)?;
    } else {
        daemon
            .meetings()
            .finalize_recovered(&loaded, engine, row)
            .map_err(|e| {
                JsonRpcError::new(AppCode::InternalError, e.to_string(), ErrorDetails::empty())
            })?;
    }
    tracing::info!(duration_ms, "partial meeting recovered; finalising");
    json(RecoverResult {
        reference: reference(&id)?,
        status: HistoryStatus::Transcribing,
        duration_ms,
    })
}

/// `meetings.discard` (Linux addition): the partial meeting and its
/// directory are removed.
pub fn discard(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: RecoverParams = params(params_value)?;
    let row = partial(daemon, p.meeting_id.as_str())?;
    daemon
        .history()
        .delete_meeting(&row.id)
        .map_err(store_error)?;
    tracing::info!("partial meeting discarded");
    json(DiscardResult {
        reference: reference(&row.id)?,
        discarded: true,
    })
}

/// `meetings.disclosure.get`.
pub fn disclosure_get(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    json(daemon.meetings().disclosure())
}

/// `meetings.disclosure.acknowledge`.
pub fn disclosure_acknowledge(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    json(
        daemon
            .meetings()
            .acknowledge()
            .map_err(|e| JsonRpcError::new(AppCode::InternalError, e, ErrorDetails::empty()))?,
    )
}

fn clear_transcript_sidecars(dir: &std::path::Path) -> Result<(), dettivo_storage::StoreError> {
    dettivo_storage::meeting_notes::remove_analysis_file(dir)?;
    let checkpoint = dettivo_meeting::checkpoint::Checkpoint::path(dir);
    if checkpoint.exists() {
        let mut value =
            dettivo_meeting::checkpoint::Checkpoint::read(dir).map_err(std::io::Error::other)?;
        value.segments.clear();
        value.next_sequence = 0;
        value.write(dir)?;
    }
    match std::fs::remove_file(dir.join(".live-checkpoint.tmp")) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let metadata = dir.join(dettivo_meeting::METADATA_FILE);
    match std::fs::read(&metadata) {
        Ok(bytes) => {
            let mut value: Value = serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
            let object = value
                .as_object_mut()
                .ok_or_else(|| std::io::Error::other("meeting metadata is not an object"))?;
            object.retain(|key, _| {
                matches!(
                    key.as_str(),
                    "meeting_id" | "takes" | "journal" | "checkpoint_schema"
                )
            });
            std::fs::write(
                &metadata,
                serde_json::to_vec_pretty(&value).map_err(std::io::Error::other)?,
            )?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(())
}
