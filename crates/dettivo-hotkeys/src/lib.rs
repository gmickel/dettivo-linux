//! Hotkeys (FR-H2 to FR-H6): the four actions a key can reach, the chords
//! that name them in `config.toml`, the binding snippets `dettivo setup`
//! writes for Hyprland, Sway and Niri, and the two daemon-side backends
//! (the portal's GlobalShortcuts and evdev) that deliver presses and
//! releases to the daemon where a compositor cannot run the CLI itself.
//! MPRIS pause and resume live here too, because they are what a press
//! and a release do to the rest of the desktop.

pub mod chord;
#[cfg(feature = "backends")]
pub mod evdev;
pub mod install;
#[cfg(feature = "mock")]
pub mod mock;
#[cfg(feature = "backends")]
pub mod mpris;
#[cfg(feature = "backends")]
pub mod portal;
pub mod snippet;

use std::sync::Arc;

use chord::{Chord, ChordError};

/// Package name of this crate, as declared in its manifest.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Package names of the workspace crates this crate builds on.
pub const UPSTREAM: &[&str] = &[dettivo_proto::CRATE_NAME];

/// What a key can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Press starts a session, release stops it.
    PushToTalk,
    /// One press starts, the next stops.
    Toggle,
    /// Drops the session.
    Cancel,
    /// Hands the last transcript over again.
    ReinsertLast,
}

impl Action {
    /// Every action, in the order the snippets and the portal list them.
    pub const ALL: [Action; 4] = [
        Self::PushToTalk,
        Self::Toggle,
        Self::Cancel,
        Self::ReinsertLast,
    ];

    /// The stable id: the portal's shortcut id, the status wire value and
    /// the config key it takes its chord from.
    pub fn id(self) -> &'static str {
        match self {
            Self::PushToTalk => "push_to_talk",
            Self::Toggle => "toggle",
            Self::Cancel => "cancel",
            Self::ReinsertLast => "reinsert_last",
        }
    }

    /// The action for an id.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.id() == id)
    }

    /// One line for a portal dialog or a snippet comment.
    pub fn description(self) -> &'static str {
        match self {
            Self::PushToTalk => "Dettivo: hold to talk",
            Self::Toggle => "Dettivo: toggle dictation",
            Self::Cancel => "Dettivo: cancel dictation",
            Self::ReinsertLast => "Dettivo: insert the last transcript again",
        }
    }
}

/// What a backend delivers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    /// The chord went down.
    Press(Action),
    /// The chord came up.
    Release(Action),
}

/// Receives the events; called on the backend's own thread.
pub type Handler = Arc<dyn Fn(HotkeyEvent) + Send + Sync>;

/// Whether a backend can run in this session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    /// It can.
    Available,
    /// It cannot, and this is why.
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

/// What a started backend bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    /// The backend's name.
    pub backend: &'static str,
    /// The actions the backend delivers.
    pub bound: Vec<Action>,
}

/// A source of presses and releases.
pub trait HotkeyBackend: Send + Sync {
    /// `portal` or `evdev`.
    fn name(&self) -> &'static str;
    /// Whether it can run right now, without starting it.
    fn availability(&self) -> Availability;
    /// Binds the actions and delivers events to `handler` until `stop`.
    /// A refusal is returned once; the backend never retries on its own.
    fn start(&self, handler: Handler) -> Result<Registration, String>;
    /// Releases the bindings and ends the delivery thread.
    fn stop(&self);
}

/// The chords of the four actions, as `[hotkeys]` names them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keys {
    /// `hold`: push to talk.
    pub hold: Chord,
    /// `toggle`.
    pub toggle: Chord,
    /// `cancel`.
    pub cancel: Chord,
    /// `reinsert`.
    pub reinsert: Chord,
}

impl Keys {
    /// Omarchy's Voxtype keys, so the same muscle memory works: hold
    /// F9 to talk, Super+Ctrl+X to toggle.
    pub const DEFAULT_HOLD: &'static str = "F9";
    /// Toggle.
    pub const DEFAULT_TOGGLE: &'static str = "SUPER CTRL, X";
    /// Cancel.
    pub const DEFAULT_CANCEL: &'static str = "SUPER CTRL, Escape";
    /// Re-insert; Super+Ctrl+R is Omarchy's reminder key, so Shift joins
    /// the toggle chord instead.
    pub const DEFAULT_REINSERT: &'static str = "SUPER CTRL SHIFT, X";

    /// Parses the four chords; the error names the key and the notation.
    pub fn parse(
        hold: &str,
        toggle: &str,
        cancel: &str,
        reinsert: &str,
    ) -> Result<Self, ChordError> {
        Ok(Self {
            hold: Chord::parse(hold).map_err(|e| e.for_key("hotkeys.hold"))?,
            toggle: Chord::parse(toggle).map_err(|e| e.for_key("hotkeys.toggle"))?,
            cancel: Chord::parse(cancel).map_err(|e| e.for_key("hotkeys.cancel"))?,
            reinsert: Chord::parse(reinsert).map_err(|e| e.for_key("hotkeys.reinsert"))?,
        })
    }

    /// The defaults.
    pub fn defaults() -> Self {
        Self::parse(
            Self::DEFAULT_HOLD,
            Self::DEFAULT_TOGGLE,
            Self::DEFAULT_CANCEL,
            Self::DEFAULT_REINSERT,
        )
        .expect("the default chords parse")
    }

    /// The chord of an action.
    pub fn chord(&self, action: Action) -> &Chord {
        match action {
            Action::PushToTalk => &self.hold,
            Action::Toggle => &self.toggle,
            Action::Cancel => &self.cancel,
            Action::ReinsertLast => &self.reinsert,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, "dettivo-hotkeys");
        assert_eq!(UPSTREAM, &["dettivo-proto"]);
    }

    #[test]
    fn action_ids_round_trip() {
        for a in Action::ALL {
            assert_eq!(Action::from_id(a.id()), Some(a));
        }
        assert_eq!(Action::from_id("nope"), None);
    }

    #[test]
    fn default_keys_match_the_config_schema_defaults() {
        let schema = dettivo_core::config::Config::default().hotkeys;
        assert_eq!(schema.hold, Keys::DEFAULT_HOLD);
        assert_eq!(schema.toggle, Keys::DEFAULT_TOGGLE);
        assert_eq!(schema.cancel, Keys::DEFAULT_CANCEL);
        assert_eq!(schema.reinsert, Keys::DEFAULT_REINSERT);
        let keys = Keys::defaults();
        assert_eq!(keys.chord(Action::PushToTalk).key.name(), "F9");
    }

    #[test]
    fn a_bad_chord_names_its_config_key() {
        let err = Keys::parse("F9", "SUPER,", "F1", "F2").unwrap_err();
        assert_eq!(err.key.as_deref(), Some("hotkeys.toggle"));
        assert!(err.to_string().contains("hotkeys.toggle"), "{err}");
    }
}
