//! The hold key's state machine (ADR 0081). Every path that reports a
//! press and a release of the hold chord feeds it: the compositor
//! bindings through `dettivo dictation press` and `release`, the portal's
//! `Activated` and `Deactivated`, and evdev. It survives a release the
//! compositor dropped (the next press finishes the take), ignores a held
//! key's repeat, and locks recording after a double tap.

use std::time::{Duration, Instant};

/// A second press this soon after a tap's release locks recording.
pub const DOUBLE_TAP: Duration = Duration::from_millis(300);
/// A press this soon after the hold began is the key repeating, not a
/// new press.
pub const REPEAT_FILTER: Duration = Duration::from_secs(1);
/// A release this soon after the press is a tap: the take is dropped and
/// the key waits for a second tap.
pub const MIN_HOLD: Duration = Duration::from_millis(300);

/// Where the hold key is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoldState {
    /// Nothing held.
    #[default]
    Idle,
    /// The key is down and a take records; `second_tap` when this press
    /// followed a tap, so its release locks.
    Held {
        /// When the press arrived.
        since: Instant,
        /// The press followed a tap within [`DOUBLE_TAP`].
        second_tap: bool,
    },
    /// Hands-free: the take records until the next press.
    Locked,
    /// A tap was released at `at`; a press within [`DOUBLE_TAP`] locks.
    LastTap {
        /// When the tap was released.
        at: Instant,
    },
}

impl HoldState {
    /// The wire name: `idle`, `held`, `locked` or `tapped`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Held { .. } => "held",
            Self::Locked => "locked",
            Self::LastTap { .. } => "tapped",
        }
    }

    /// True while the machine owns a recording take.
    pub fn records(self) -> bool {
        matches!(self, Self::Held { .. } | Self::Locked)
    }
}

/// What a transition asks of the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldAction {
    /// Nothing.
    None,
    /// Start a take.
    Start,
    /// Finish the take.
    Stop,
    /// Keep the take recording after the release (hands-free).
    Lock,
    /// Drop the take a tap started.
    DiscardTap,
}

impl HoldAction {
    /// The wire name: `none`, `started`, `stopped`, `locked`, `discarded`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Start => "started",
            Self::Stop => "stopped",
            Self::Lock => "locked",
            Self::DiscardTap => "discarded",
        }
    }
}

/// The hold chord went down at `now`.
pub fn on_press(state: HoldState, now: Instant) -> (HoldState, HoldAction) {
    let held = |second_tap| HoldState::Held {
        since: now,
        second_tap,
    };
    match state {
        HoldState::LastTap { at } if now.saturating_duration_since(at) <= DOUBLE_TAP => {
            (held(true), HoldAction::Start)
        }
        HoldState::Held { since, .. } if now.saturating_duration_since(since) < REPEAT_FILTER => {
            (state, HoldAction::None)
        }
        // The release was lost, or the session is hands-free: this press
        // finishes the take.
        HoldState::Held { .. } | HoldState::Locked => (HoldState::Idle, HoldAction::Stop),
        HoldState::Idle | HoldState::LastTap { .. } => (held(false), HoldAction::Start),
    }
}

/// The hold chord came up at `now`.
pub fn on_release(state: HoldState, now: Instant) -> (HoldState, HoldAction) {
    match state {
        HoldState::Held {
            second_tap: true, ..
        } => (HoldState::Locked, HoldAction::Lock),
        HoldState::Held { since, .. } if now.saturating_duration_since(since) < MIN_HOLD => {
            (HoldState::LastTap { at: now }, HoldAction::DiscardTap)
        }
        HoldState::Held { .. } => (HoldState::Idle, HoldAction::Stop),
        other => (other, HoldAction::None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    enum Key {
        Down(u64),
        Up(u64),
    }

    /// Feeds key events at millisecond offsets and collects the actions.
    fn run(keys: &[Key]) -> (HoldState, Vec<HoldAction>) {
        let t0 = Instant::now();
        let mut state = HoldState::Idle;
        let mut actions = Vec::new();
        for key in keys {
            let (next, action) = match *key {
                Key::Down(at) => on_press(state, t0 + Duration::from_millis(at)),
                Key::Up(at) => on_release(state, t0 + Duration::from_millis(at)),
            };
            state = next;
            actions.push(action);
        }
        (state, actions)
    }

    use HoldAction::{DiscardTap, Lock, None as Nothing, Start, Stop};
    use Key::{Down, Up};

    #[test]
    fn the_hold_key_transitions() {
        let cases: &[(&str, &[Key], &str, &[HoldAction])] = &[
            (
                "hold and release",
                &[Down(0), Up(2000)],
                "idle",
                &[Start, Stop],
            ),
            (
                "a repeat inside 1 s is ignored",
                &[Down(0), Down(30), Down(999), Up(2000)],
                "idle",
                &[Start, Nothing, Nothing, Stop],
            ),
            (
                "a lost release: the next press after 1 s finishes",
                &[Down(0), Down(1500), Up(1600)],
                "idle",
                &[Start, Stop, Nothing],
            ),
            (
                "a tap drops its take",
                &[Down(0), Up(100)],
                "tapped",
                &[Start, DiscardTap],
            ),
            (
                "a press within 300 ms of a tap starts the take that locks",
                &[Down(0), Up(100), Down(350)],
                "held",
                &[Start, DiscardTap, Start],
            ),
            (
                "locked survives its release and the next press finishes",
                &[Down(0), Up(100), Down(350), Up(450), Down(5000), Up(5100)],
                "idle",
                &[Start, DiscardTap, Start, Lock, Stop, Nothing],
            ),
            (
                "a second press past 300 ms is a fresh hold, not a lock",
                &[Down(0), Up(100), Down(401), Up(2500)],
                "idle",
                &[Start, DiscardTap, Start, Stop],
            ),
            (
                "a release with nothing held does nothing",
                &[Up(0)],
                "idle",
                &[Nothing],
            ),
        ];
        for (name, keys, state, actions) in cases {
            let (end, got) = run(keys);
            assert_eq!(end.as_str(), *state, "{name}");
            assert_eq!(got.as_slice(), *actions, "{name}");
        }
    }

    #[test]
    fn locked_ignores_releases() {
        let now = Instant::now();
        assert_eq!(
            on_release(HoldState::Locked, now),
            (HoldState::Locked, HoldAction::None)
        );
        assert!(HoldState::Locked.records());
        assert!(!HoldState::LastTap { at: now }.records());
    }
}
