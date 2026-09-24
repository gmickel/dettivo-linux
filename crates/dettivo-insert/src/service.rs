//! The `Inserter` the daemon calls: probe the target, apply the guards
//! and the self check, run the chain, record the insertion for `undo`,
//! and shape the contract result. Logs carry lengths and backend names,
//! never the text.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use dettivo_proto::capabilities::InsertMode;
use dettivo_proto::methods::insert::{Target, TargetResult, UndoResult};
use dettivo_proto::runtime::{
    ContextPack, ContextPackMetrics, ContextPackSource, ContextPackStatus, InsertionBackend,
    InsertionMethod, InsertionOutcome, InsertionResult, TargetApp,
};

use crate::backend::{self, Backend, Ctx};
use crate::chain::{self, Candidate, Guards, Refusal, TargetFacts};
use crate::delivery::{self, Origin};
use crate::probe;
use crate::session::Session;
use crate::settings::Settings;

/// One insertion request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request<'a> {
    /// The text.
    pub text: &'a str,
    /// The mode.
    pub mode: InsertMode,
    /// The guards.
    pub guards: Guards<'a>,
    /// Let the text land in one of Dettivo's own windows (the first-run
    /// Try it field); the daemon sets it only while its allowance is
    /// armed (ADR 0024). Off, a Dettivo target is `target_is_self`.
    pub allow_self_target: bool,
}

/// What `insert(text, target, mode)` answers with.
pub trait Inserter {
    /// Performs one insertion. `Err(Conflict)` is the wire `CONFLICT`.
    fn insert(&self, settings: &Settings, request: &Request<'_>)
    -> Result<InsertionResult, String>;
}

/// The last insertion, for `insert.undo`.
#[derive(Debug, Clone)]
struct UndoRecord {
    backend: &'static str,
    chars: usize,
    at: Instant,
    target: Option<Target>,
}

/// The daemon's insertion service.
pub struct Service {
    session_builder: Box<dyn Fn() -> Session + Send + Sync>,
    undo: Mutex<Option<UndoRecord>>,
    /// One insertion at a time: a second request waits for the first to
    /// finish delivering, so two takes never interleave their keys.
    insert_lock: Mutex<()>,
    /// A scripted probe for the tests: each call answers the next window.
    #[cfg(test)]
    scripted_probe: Mutex<std::collections::VecDeque<Option<probe::RawWindow>>>,
}

/// The probed target with the probe's name.
pub struct Probed {
    /// The probe that answered, or `none`.
    pub probe: &'static str,
    /// The target, when a probe saw one.
    pub target: Option<Target>,
    /// The window's identity as the probe knows it (never on the wire):
    /// the guard that tells two windows of one process apart.
    pub window: Option<String>,
}

impl Service {
    /// A service whose session is rebuilt per call from `builder`.
    pub fn new(builder: impl Fn() -> Session + Send + Sync + 'static) -> Self {
        Self {
            session_builder: Box::new(builder),
            undo: Mutex::new(None),
            insert_lock: Mutex::new(()),
            #[cfg(test)]
            scripted_probe: Mutex::new(std::collections::VecDeque::new()),
        }
    }

    /// The session right now.
    pub fn session(&self) -> Session {
        (self.session_builder)()
    }

    /// Probes the focused window.
    pub fn probe(&self, session: &Session, settings: &Settings) -> Probed {
        #[cfg(test)]
        if let Some(scripted) = self.scripted_probe.lock().unwrap().pop_front() {
            return Self::probed("scripted", settings, scripted);
        }
        match probe::for_session(session) {
            None => Probed {
                probe: "none",
                target: None,
                window: None,
            },
            Some(p) => match p.focused() {
                Ok(raw) => Self::probed(p.name(), settings, raw),
                Err(e) => {
                    tracing::warn!(probe = p.name(), error = %e, "focus probe failed");
                    Probed {
                        probe: p.name(),
                        target: None,
                        window: None,
                    }
                }
            },
        }
    }

    fn probed(probe: &'static str, settings: &Settings, raw: Option<probe::RawWindow>) -> Probed {
        Probed {
            probe,
            window: raw.as_ref().and_then(|r| r.window.clone()),
            target: raw.map(|r| probe::target_from(&r, settings)),
        }
    }

