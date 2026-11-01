//! The session's inserter: the backend chain from `dettivo-insert` with
//! the window captured at the session's start as the guard, so the text
//! only lands where the user was when the key went down (S-08 guards). A
//! mismatch at insertion time is reported as a failed insertion with a
//! `CONFLICT` reason; the transcript is kept either way. A take that
//! continues the previous one in the same window is joined to it first
//! (ADR 0091).

use std::borrow::Cow;
use std::sync::{Arc, RwLock};

use dettivo_core::config::Loaded;
use dettivo_insert::chain::Guards;
use dettivo_insert::join::{Continuation, Place};
use dettivo_insert::{Inserter as ChainInsert, Request, Service, Settings};
use dettivo_proto::capabilities::InsertMode;
use dettivo_proto::runtime::{InsertionMethod, InsertionOutcome, InsertionResult, TargetApp};
use dettivo_session::{Inserter, SessionTarget, no_context_pack};

use crate::self_target::{APP_IDS, SelfTargetAllowance};

/// The bridge.
pub struct ChainInserter {
    service: Arc<Service>,
    config: Arc<RwLock<Loaded>>,
    self_target: Arc<SelfTargetAllowance>,
    joins: Arc<Continuation>,
}

impl ChainInserter {
    /// Over the daemon's service, configuration, self-target allowance
    /// and the last delivery a continuation joins.
    pub fn new(
        service: Arc<Service>,
        config: Arc<RwLock<Loaded>>,
        self_target: Arc<SelfTargetAllowance>,
        joins: Arc<Continuation>,
    ) -> Self {
        Self {
            service,
            config,
            self_target,
            joins,
        }
    }
}

/// The `[insert]` settings of a configuration snapshot.
pub fn insert_settings(loaded: &Loaded) -> Settings {
    let insert = loaded.config.insert.clone();
    Settings {
        backend: insert.backend,
        paste_keys: insert.paste_keys,
        terminal_app_ids: insert.terminal_app_ids,
        self_app_ids: insert.self_app_ids,
        inter_key_delay_ms: insert.inter_key_delay_ms,
        restore_clipboard: insert.restore_clipboard,
        clipboard_restore_delay_ms: insert.clipboard_restore_delay_ms,
        undo_window_ms: insert.undo_window_ms,
    }
}

/// When the text landed: now, or for a paste whose clipboard was put
/// back, before the restore delay, so typing during that delay counts as
/// input after the delivery.
fn landed(result: &InsertionResult, settings: &Settings) -> std::time::Instant {
    let now = std::time::Instant::now();
    let pasted = result
        .backend
        .as_ref()
        .is_some_and(|b| b.name == "clipboard_paste");
    if pasted && settings.restore_clipboard {
        let delay = std::time::Duration::from_millis(settings.clipboard_restore_delay_ms);
        now.checked_sub(delay).unwrap_or(now)
    } else {
        now
    }
}

/// The window a take was captured in, when the probe named it whole: a
/// join needs the window itself, not only its process.
fn place_of(target: Option<&SessionTarget>) -> Option<Place> {
    let t = target.filter(|t| !t.unverified)?;
    Some(Place {
        app_id: t.app_id.clone()?,
        pid: t.pid,
        window: t.window.clone()?,
    })
}

/// The result for a guard mismatch.
pub fn conflict_result(target: Option<&SessionTarget>, message: &str) -> InsertionResult {
    let app_id = target.and_then(|t| t.app_id.clone()).unwrap_or_default();
    InsertionResult {
        outcome: InsertionOutcome::Failed,
        method: InsertionMethod::Paste,
        target_app: TargetApp {
            bundle_id: app_id.clone(),
            name: app_id,
        },
        context_pack: no_context_pack(),
        reason: Some(format!("CONFLICT: {message}")),
        backend: None,
    }
}

/// The reason an unverified origin's clipboard result carries.
pub const ORIGIN_UNVERIFIED: &str = "origin_unverified";

/// The mode a session's take is inserted in: raw typing into the window
/// captured at start, or the clipboard alone when the probe could not
/// name that window, so the text never lands in whatever has the focus
/// once the take is transcribed.
pub fn mode_for(target: Option<&SessionTarget>) -> InsertMode {
    if target.is_some_and(|t| t.unverified) {
        InsertMode::ClipboardOnly
    } else {
        InsertMode::Raw
    }
}

/// Names the unverified origin on a clipboard result that has no other
/// reason, so the OSD and the history say why nothing was typed.
pub fn annotate(mut result: InsertionResult, target: Option<&SessionTarget>) -> InsertionResult {
    if target.is_some_and(|t| t.unverified)
        && result.outcome == InsertionOutcome::CopiedToClipboard
        && result.reason.is_none()
    {
        result.reason = Some(ORIGIN_UNVERIFIED.to_string());
    }
    result
}

