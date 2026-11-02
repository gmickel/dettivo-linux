//! What a session does to the rest of the desktop (FR-H5, FR-H6): hands
//! the take's start and end to the media service (`[audio]
//! while_dictating`, ADR 0091), and plays the start, stop and error cues.
//! Both are off by default, read at each transition, and silent during a
//! meeting.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use dettivo_audio::playback::{Cue, Player};
use dettivo_core::config::Loaded;
use dettivo_core::config::schema::WhileDictating;
use dettivo_session::{State, StateChange};

use crate::media::{self, Media};

/// The service.
pub struct Feedback {
    config: Arc<RwLock<Loaded>>,
    player: Player,
    media: Media,
}

/// True when a cue may play: sounds are on and no meeting runs.
pub fn sound_allowed(sounds: bool, meeting_active: bool) -> bool {
    sounds && !meeting_active
}

/// What a transition asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Capture began: media, start cue.
    Started,
    /// Capture stopped and transcription follows: a tap leaves media.
    Stopped,
    /// The session ended well or was cancelled: media back, stop cue.
    Ended,
    /// The session failed: media back, error cue.
    Failed,
}

/// The effect of a state change, if any. `Idle` after a failure or a
/// cancellation is the second report of one exit and carries nothing.
pub fn effect_of(change: &StateChange) -> Option<Effect> {
    match (change.state, change.previous) {
        (State::Recording, State::Idle) => Some(Effect::Started),
        (State::Transcribing | State::Inserting, State::Recording) => Some(Effect::Stopped),
        (State::Cancelled, _) => Some(Effect::Ended),
        (State::Failed, _) => Some(Effect::Failed),
        (State::Idle, State::Cancelled | State::Failed | State::Idle) => None,
        (State::Idle, _) => Some(Effect::Ended),
        _ => None,
    }
}

impl Feedback {
    /// Cue files go under `cache_dir`; `mock_log` names the QA file that
    /// records cue names instead of playing them.
    pub fn new(config: Arc<RwLock<Loaded>>, cache_dir: PathBuf, mock_log: Option<PathBuf>) -> Self {
        Self {
            config,
            player: Player::new(cache_dir, mock_log),
            media: Media::new(Box::new(media::Wpctl), Box::new(media::Mpris)),
        }
    }

    /// Whether a meeting is being captured. No meeting subsystem exists
    /// yet (S-23 brings it and its flag), so nothing is ever in a meeting.
    fn meeting_active(&self) -> bool {
        false
    }

    fn settings(&self) -> (WhileDictating, bool) {
        let loaded = self.config.read().unwrap_or_else(|p| p.into_inner());
        let c = &loaded.config;
        (
            media::effective(c.audio.while_dictating, c.hotkeys.pause_media),
            c.hotkeys.sounds,
        )
    }

    /// Applies a session transition.
    pub fn on_state(&self, change: &StateChange) {
        let Some(effect) = effect_of(change) else {
            return;
        };
        let (while_dictating, sounds) = self.settings();
        match effect {
            Effect::Started => {
                self.media.started(while_dictating);
                self.cue(sounds, Cue::Start);
            }
            Effect::Stopped => self.media.stopped(),
            Effect::Ended | Effect::Failed => {
                self.media.ended();
                self.cue(
                    sounds,
                    if effect == Effect::Failed {
                        Cue::Error
                    } else {
                        Cue::Stop
                    },
                );
            }
        }
    }

    fn cue(&self, sounds: bool, cue: Cue) {
        if !sound_allowed(sounds, self.meeting_active()) {
            return;
        }
        if let Err(e) = self.player.play(cue) {
            tracing::warn!(cue = cue.name(), error = %e, "feedback sound not played");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(state: State, previous: State) -> StateChange {
        StateChange {
            job_id: "job".into(),
            state,
            previous,
            reason: None,
            insertion: None,
            first_words: None,
            timings: None,
            mode: None,
            notice: None,
            policy_hash: None,
        }
    }

    #[test]
    fn transitions_map_to_one_effect_each() {
        assert_eq!(
            effect_of(&change(State::Recording, State::Idle)),
            Some(Effect::Started)
        );
        assert_eq!(
            effect_of(&change(State::Transcribing, State::Recording)),
            Some(Effect::Stopped)
        );
        assert_eq!(
            effect_of(&change(State::Inserting, State::Transcribing)),
            None
        );
        assert_eq!(
            effect_of(&change(State::Idle, State::Inserting)),
            Some(Effect::Ended)
        );
        assert_eq!(
            effect_of(&change(State::Cancelled, State::Recording)),
            Some(Effect::Ended)
        );
        assert_eq!(effect_of(&change(State::Idle, State::Cancelled)), None);
        assert_eq!(
            effect_of(&change(State::Failed, State::Transcribing)),
            Some(Effect::Failed)
        );
        assert_eq!(effect_of(&change(State::Idle, State::Failed)), None);
    }

    #[test]
    fn sounds_are_off_by_default_and_silent_in_meetings() {
        assert!(!sound_allowed(false, false));
        assert!(sound_allowed(true, false));
        assert!(!sound_allowed(true, true));
    }
}