    fn candidates<'a>(
        &self,
        backends: &'a [Box<dyn Backend>],
        session: &Session,
    ) -> Vec<Candidate<'a>> {
        backends
            .iter()
            .map(|b| Candidate {
                backend: b.as_ref(),
                availability: b.availability(session),
            })
            .collect()
    }

    /// `insert.target`: the focused window and the chain's availability.
    pub fn target(&self, settings: &Settings) -> TargetResult {
        let session = self.session();
        let probed = self.probe(&session, settings);
        let backends = backend::for_session(&session);
        let candidates = self.candidates(&backends, &session);
        let chosen = chain::choose(
            &candidates,
            settings,
            "",
            InsertMode::Raw,
            TargetFacts::of(probed.target.as_ref()),
        )
        .ok()
        .and_then(|c| c.order.first().cloned());
        TargetResult {
            target: probed.target,
            probe: probed.probe.to_string(),
            backends: candidates.iter().map(Candidate::status).collect(),
            chosen,
        }
    }

    /// The backend the chain would use right now, for `system.capabilities`.
    pub fn chosen_backend(&self, settings: &Settings) -> Option<String> {
        self.target(settings).chosen
    }

    /// `insert.undo`.
    pub fn undo(&self, settings: &Settings) -> UndoResult {
        let record = self.undo.lock().unwrap_or_else(|p| p.into_inner()).take();
        let refuse = |reason: &str| UndoResult {
            undone: false,
            reason: Some(reason.to_string()),
        };
        let Some(record) = record else {
            return refuse("nothing_to_undo");
        };
        if record.at.elapsed() > Duration::from_millis(settings.undo_window_ms) {
            return refuse("expired");
        }
        let session = self.session();
        let backends = backend::for_session(&session);
        let Some(backend) = backends.iter().find(|b| b.name() == record.backend) else {
            return refuse("unsupported_backend");
        };
        if !backend.undo_supported() || record.chars == 0 {
            return refuse("unsupported_backend");
        }
        let now = self.probe(&session, settings).target;
        let same = match (&record.target, &now) {
            (Some(then), Some(now)) => then.app_id == now.app_id && then.pid == now.pid,
            (None, None) => true,
            _ => false,
        };
        if !same {
            return refuse("target_changed");
        }
        let app_id = now.as_ref().map(|t| t.app_id.clone()).unwrap_or_default();
        let ctx = Ctx {
            session: &session,
            settings,
            app_id: &app_id,
            keystroke: None,
            recheck: None,
        };
        match backend.undo(record.chars, &ctx) {
            Ok(()) => {
                tracing::info!(
                    backend = record.backend,
                    chars = record.chars,
                    "insertion undone"
                );
                UndoResult {
                    undone: true,
                    reason: None,
                }
            }
            Err(e) => {
                tracing::warn!(backend = record.backend, error = %e, "undo failed");
                refuse("backend_failed")
            }
        }
    }
}

fn result(
    outcome: InsertionOutcome,
    method: InsertionMethod,
    target: Option<&Target>,
    reason: Option<&str>,
    backend: Option<InsertionBackend>,
) -> InsertionResult {
    let app_id = target.map(|t| t.app_id.clone()).unwrap_or_default();
    let name = if app_id == probe::MOCK_APP_ID {
        "Text Editor".to_string()
    } else {
        app_id.clone()
    };
    InsertionResult {
        outcome,
        method,
        target_app: TargetApp {
            bundle_id: app_id.clone(),
            name,
        },
        context_pack: ContextPack {
            status: ContextPackStatus::Off,
            reason: Some("context_capture_off".to_string()),
            source: ContextPackSource {
                adapter_id: "none".to_string(),
                app_class: "generic".to_string(),
                bundle_id: app_id,
            },
            metrics: ContextPackMetrics {
                capture_duration_ms: 0,
                payload_bytes: 0,
                character_count: 0,
                token_budget: 0,
                token_estimate: 0,
            },
        },
        reason: reason.map(str::to_string),
        backend,
    }
}

