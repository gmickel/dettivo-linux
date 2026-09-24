//! The session's inserter: the backend chain from `dettivo-insert` with
//! the window captured at the session's start as the guard, so the text
//! only lands where the user was when the key went down (S-08 guards). A
//! mismatch at insertion time is reported as a failed insertion with a
//! `CONFLICT` reason; the transcript is kept either way.

use std::sync::{Arc, RwLock};

use dettivo_core::config::Loaded;
use dettivo_insert::chain::Guards;
use dettivo_insert::{Inserter as ChainInsert, Request, Service};
use dettivo_proto::capabilities::InsertMode;
use dettivo_proto::runtime::{InsertionMethod, InsertionOutcome, InsertionResult, TargetApp};
use dettivo_session::{Inserter, SessionTarget, no_context_pack};

use crate::self_target::{APP_IDS, SelfTargetAllowance};

/// The bridge.
pub struct ChainInserter {
    service: Arc<Service>,
    config: Arc<RwLock<Loaded>>,
    self_target: Arc<SelfTargetAllowance>,
}

impl ChainInserter {
    /// Over the daemon's service, configuration and self-target allowance.
    pub fn new(
        service: Arc<Service>,
        config: Arc<RwLock<Loaded>>,
        self_target: Arc<SelfTargetAllowance>,
    ) -> Self {
        Self {
            service,
            config,
            self_target,
        }
    }
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
        let settings =
            crate::daemon::insert_settings(&self.config.read().unwrap_or_else(|p| p.into_inner()));
        let pid = target.and_then(|t| t.pid).map(|p| p.to_string());
        // The allowance covers the app's own window ids only: a take whose
        // key went down over the pill or the sheet is refused as ever.
        let app_window = target
            .and_then(|t| t.app_id.as_deref())
            .is_some_and(|id| APP_IDS.iter().any(|a| a.eq_ignore_ascii_case(id)));
        let request = Request {
            text,
            mode: mode_for(target),
            guards: Guards {
                app_id: target.and_then(|t| t.app_id.as_deref()),
                pid: pid.as_deref(),
                window: target.and_then(|t| t.window.as_deref()),
            },
            allow_self_target: app_window && self.self_target.armed(),
        };
        match self.service.insert(&settings, &request) {
            Ok(result) => annotate(result, target),
            Err(message) => {
                tracing::info!(reason = %message, "insertion refused: the focus moved since the session started");
                conflict_result(target, &message)
            }
        }
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
}
