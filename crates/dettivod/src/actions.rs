//! The dictation verbs as one path for every caller: the `dictation.*`
//! handlers, a compositor binding through the CLI, the portal and the
//! evdev backend. Every start captures the focused window before the
//! microphone opens (FR-H2), and a hotkey press while a session runs is
//! ignored with a log line, never a second session.

use std::sync::Arc;

use dettivo_hotkeys::{Action, HotkeyEvent};
use dettivo_proto::error::{AppCode, ErrorDetails, JsonRpcError};
use dettivo_proto::runtime::InsertionResult;
use dettivo_session::machine::Snapshot;
use dettivo_session::{SessionError, SessionTarget, Transcript};

use crate::daemon::{Daemon, blocking};

/// Maps a session refusal to the wire error.
pub fn session_error(e: SessionError) -> JsonRpcError {
    let message = e.to_string();
    match e {
        SessionError::Active => JsonRpcError::new(
            AppCode::Conflict,
            message,
            ErrorDetails::conflict_kind("sessionActive"),
        ),
        SessionError::NoSession | SessionError::NoTranscript => {
            JsonRpcError::new(AppCode::NotFound, message, ErrorDetails::empty())
        }
        SessionError::ModelMissing { .. } => {
            JsonRpcError::new(AppCode::NotFound, message, ErrorDetails::empty())
        }
        SessionError::ModeUnknown(_) => {
            JsonRpcError::new(AppCode::InvalidParams, message, ErrorDetails::empty())
        }
        SessionError::Cancelled => JsonRpcError::new(
            AppCode::Conflict,
            message,
            ErrorDetails::conflict_kind("sessionCancelled"),
        ),
        SessionError::Audio(_) | SessionError::Failed(_) => {
            JsonRpcError::new(AppCode::InternalError, message, ErrorDetails::empty())
        }
    }
}

/// The focused window right now, through the session's probe: the
/// session's origin, or an unverified one when the probe saw nothing.
pub fn capture_target(daemon: &Daemon) -> SessionTarget {
    let settings = daemon.insert_settings();
    let probed = blocking(|| {
        let session = daemon.insert.session();
        daemon.insert.probe(&session, &settings)
    });
    session_target(probed)
}

/// The session target a probe's answer becomes. A probe that failed or
/// saw no focused window is an unverified origin, never an unguarded
/// one: the take goes to the clipboard rather than to whatever window
/// has the focus once the take is transcribed.
pub fn session_target(probed: dettivo_insert::service::Probed) -> SessionTarget {
    match probed.target {
        Some(target) => {
            tracing::debug!(probe = probed.probe, "target captured at start");
            SessionTarget {
                app_id: Some(target.app_id),
                pid: target.pid,
                window: probed.window,
                unverified: false,
            }
        }
        None => {
            tracing::warn!(
                probe = probed.probe,
                "no focused window at start: the take will go to the clipboard"
            );
            SessionTarget::unverified()
        }
    }
}

/// Starts a session. `expected` is a target the caller captured itself;
/// without one the daemon probes the focused window now.
pub fn start(
    daemon: &Daemon,
    language: Option<&str>,
    mode: Option<&str>,
    expected: Option<SessionTarget>,
) -> Result<Snapshot, JsonRpcError> {
    let _reservation = daemon.capture_start_lock();
    start_reserved(daemon, language, mode, expected)
}

fn start_reserved(
    daemon: &Daemon,
    language: Option<&str>,
    mode: Option<&str>,
    expected: Option<SessionTarget>,
) -> Result<Snapshot, JsonRpcError> {
    let loaded = daemon.config();
    let policy = crate::dictation::Dictation::policy(
        &loaded,
        &daemon.paths,
        daemon.engines().model_path(),
        language,
        mode,
    );
    if daemon.dictation().snapshot().is_some() {
        return Err(session_error(SessionError::Active));
    }
    if daemon.meetings().snapshot().is_some() {
        return Err(JsonRpcError::new(
            AppCode::Conflict,
            "A meeting session is active",
            ErrorDetails::conflict_kind("sessionActive"),
        ));
    }
    let target = match expected {
        Some(t) if !t.is_empty() => t,
        _ => capture_target(daemon),
    };
    daemon.engines().ensure_selected_verified()?;
    let engine = daemon.engines().engine();
    daemon.engines().preload_session_start();
    daemon
        .dictation()
        .start(
            policy,
            engine,
            daemon.audio().cloned(),
            &loaded.config.audio.input_device,
            Some(target),
        )
        .map_err(session_error)
}

/// Resolves a toggle under the capture reservation, then settles that take.
/// Toggles during settlement join the same job rather than start another recording.
pub fn toggle(daemon: &Daemon) -> Result<Toggle, JsonRpcError> {
    match toggle_decision(daemon)? {
        ToggleDecision::Started(snapshot) => Ok(Toggle::Started(snapshot)),
        ToggleDecision::Stop(id) => daemon
            .dictation()
            .stop_expected(&id)
            .map(|t| Toggle::Stopped(Box::new(t)))
            .map_err(session_error),
    }
}

fn toggle_decision(daemon: &Daemon) -> Result<ToggleDecision, JsonRpcError> {
    let _reservation = daemon.capture_start_lock();
    match daemon.dictation().snapshot() {
        Some(snapshot) => Ok(ToggleDecision::Stop(snapshot.job_id)),
        None => {
            start_reserved(daemon, None, None, None).map(|s| ToggleDecision::Started(Box::new(s)))
        }
    }
}

enum ToggleDecision {
    Started(Box<Snapshot>),
    Stop(String),
}

/// The result of the daemon-owned toggle decision.
pub enum Toggle {
    /// A newly recording take.
    Started(Box<Snapshot>),
    /// A settled take.
    Stopped(Box<Transcript>),
}

