//! Binding a portal session: create it, bind the shortcuts, and after a
//! portal backend restart bind again, retrying a backend still starting
//! but never a refusal. Every wait gives way to the stop flag.

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use ashpd::desktop::global_shortcuts::{
    BindShortcuts, BindShortcutsOptions, GlobalShortcuts, NewShortcut,
};
use ashpd::desktop::{CreateSessionOptions, Session};

use super::{BIND_TIMEOUT, PortalBackend, REBIND_TRIES};
use crate::identity::FIX;
use crate::{Identity, Keys, Registration};

/// How often a wait looks at the stop flag.
pub(super) const STOP_POLL: Duration = Duration::from_millis(50);

/// How long closing a session may take on the way out.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);

/// Why a session was not bound.
pub(super) enum Unbound {
    /// The portal (or a backend still starting) failed the request; a
    /// later try may work.
    Failed(String),
    /// The portal answered the bind with a refusal: the person dismissed
    /// or denied it, or left it unanswered. Asking again is not ours.
    Refused(String),
    /// The backend was stopped while waiting.
    Stopped,
}

impl Unbound {
    pub(super) fn reason(self) -> String {
        match self {
            Self::Failed(r) | Self::Refused(r) => r,
            Self::Stopped => "the portal backend was stopped".to_string(),
        }
    }
}

/// `work`, unless `stop` is set first.
async fn unless_stopped<T>(stop: &AtomicBool, work: impl Future<Output = T>) -> Option<T> {
    let stopped = async {
        while !stop.load(Ordering::Relaxed) {
            tokio::time::sleep(STOP_POLL).await;
        }
    };
    tokio::select! {
        done = work => Some(done),
        () = stopped => None,
    }
}

/// Closes `session`, giving a portal that went away [`CLOSE_TIMEOUT`].
pub(super) async fn close(session: &Session<GlobalShortcuts>) {
    let _ = tokio::time::timeout(CLOSE_TIMEOUT, session.close()).await;
}

impl PortalBackend {
    /// Creates a session and binds `shortcuts` in it; a stop while waiting
    /// closes the session it made.
    pub(super) async fn open(
        portal: &GlobalShortcuts,
        shortcuts: &[NewShortcut],
        stop: &AtomicBool,
    ) -> Result<(Session<GlobalShortcuts>, BindShortcuts), Unbound> {
        let session = unless_stopped(stop, portal.create_session(CreateSessionOptions::default()))
            .await
            .ok_or(Unbound::Stopped)?
            .map_err(|e| {
                let refused = format!("portal session refused: {e}");
                Unbound::Failed(if refused.contains("app id") {
                    format!("{refused}; {FIX}")
                } else {
                    refused
                })
            })?;
        let asked = tokio::time::timeout(
            BIND_TIMEOUT,
            portal.bind_shortcuts(&session, shortcuts, None, BindShortcutsOptions::default()),
        );
        let bound = match unless_stopped(stop, asked).await {
            None => Err(Unbound::Stopped),
            Some(Err(_)) => Err(Unbound::Refused(
                "the portal did not answer the bind request".to_string(),
            )),
            Some(Ok(Err(e))) => Err(Unbound::Failed(format!("portal bind refused: {e}"))),
            Some(Ok(Ok(request))) => request
                .response()
                .map_err(|e| Unbound::Refused(format!("the portal denied the shortcuts: {e}"))),
        };
        match bound {
            Ok(bound) => Ok((session, bound)),
            Err(e) => {
                close(&session).await;
                Err(e)
            }
        }
    }

    /// Binds again in a new session once a portal backend that went away
    /// is back: the backend that held the session (on Hyprland,
    /// `xdg-desktop-portal-hyprland`, which exits with the compositor)
    /// took the bindings with it. A backend still starting gets
    /// [`REBIND_TRIES`] tries, the wait doubling from 250 ms; a refused
    /// bind or a stop ends it at once.
    pub(super) async fn rebind(
        portal: &GlobalShortcuts,
        shortcuts: &[NewShortcut],
        stop: &AtomicBool,
    ) -> Result<(Session<GlobalShortcuts>, BindShortcuts), Unbound> {
        let mut wait = Duration::from_millis(250);
        let mut failed = Unbound::Stopped;
        for tried in 0..REBIND_TRIES {
            if tried > 0 {
                unless_stopped(stop, tokio::time::sleep(wait))
                    .await
                    .ok_or(Unbound::Stopped)?;
                wait *= 2;
            }
            if stop.load(Ordering::Relaxed) {
                return Err(Unbound::Stopped);
            }
            match Self::open(portal, shortcuts, stop).await {
                Err(Unbound::Failed(e)) => {
                    tracing::warn!(error = %e, "hotkeys: binding again after the portal backend restarted failed");
                    failed = Unbound::Failed(e);
                }
                done => return done,
            }
        }
        Err(failed)
    }

    /// What `bound` delivers.
    pub(super) fn registration(
        keys: &Keys,
        identity: Identity,
        bound: &BindShortcuts,
    ) -> Registration {
        // Hyprland's portal assigns no keys: the user binds them in the
        // compositor's configuration, which the snippet does.
        let compositor_bound = !bound.shortcuts().is_empty()
            && bound
                .shortcuts()
                .iter()
                .all(|s| s.trigger_description().is_empty());
        Registration {
            backend: "portal",
            bound: bound
                .shortcuts()
                .iter()
                .filter_map(|s| keys.action_for(s.id()))
                .collect(),
            identity: Some(identity),
            compositor_bound,
        }
    }
}