impl Inserter for Service {
    fn insert(
        &self,
        settings: &Settings,
        request: &Request<'_>,
    ) -> Result<InsertionResult, String> {
        let _one_at_a_time = self.insert_lock.lock().unwrap_or_else(|p| p.into_inner());
        let session = self.session();
        let probed = self.probe(&session, settings);
        chain::check_guards(
            &request.guards,
            probed.target.as_ref(),
            probed.window.as_deref(),
        )
        .map_err(|r| match r {
            Refusal::Conflict(m) => m,
            other => format!("{other:?}"),
        })?;
        let target = probed.target.as_ref();
        if target.is_some_and(|t| t.is_dettivo) && request.allow_self_target {
            tracing::info!(
                probe = probed.probe,
                "insertion into a Dettivo window allowed: the self-target allowance is armed"
            );
        } else if target.is_some_and(|t| t.is_dettivo) {
            tracing::info!(
                probe = probed.probe,
                "insertion refused: target is a Dettivo window"
            );
            return Ok(result(
                InsertionOutcome::Failed,
                InsertionMethod::Paste,
                target,
                Some("target_is_self"),
                None,
            ));
        }
        let backends = backend::for_session(&session);
        let candidates = self.candidates(&backends, &session);
        let facts = TargetFacts::of(target);
        let choice = match chain::choose(&candidates, settings, request.text, request.mode, facts) {
            Ok(c) => c,
            Err(Refusal::NothingAvailable(reason)) => {
                tracing::warn!(chars = request.text.chars().count(), reason = %reason, "no insertion backend");
                return Ok(result(
                    InsertionOutcome::Failed,
                    InsertionMethod::FallbackCopy,
                    target,
                    Some(&reason),
                    None,
                ));
            }
            Err(other) => return Err(format!("{other:?}")),
        };
        for (name, reason) in &choice.skipped {
            tracing::info!(backend = %name, reason = %reason, "backend skipped");
        }
        // The decision above is checked again right before a backend
        // delivers (and between its typing batches): the window that has
        // the focus then must still be the one probed here, so a portal
        // dialog or a long take cannot hand the text to another window,
        // Dettivo's own included.
        let origin = Origin::of(target, probed.window.as_deref());
        let recheck = || {
            let now = self.probe(&session, settings);
            delivery::moved(
                origin.as_ref(),
                Origin::of(now.target.as_ref(), now.window.as_deref()).as_ref(),
            )
        };
        let app_id = target.map(|t| t.app_id.clone()).unwrap_or_default();
        let ctx = Ctx {
            session: &session,
            settings,
            app_id: &app_id,
            keystroke: chain::keystroke_sender(&candidates, facts),
            recheck: Some(&recheck),
        };
        let delivered = delivery::deliver(
            &candidates,
            &choice.order,
            request.text,
            request.mode == InsertMode::ClipboardOnly,
            &ctx,
        );
        if let Some((backend, chars)) = delivered.typed {
            *self.undo.lock().unwrap_or_else(|p| p.into_inner()) = Some(UndoRecord {
                backend,
                chars,
                at: Instant::now(),
                target: target.cloned(),
            });
        }
        Ok(result(
            delivered.outcome,
            delivered.method,
            target,
            delivered.reason.as_deref(),
            delivered.backend,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::RawWindow;
    use crate::session::Mock;

    fn window(app_id: &str, pid: u32, window: &str) -> Option<RawWindow> {
        Some(RawWindow {
            app_id: app_id.into(),
            pid: Some(pid),
            title: "t".into(),
            xwayland: false,
            window: Some(window.into()),
        })
    }

    /// A service over the QA mock backends whose probe answers `windows`
    /// in order; the file the mock appends to says what was typed.
    fn scripted_service(
        windows: Vec<Option<RawWindow>>,
    ) -> (Service, std::path::PathBuf, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("inserted.txt");
        let inserted = file.clone();
        let service = Service::new(move || Session {
            mock: Some(Mock {
                inserted_file: inserted.clone(),
            }),
            ..Session::default()
        });
        *service.scripted_probe.lock().unwrap() = windows.into();
        (service, file, dir)
    }

    fn request<'a>(text: &'a str, window: Option<&'a str>) -> Request<'a> {
        Request {
            text,
            mode: InsertMode::Raw,
            guards: Guards {
                app_id: Some("foot"),
                pid: Some("42"),
                window,
            },
            allow_self_target: false,
        }
    }

    /// The window that has the focus right before the backend types must
    /// be the one the chain decided on: a second window of the same
    /// process, or another app, refuses the delivery with the target
    /// named as changed and nothing typed.
    #[test]
    fn the_origin_is_checked_again_right_before_delivery() {
        let settings = Settings::default();
        let (service, file, _dir) =
            scripted_service(vec![window("foot", 42, "0x1"), window("foot", 42, "0x2")]);
        let done = service
            .insert(&settings, &request("hello", Some("0x1")))
            .unwrap();
        assert_eq!(done.outcome, InsertionOutcome::Failed);
        assert_eq!(
            done.reason.as_deref(),
            Some("target_changed: focus moved to foot")
        );
        assert!(!file.exists(), "nothing was typed");

        let (service, file, _dir) =
            scripted_service(vec![window("foot", 42, "0x1"), window("foot", 42, "0x1")]);
        let done = service
            .insert(&settings, &request("hello", Some("0x1")))
            .unwrap();
        assert_eq!(done.outcome, InsertionOutcome::Inserted);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "hello\n");
    }

    /// A guard on the window identity refuses a second window of the same
    /// process before any backend runs.
    #[test]
    fn the_window_guard_refuses_another_window_of_the_same_process() {
        let settings = Settings::default();
        let (service, file, _dir) = scripted_service(vec![window("foot", 42, "0x2")]);
        let err = service
            .insert(&settings, &request("hello", Some("0x1")))
            .unwrap_err();
        assert_eq!(err, "Frontmost window mismatch");
        assert!(!file.exists());
    }
}
