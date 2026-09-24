//! The archive hook: every finished transcript reaches the archive with
//! its raw text and take directory before the final state is published,
//! and an archive that fails puts its reason on that state while the
//! insertion stands.

mod support;

use std::path::Path;
use std::sync::{Arc, Mutex};

use dettivo_session::machine::Session;
use dettivo_session::{Archive, State, Transcript};
use support::{Recorder, policy, rig};

#[derive(Default)]
struct Shelf {
    seen: Mutex<Vec<(Transcript, bool)>>,
    fail: bool,
}

impl Archive for Shelf {
    fn archive(&self, transcript: &Transcript, take_dir: &Path) -> Result<(), String> {
        let take = take_dir.join("microphone.wav").is_file();
        self.seen.lock().unwrap().push((transcript.clone(), take));
        if self.fail {
            Err("disk full".into())
        } else {
            Ok(())
        }
    }
}

fn session_with(
    shelf: Arc<Shelf>,
) -> (
    tempfile::TempDir,
    Arc<Recorder>,
    Session,
    Arc<support::FakeEngine>,
    Arc<support::Feed>,
) {
    let (dir, rec, mut session, engine, feed) = rig("teh archive comma please period");
    session.set_archive(shelf);
    (dir, rec, session, engine, feed)
}

#[test]
fn the_archive_gets_the_transcript_and_the_take_before_idle_is_published() {
    let shelf = Arc::new(Shelf::default());
    let (dir, rec, session, engine, feed) = session_with(shelf.clone());
    session
        .start(policy(dir.path(), true), engine, feed.clone(), None)
        .unwrap();
    feed.speak(200);
    let t = session.stop().unwrap();
    let seen = shelf.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    let (archived, take_present) = &seen[0];
    assert_eq!(archived.text, "The archive, please.");
    assert_eq!(archived.raw_text, "teh archive comma please period");
    assert_eq!(archived.id, t.id);
    assert!(archived.insertion.is_some());
    assert!(
        *take_present,
        "the take is still there when the archive runs"
    );
    assert!(
        !dir.path()
            .join("sessions/job_dict_1/transcript.json")
            .exists()
    );
    assert!(
        !dir.path()
            .join("sessions/job_dict_1/microphone.wav")
            .exists()
    );
    let states = rec.states.lock().unwrap();
    let idle = states.iter().find(|c| c.state == State::Idle).unwrap();
    assert_eq!(idle.reason, None);
}

#[test]
fn a_failing_archive_is_reported_on_the_final_state_and_the_insertion_stands() {
    let shelf = Arc::new(Shelf {
        fail: true,
        ..Shelf::default()
    });
    let (dir, rec, session, engine, feed) = session_with(shelf.clone());
    session
        .start(policy(dir.path(), false), engine, feed.clone(), None)
        .unwrap();
    feed.speak(200);
    let t = session.stop().unwrap();
    assert_eq!(t.text, "The archive, please.");
    assert_eq!(
        rec.inserted.lock().unwrap().as_slice(),
        ["The archive, please."]
    );
    assert_eq!(session.last().unwrap().id, t.id);
    assert!(
        !dir.path()
            .join("sessions/job_dict_1/transcript.json")
            .exists()
    );
    assert!(
        !dir.path()
            .join("sessions/job_dict_1/microphone.wav")
            .exists()
    );
    let states = rec.states.lock().unwrap();
    let idle = states.iter().find(|c| c.state == State::Idle).unwrap();
    assert_eq!(idle.reason.as_deref(), Some("history: disk full"));
    assert_eq!(idle.previous, State::Inserting);
}

#[test]
fn cancellation_discards_staging_even_when_successful_audio_is_retained() {
    let (dir, _, session, engine, feed) = rig("cancelled");
    session
        .start(policy(dir.path(), true), engine, feed.clone(), None)
        .unwrap();
    feed.speak(200);
    session.cancel().unwrap();
    support::wait_idle(&session);
    assert!(
        !dir.path()
            .join("sessions/job_dict_1/microphone.wav")
            .exists()
    );
    assert!(
        !dir.path()
            .join("sessions/job_dict_1/transcript.json")
            .exists()
    );
}
