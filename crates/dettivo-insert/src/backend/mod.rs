//! The backends of the chain (FR-I1). Each one says whether it can run in
//! this session and why not, whether it can type a given text, and
//! performs the insertion; the chain decides which one runs.

pub mod clipboard;
pub mod commands;
pub mod libei;
pub mod mock;
pub mod virtual_keyboard;

use std::time::Duration;

use dettivo_proto::runtime::{InsertionMethod, InsertionOutcome};

use crate::keymap::Unmappable;
use crate::session::Session;
use crate::settings::{PasteKeys, Settings};

/// Whether a backend can run right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    /// It can.
    Available,
    /// It cannot; the reason names the missing piece.
    Unavailable(String),
}

impl Availability {
    /// True when available.
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }

    /// The reason when unavailable.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Available => None,
            Self::Unavailable(r) => Some(r),
        }
    }
}

/// How a backend delivers text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Simulated key presses into the focused window.
    Keystroke,
    /// The clipboard plus a simulated paste keystroke.
    ClipboardPaste,
    /// The clipboard only; the user pastes.
    ClipboardOnly,
}

/// The origin check a backend runs right before it delivers, and again
/// between typing batches: the focused window is still the one the
/// chain decided on. `Err` names where the focus went.
pub type Recheck<'a> = &'a (dyn Fn() -> Result<(), String> + Sync);

/// What a backend needs while it runs.
pub struct Ctx<'a> {
    /// The session facts.
    pub session: &'a Session,
    /// The settings in force.
    pub settings: &'a Settings,
    /// The app id of the target (for the paste keystroke choice).
    pub app_id: &'a str,
    /// The keystroke-capable backend the clipboard path pastes with.
    pub keystroke: Option<&'a dyn Backend>,
    /// The origin check; `None` when nothing guards the delivery.
    pub recheck: Option<Recheck<'a>>,
}

impl Ctx<'_> {
    /// The pause between typed keys.
    pub fn key_delay(&self) -> Duration {
        Duration::from_millis(self.settings.inter_key_delay_ms)
    }

    /// Runs the origin check; a focus that moved is `Failure::FocusMoved`
    /// with nothing delivered yet.
    pub fn recheck(&self) -> Result<(), Failure> {
        match self.recheck {
            Some(check) => check().map_err(Failure::FocusMoved),
            None => Ok(()),
        }
    }
}

/// Why a backend did not perform, and what that means for the text: the
/// chain tries the next backend only when nothing reached the target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The backend stopped before delivering anything; another backend
    /// may take the text.
    Before(String),
    /// Delivery had begun, or cannot be ruled out (a helper that exited
    /// or timed out while typing): the text is never typed again.
    During(String),
    /// The focused window changed before delivery; nothing was typed and
    /// nothing is retried.
    FocusMoved(String),
}

impl Failure {
    /// The reason.
    pub fn reason(&self) -> &str {
        match self {
            Self::Before(r) | Self::During(r) | Self::FocusMoved(r) => r,
        }
    }

    /// A focus move that found nothing delivered yet is `FocusMoved`; one
    /// that interrupted typing stays a partial delivery.
    pub fn into_focus_moved(self) -> Self {
        match self {
            Self::Before(r) => Self::FocusMoved(r),
            other => other,
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.reason())
    }
}

/// What a backend did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Performed {
    /// How it concluded.
    pub outcome: InsertionOutcome,
    /// The contract method name.
    pub method: InsertionMethod,
    /// Number of characters typed (for undo).
    pub typed_chars: usize,
}

impl Performed {
    /// A keystroke insertion of `chars` characters.
    pub fn typed(chars: usize) -> Self {
        Self {
            outcome: InsertionOutcome::Inserted,
            method: InsertionMethod::Paste,
            typed_chars: chars,
        }
    }

    /// A clipboard-only insertion.
    pub fn copied() -> Self {
        Self {
            outcome: InsertionOutcome::CopiedToClipboard,
            method: InsertionMethod::ClipboardOnly,
            typed_chars: 0,
        }
    }
}

/// One backend of the chain.
pub trait Backend: Send + Sync {
    /// The name reported in `InsertionResult.backend`.
    fn name(&self) -> &'static str;
    /// How it delivers text.
    fn kind(&self) -> Kind;
    /// Whether it can run in `session`, and why not.
    fn availability(&self, session: &Session) -> Availability;
    /// Whether it can type every character of `text`; a keystroke backend
    /// that cannot is demoted for that text (FR-I4).
    fn covers(&self, text: &str) -> Result<(), Unmappable> {
        let _ = text;
        Ok(())
    }
    /// Whether its keys reach an X client under XWayland. A Wayland
    /// virtual keyboard's keymap does not (XWayland keeps the seat's
    /// keymap), so it is demoted for such targets.
    fn reaches_xwayland(&self) -> bool {
        true
    }
    /// Performs the insertion, running `ctx.recheck()` right before the
    /// first key and between typing batches; a failure says whether any
    /// of the text may have reached the target.
    fn insert(&self, text: &str, ctx: &Ctx<'_>) -> Result<Performed, Failure>;
    /// Sends a paste keystroke (keystroke backends only).
    fn paste_keystroke(&self, keys: PasteKeys, ctx: &Ctx<'_>) -> Result<(), String> {
        let _ = (keys, ctx);
        Err(format!("{} cannot send keystrokes", self.name()))
    }
    /// True only when the backend can verify the inserted range or transaction.
    fn undo_supported(&self) -> bool {
        false
    }
    /// Undoes a verified insertion transaction.
    fn undo(&self, chars: usize, ctx: &Ctx<'_>) -> Result<(), String> {
        let _ = (chars, ctx);
        Err(format!("{} cannot undo", self.name()))
    }
}

/// Every real backend in FR-I1 order.
pub fn all() -> Vec<Box<dyn Backend>> {
    vec![
        Box::new(virtual_keyboard::VirtualKeyboard),
        Box::new(libei::Libei),
        Box::new(commands::Ydotool),
        Box::new(commands::Xdotool),
        Box::new(clipboard::ClipboardPaste),
        Box::new(clipboard::ClipboardOnly),
    ]
}

/// The backends for a session: the mock alone under the QA mock, the
/// real chain otherwise.
pub fn for_session(session: &Session) -> Vec<Box<dyn Backend>> {
    match &session.mock {
        Some(mock) => vec![
            Box::new(mock::MockBackend {
                inserted_file: mock.inserted_file.clone(),
                kind: Kind::Keystroke,
            }),
            Box::new(mock::MockBackend {
                inserted_file: mock.inserted_file.clone(),
                kind: Kind::ClipboardOnly,
            }),
        ],
        None => all(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unverified_inserted_ranges_never_advertise_undo() {
        for backend in all() {
            assert!(
                !backend.undo_supported(),
                "{} cannot identify the inserted range",
                backend.name()
            );
        }
    }

    #[test]
    fn the_chain_is_in_fr_i1_order() {
        let names: Vec<&str> = all().iter().map(|b| b.name()).collect();
        assert_eq!(names, crate::settings::BACKEND_NAMES);
        let empty = Session::default();
        for backend in all() {
            assert!(
                !backend.availability(&empty).is_available(),
                "{} claims availability with no display",
                backend.name()
            );
        }
    }
}
