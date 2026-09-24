//! R5 against mock players on a private bus: a playing player is paused
//! and resumed, a paused one is left alone, and a player that vanished
//! before the resume is skipped and named.

use dettivo_hotkeys::mock::{MockPlayer, PrivateBus};
use dettivo_hotkeys::mpris::{PREFIX, pause_playing_blocking, resume_blocking};

#[test]
fn playing_players_are_paused_and_resumed_and_paused_ones_left_alone() {
    let bus = match PrivateBus::start() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("skip: {e}");
            return;
        }
    };
    let playing = MockPlayer::serve(bus.address(), "playing", "Playing").unwrap();
    let paused = MockPlayer::serve(bus.address(), "paused", "Paused").unwrap();
    let gone = MockPlayer::serve(bus.address(), "gone", "Playing").unwrap();
    let mut names = pause_playing_blocking(Some(bus.address())).unwrap();
    names.sort();
    assert_eq!(names, [format!("{PREFIX}gone"), format!("{PREFIX}playing")]);
    assert_eq!(playing.status(), "Paused");
    assert_eq!(paused.status(), "Paused");
    assert_eq!(gone.status(), "Paused");
    drop(gone);
    let skipped = resume_blocking(Some(bus.address()), &names).unwrap();
    assert_eq!(playing.status(), "Playing");
    assert_eq!(paused.status(), "Paused");
    assert_eq!(skipped.len(), 1, "{skipped:?}");
    assert_eq!(skipped[0].0, format!("{PREFIX}gone"));
}
