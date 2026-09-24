//! `dictation.start`, `stop`, `cancel`, `status` and the Linux addition
//! `reinsert_last`.

use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::id::Id;
use dettivo_proto::methods::dictation::{
    CancelParams, CancelResult, DictationMode, ReinsertLastResult, SessionTarget as WireTarget,
    StartParams, StartResult, StatusResult, StopResult,
};
use dettivo_proto::runtime::{JobState, JobStatus, RefKind, TranscriptRef};
use dettivo_session::machine::Snapshot;
use dettivo_session::{SessionTarget, State};
use serde_json::Value;

use super::{json, params};
use crate::actions;

use crate::daemon::Daemon;

/// The guards a caller captured itself, as a session target. A pid that
/// is not a number is `INVALID_PARAMS` naming the field.
fn expected_target(p: &StartParams) -> Result<Option<SessionTarget>, JsonRpcError> {
    let pid = match p.expected_target_pid.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(text) => Some(text.parse::<u32>().map_err(|_| {
            JsonRpcError::new(
                AppCode::InvalidParams,
                "expected_target_pid must be a process id",
                ErrorDetails::empty(),
            )
        })?),
    };
    let app_id = p
        .expected_target_bundle_id
        .clone()
        .filter(|s| !s.trim().is_empty());
    let target = SessionTarget {
        app_id,
        pid,
        window: None,
        unverified: false,
    };
    Ok((!target.is_empty()).then_some(target))
}

fn wire_target(t: &SessionTarget) -> WireTarget {
    WireTarget {
        app_id: t.app_id.clone(),
        pid: t.pid,
    }
}

/// The contract job for a snapshot.
pub fn job(snapshot: &Snapshot) -> JobStatus {
    let (state, message) = match snapshot.state {
        State::Recording => (JobState::Running, Some("listening".to_string())),
        State::Transcribing => (JobState::Running, Some("transcribing".to_string())),
        State::Inserting => (JobState::Running, Some("inserting".to_string())),
        State::Idle => (JobState::Succeeded, None),
        State::Cancelled => (JobState::Cancelled, None),
        State::Failed => (JobState::Failed, None),
    };
    JobStatus {
        job_id: snapshot.job_id.clone(),
        state,
        progress: snapshot.progress,
        message: message.map(|m| {
            format!(
                "{m} with {}/{}",
                snapshot.policy.provider, snapshot.policy.model
            )
        }),
        error: snapshot.error.clone(),
    }
}

/// `dictation.start`.
pub fn start(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: StartParams = params(params_value)?;
    // A field left out means the configured value, as the hotkey path
    // has it, so a client never has to guess what the file says.
    let mode = p.mode.map(DictationMode::as_str);
    let language = p.language.trim();
    let language = (!language.is_empty()).then_some(language);
    let expected = expected_target(&p)?;
    let snapshot = actions::start(daemon, language, mode, expected)?;
    json(StartResult {
        job: job(&snapshot),
    })
}

/// `dictation.toggle`: the daemon serializes the decision with capture starts.
pub fn toggle(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    match actions::toggle(daemon)? {
        actions::Toggle::Started(snapshot) => json(StartResult {
            job: job(&snapshot),
        }),
        actions::Toggle::Stopped(transcript) => stopped(*transcript),
    }
}

/// `dictation.stop`.
pub fn stop(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    stopped(actions::stop(daemon)?)
}

fn stopped(transcript: dettivo_session::Transcript) -> Result<Value, JsonRpcError> {
    json(StopResult {
        reference: TranscriptRef {
            kind: RefKind::Dictation,
            id: Id::new(transcript.id.clone()).map_err(|e| {
                JsonRpcError::new(AppCode::InternalError, e.to_string(), ErrorDetails::empty())
            })?,
        },
        job: JobStatus {
            job_id: transcript.job_id.clone(),
            state: JobState::Succeeded,
            progress: 1.0,
            message: None,
            error: None,
        },
        insertion: transcript.insertion,
    })
}

/// `dictation.cancel`.
pub fn cancel(daemon: &Daemon, params_value: Value) -> Result<Value, JsonRpcError> {
    let p: CancelParams = params(params_value)?;
    let snapshot = daemon
        .dictation()
        .cancel_expected(p.expected_job_id.as_deref())
        .map_err(actions::session_error)?;
    json(CancelResult {
        job: job(&snapshot),
    })
}

/// `dictation.status`.
pub fn status(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let snapshot = daemon.dictation().snapshot();
    json(StatusResult {
        is_active: snapshot.is_some(),
        job: snapshot.as_ref().map(job),
        context_pack: None,
        // An unverified origin names no window: the field stays absent.
        target: snapshot
            .as_ref()
            .and_then(|s| s.target.as_ref())
            .filter(|t| !t.unverified)
            .map(wire_target),
    })
}

/// `dictation.reinsert_last` (Linux addition): the session's last
/// transcript, or after a restart the newest item in the store.
pub fn reinsert_last(daemon: &Daemon) -> Result<Value, JsonRpcError> {
    let (id, insertion) = actions::reinsert_last(daemon)?;
    json(ReinsertLastResult {
        reference: TranscriptRef {
            kind: RefKind::Dictation,
            id: Id::new(id).map_err(|e| {
                JsonRpcError::new(AppCode::InternalError, e.to_string(), ErrorDetails::empty())
            })?,
        },
        insertion,
    })
}