impl Inserter for ChainInserter {
    fn insert(&self, text: &str, target: Option<&SessionTarget>) -> InsertionResult {
        let (settings, join) = {
            let loaded = self.config.read().unwrap_or_else(|p| p.into_inner());
            (
                insert_settings(&loaded),
                loaded.config.dictation.join_continuations,
            )
        };
        let place = place_of(target);
        let text = if join {
            self.joins.text_for(place.as_ref(), text)
        } else {
            Cow::Borrowed(text)
        };
        let pid = target.and_then(|t| t.pid).map(|p| p.to_string());
        // The allowance covers the app's own window ids only: a take whose
        // key went down over the pill or the sheet is refused as ever.
        let app_window = target
            .and_then(|t| t.app_id.as_deref())
            .is_some_and(|id| APP_IDS.iter().any(|a| a.eq_ignore_ascii_case(id)));
        let request = Request {
            text: &text,
            mode: mode_for(target),
            guards: Guards {
                app_id: target.and_then(|t| t.app_id.as_deref()),
                pid: pid.as_deref(),
                window: target.and_then(|t| t.window.as_deref()),
            },
            allow_self_target: app_window && self.self_target.armed(),
        };
        let result = match self.service.insert(&settings, &request) {
            Ok(result) => annotate(result, target),
            Err(message) => {
                tracing::info!(reason = %message, "insertion refused: the focus moved since the session started");
                conflict_result(target, &message)
            }
        };
        let typed = result.outcome == InsertionOutcome::Inserted;
        self.joins
            .delivered(place.filter(|_| typed), &text, landed(&result, &settings));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copied() -> InsertionResult {
        InsertionResult {
            outcome: InsertionOutcome::CopiedToClipboard,
            method: InsertionMethod::ClipboardOnly,
            target_app: TargetApp {
                bundle_id: String::new(),
                name: String::new(),
            },
            context_pack: no_context_pack(),
            reason: None,
            backend: None,
        }
    }

    /// An origin the probe could not verify goes to the clipboard with
    /// the reason named; a verified origin and an unguarded re-insert type.
    #[test]
    fn an_unverified_origin_is_clipboard_only_and_says_so() {
        let unverified = SessionTarget::unverified();
        assert_eq!(mode_for(Some(&unverified)), InsertMode::ClipboardOnly);
        assert_eq!(
            annotate(copied(), Some(&unverified)).reason.as_deref(),
            Some(ORIGIN_UNVERIFIED)
        );
        let verified = SessionTarget {
            app_id: Some("foot".into()),
            pid: Some(42),
            window: Some("0x1".into()),
            unverified: false,
        };
        assert_eq!(mode_for(Some(&verified)), InsertMode::Raw);
        assert_eq!(annotate(copied(), Some(&verified)).reason, None);
        assert_eq!(mode_for(None), InsertMode::Raw);
        assert_eq!(annotate(copied(), None).reason, None);
    }

    /// A paste lands before the restore delay, so typing during the delay
    /// is input after the delivery; typed text lands at once.
    #[test]
    fn a_restored_paste_landed_before_its_restore_delay() {
        let mut result = copied();
        let settings = Settings::default();
        let now = std::time::Instant::now();
        assert!(landed(&result, &settings) >= now);
        result.backend = Some(dettivo_proto::runtime::InsertionBackend {
            name: "clipboard_paste".into(),
            latency_ms: 600,
            undo_supported: false,
        });
        let delay = std::time::Duration::from_millis(settings.clipboard_restore_delay_ms);
        assert!(landed(&result, &settings) <= now - delay + std::time::Duration::from_millis(50));
    }

    struct Quiet;
    impl dettivo_insert::idle::InputActivity for Quiet {
        fn active_between(&self, _: std::time::Instant, _: std::time::Instant) -> Option<bool> {
            Some(false)
        }
    }

    /// fn-85 R3 through the chain: a second quiet take into the window
    /// the first one was typed into continues it; `join_continuations =
    /// false` types it as it is, and an unverified origin never joins.
    #[test]
    fn a_quiet_second_take_continues_the_first_unless_joins_are_off() {
        for join in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join("inserted.txt");
            let mock = dettivo_insert::Mock {
                inserted_file: file.clone(),
            };
            let service = Arc::new(Service::new(move || dettivo_insert::Session {
                mock: Some(mock.clone()),
                ..dettivo_insert::Session::default()
            }));
            let mut loaded = Loaded::load(&dir.path().join("none.toml"), |_| None);
            loaded.config.dictation.join_continuations = join;
            let joins = Arc::new(Continuation::new(Arc::new(Quiet)));
            let inserter = ChainInserter::new(
                service,
                Arc::new(RwLock::new(loaded)),
                Arc::new(SelfTargetAllowance::default()),
                joins.clone(),
            );
            let target = SessionTarget {
                app_id: Some(dettivo_insert::probe::MOCK_APP_ID.into()),
                pid: Some(dettivo_insert::probe::MOCK_PID),
                window: Some("mock-window".into()),
                unverified: false,
            };
            inserter.insert("I wrote the report", Some(&target));
            joins.take_started(std::time::Instant::now());
            inserter.insert("And then I left.", Some(&target));
            joins.take_started(std::time::Instant::now());
            inserter.insert("Unrelated", Some(&SessionTarget::unverified()));
            let typed = std::fs::read_to_string(&file).unwrap();
            let second = if join {
                " and then I left."
            } else {
                "And then I left."
            };
            assert_eq!(
                typed.lines().take(2).collect::<Vec<_>>(),
                ["I wrote the report", second],
                "join_continuations = {join}"
            );
        }
    }
}