/// Stops and finishes the session.
pub fn stop(daemon: &Daemon) -> Result<Transcript, JsonRpcError> {
    daemon.dictation().stop().map_err(session_error)
}

/// Cancels the session.
pub fn cancel(daemon: &Daemon) -> Result<Snapshot, JsonRpcError> {
    daemon.dictation().cancel().map_err(session_error)
}

/// Inserts the last transcript again: the session's last transcript, or
/// after a restart the newest item in the store. The item id and the
/// insertion result come back.
pub fn reinsert_last(daemon: &Daemon) -> Result<(String, InsertionResult), JsonRpcError> {
    match daemon.dictation().reinsert_last() {
        Ok(t) => {
            let insertion = t
                .insertion
                .unwrap_or_else(|| daemon.dictation().inserter().insert(&t.text, None));
            Ok((t.id, insertion))
        }
        Err(SessionError::NoTranscript) => {
            let item = daemon
                .history()
                .with_store(|store| store.latest())
                .map_err(crate::history::store_error)?
                .ok_or_else(|| session_error(SessionError::NoTranscript))?;
            let insertion = daemon.dictation().inserter().insert(&item.final_text, None);
            Ok((item.id, insertion))
        }
        Err(e) => Err(session_error(e)),
    }
}

/// What a hotkey does. Runs on the one action worker, in order; a stop
/// waits for the transcription and the insertion on a thread of its own,
/// so a cancel pressed meanwhile is acted on at once instead of queueing
/// behind the text it was meant to stop.
pub fn on_hotkey(daemon: &Arc<Daemon>, event: HotkeyEvent) {
    daemon.hotkeys().note_press();
    let active = daemon.dictation().snapshot().is_some();
    let action = match event {
        HotkeyEvent::Press(a) | HotkeyEvent::Release(a) => a.id(),
    };
    let outcome: Result<&str, JsonRpcError> = match event {
        HotkeyEvent::Press(Action::PushToTalk) if active => {
            tracing::info!(
                action = "push_to_talk",
                "hotkey press ignored: a session is active"
            );
            return;
        }
        HotkeyEvent::Press(Action::PushToTalk) => {
            start(daemon, None, None, None).map(|_| "started")
        }
        HotkeyEvent::Release(Action::PushToTalk) => return stop_detached(daemon, action),
        HotkeyEvent::Press(Action::Toggle) => return toggle_detached(daemon, action),
        HotkeyEvent::Press(Action::Cancel) => cancel(daemon).map(|_| "cancelled"),
        HotkeyEvent::Press(Action::ReinsertLast) => reinsert_last(daemon).map(|_| "reinserted"),
        HotkeyEvent::Release(_) => return,
    };
    report(action, outcome);
}

fn toggle_detached(daemon: &Arc<Daemon>, action: &'static str) {
    match toggle_decision(daemon) {
        Ok(ToggleDecision::Started(_)) => report(action, Ok("started")),
        Ok(ToggleDecision::Stop(id)) => stop_job_detached(daemon, action, id),
        Err(error) => report(action, Err(error)),
    }
}

/// Captures the take identity before handing settlement to a background thread.
fn stop_detached(daemon: &Arc<Daemon>, action: &'static str) {
    match daemon.dictation().snapshot() {
        Some(snapshot) => stop_job_detached(daemon, action, snapshot.job_id),
        None => report(action, Err(session_error(SessionError::NoSession))),
    }
}

fn stop_job_detached(daemon: &Arc<Daemon>, action: &'static str, job_id: String) {
    let owned = daemon.clone();
    let job = job_id.clone();
    let spawned = std::thread::Builder::new()
        .name("hotkey-stop".into())
        .spawn(move || {
            report(
                action,
                owned
                    .dictation()
                    .stop_expected(&job)
                    .map(|_| "stopped")
                    .map_err(session_error),
            )
        });
    if let Err(e) = spawned {
        tracing::warn!(action, error = %e, "hotkey stop thread not started; stopping inline");
        report(
            action,
            daemon
                .dictation()
                .stop_expected(&job_id)
                .map(|_| "stopped")
                .map_err(session_error),
        );
    }
}

/// Logs what a hotkey did, or why it was refused.
fn report(action: &'static str, outcome: Result<&str, JsonRpcError>) {
    match outcome {
        Ok(what) => tracing::info!(action, what, "hotkey"),
        Err(e) if e.app_code() == AppCode::NotFound => {
            tracing::debug!(
                action,
                code = e.app_code().as_str(),
                "hotkey without a session"
            )
        }
        Err(e) => tracing::warn!(
            action,
            code = e.app_code().as_str(),
            "hotkey action refused"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dettivo_insert::service::Probed;
    use dettivo_proto::methods::insert::Target;

    /// A probe that saw the window keeps its identity; one that saw
    /// nothing is an unverified origin, never an unguarded one.
    #[test]
    fn a_failed_probe_at_start_is_an_unverified_origin_not_an_unguarded_one() {
        let seen = session_target(Probed {
            probe: "hyprland",
            target: Some(Target {
                app_id: "foot".into(),
                pid: Some(42),
                title_hash: "h".into(),
                is_dettivo: false,
                xwayland: false,
            }),
            window: Some("0x1".into()),
        });
        assert_eq!(
            seen,
            SessionTarget {
                app_id: Some("foot".into()),
                pid: Some(42),
                window: Some("0x1".into()),
                unverified: false,
            }
        );
        let unseen = session_target(Probed {
            probe: "hyprland",
            target: None,
            window: None,
        });
        assert!(unseen.unverified && unseen.is_empty());
        assert_ne!(unseen, SessionTarget::default(), "distinct from no guard");
    }
}
