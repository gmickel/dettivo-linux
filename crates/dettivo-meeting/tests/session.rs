//! The meeting machine over channel sources: two tracks land as takes on
//! one clock, a microphone that vanishes restarts as `microphone-2.wav`
//! with a gap marker while the system track carries on, a device loss
//! that finds no device keeps the system track, stop reports stopping,
//! stopped, transcribing and completed and is idempotent, cancel removes
//! the directory, and a take that cannot be written ends the meeting
//! failed with its takes.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{FakeEngine, Feeds, Log, Rows, policy, setup, states, tone};
use dettivo_audio::{EndReason, Event};
use dettivo_meeting::machine::Session;
use dettivo_meeting::{JOURNAL_FILE, METADATA_FILE, State, Track, checkpoint::Checkpoint, journal};
use dettivo_storage::meetings::{MeetingArtifacts, MeetingRow, MeetingStatus};
use dettivo_transcribe::live::Source;

#[test]
fn two_tracks_switch_stop_and_the_journal_says_what_happened() {
    let (session, log, rows, feeds, _dir) = setup(Feeds::new(false, false));
    let engine = FakeEngine::new(Duration::ZERO);
    let row = MeetingRow::new();
    let id = row.id.clone();
    let snapshot = session
        .start(policy(), feeds.clone(), engine.clone(), row)
        .unwrap();
    assert_eq!(snapshot.state, State::Recording);
    assert_eq!(snapshot.job_id, "job_meeting_1");
    assert!(
        session
            .start(policy(), feeds.clone(), engine.clone(), MeetingRow::new())
            .is_err()
    );
    let dir = session.artifacts().dir(&id);

    feeds.mic(0).send(Event::Pcm(vec![1; 1600])).unwrap();
    feeds
        .mic(0)
        .send(Event::Level {
            rms: 0.2,
            peak: 0.5,
        })
        .unwrap();
    feeds.system().send(Event::Pcm(vec![2; 3200])).unwrap();
    feeds
        .system()
        .send(Event::Level {
            rms: 0.1,
            peak: 0.3,
        })
        .unwrap();
    std::thread::sleep(Duration::from_millis(400));
    assert!(
        Checkpoint::read(&dir).is_ok(),
        "a checkpoint was written on the interval"
    );

    // The microphone vanishes: a new take on the reopened source with a
    // gap marker; the system track carries on.
    feeds
        .mic(0)
        .send(Event::Ended {
            reason: EndReason::DeviceLost("usb-headset".into()),
        })
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));
    feeds.mic(1).send(Event::Pcm(vec![3; 800])).unwrap();
    feeds.system().send(Event::Pcm(vec![2; 1600])).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let live = session.snapshot().unwrap();
    assert_eq!(live.microphone_takes, 2);
    assert!(live.system_audio);

    let stopping = session.stop().unwrap();
    assert_eq!(stopping.state, State::Stopping);
    assert_eq!(session.stop().unwrap().state, State::Stopping, "idempotent");
    assert!(session.wait_idle(Duration::from_secs(5)));
    assert!(session.stop().is_err(), "nothing active");

    let final_row = rows.last();
    assert_eq!(final_row.status, MeetingStatus::Completed);
    assert_eq!(final_row.microphone_takes, 2);
    assert!(final_row.system_audio);
    assert!(
        final_row.duration_ms >= 300,
        "the system track is the longer one: {}",
        final_row.duration_ms
    );
    assert!(final_row.ended_at.is_some());

    let mic_takes = dettivo_audio::takes::read_sidecar(&dir.join("takes.json")).unwrap();
    assert_eq!(mic_takes.takes.len(), 2);
    assert_eq!(mic_takes.takes[1].file, "microphone-2.wav");
    assert!(mic_takes.takes[1].gap_before);
    assert!(mic_takes.takes[1].start_offset_ms >= 400);
    assert_eq!(mic_takes.takes[1].samples, 800);
    let sys_takes = dettivo_audio::takes::read_sidecar(&dir.join("system-takes.json")).unwrap();
    assert_eq!(sys_takes.takes[0].file, "system.wav");
    assert_eq!(sys_takes.takes[0].samples, 4800);
    assert!(
        mic_takes.takes[0]
            .start_offset_ms
            .abs_diff(sys_takes.takes[0].start_offset_ms)
            < 50
    );
    assert!(dir.join("microphone.wav").is_file() && dir.join("system.wav").is_file());
    assert!(
        !Checkpoint::path(&dir).exists(),
        "a settled meeting has no checkpoint"
    );

    let entries = journal::read(&dir.join(JOURNAL_FILE));
    let events: Vec<&str> = entries.iter().map(|e| e.event.as_str()).collect();
    assert_eq!(events[0], "start");
    assert!(events.contains(&"checkpoint"));
    assert!(events.contains(&"device_lost"));
    let gap = entries.iter().find(|e| e.event == "gap").unwrap();
    assert_eq!(gap.take.as_deref(), Some("microphone-2.wav"));
    assert_eq!(gap.track.as_deref(), Some("microphone"));
    assert!(events.contains(&"stop"));
    assert_eq!(events.last(), Some(&"finalized"));

    let metadata: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(METADATA_FILE)).unwrap()).unwrap();
    assert_eq!(metadata["meeting_id"], id);
    assert!(
        metadata.get("meeting").is_none(),
        "no transcript-bearing row copy"
    );
    assert_eq!(metadata["takes"][0]["takes"].as_array().unwrap().len(), 2);
    assert_eq!(metadata["checkpoint_schema"], 1);

    let seen = states(&log);
    assert_eq!(seen[0].0, State::Recording);
    assert!(seen.iter().any(|(s, _, r)| *s == State::Recording
        && r.as_deref().is_some_and(|r| r.contains("microphone-2.wav"))));
    let tail: Vec<State> = seen.iter().rev().take(4).map(|(s, _, _)| *s).collect();
    assert_eq!(
        tail,
        [
            State::Completed,
            State::Transcribing,
            State::Stopped,
            State::Stopping
        ]
    );
    let levels = log.levels.lock().unwrap();
    assert!(levels.iter().any(|l| l.track == Track::Microphone));
    assert!(levels.iter().any(|l| l.track == Track::System));
}

#[test]
fn a_loss_with_no_device_keeps_the_system_track_and_no_monitor_is_room_audio() {
    let (session, _log, rows, feeds, _dir) = setup(Feeds::new(true, false));
    let engine = FakeEngine::new(Duration::ZERO);
    let id = MeetingRow::new();
    session
        .start(policy(), feeds.clone(), engine.clone(), id)
        .unwrap();
    feeds.mic(0).send(Event::Pcm(vec![1; 160])).unwrap();
    feeds
        .mic(0)
        .send(Event::Ended {
            reason: EndReason::NoSource,
        })
        .unwrap();
    feeds.system().send(Event::Pcm(vec![2; 1600])).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    session.stop().unwrap();
    assert!(session.wait_idle(Duration::from_secs(5)));
    let row = rows.last();
    assert_eq!(row.status, MeetingStatus::Completed);
    assert_eq!(row.microphone_takes, 1);
    assert!(row.duration_ms >= 100);
    let dir = session.artifacts().dir(&row.id);
    let entries = journal::read(&dir.join(JOURNAL_FILE));
    let gap = entries.iter().find(|e| e.event == "gap").unwrap();
    assert!(gap.detail.as_deref().unwrap().contains("no device"));
    assert!(gap.take.is_none());

    let (session, _log, rows, feeds, _dir) = setup(Feeds::new(false, true));
    let snapshot = session
        .start(policy(), feeds.clone(), engine, MeetingRow::new())
        .unwrap();
    assert!(!snapshot.system_audio);
    feeds.mic(0).send(Event::Pcm(vec![1; 160])).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    session.stop().unwrap();
    assert!(session.wait_idle(Duration::from_secs(5)));
    let row = rows.last();
    assert!(!row.system_audio);
    let dir = session.artifacts().dir(&row.id);
    let entries = journal::read(&dir.join(JOURNAL_FILE));
    assert!(entries.iter().any(|e| e.event == "system_unavailable"));
    assert!(!dir.join("system.wav").exists());
}

#[test]
fn cancel_removes_the_directory_and_a_write_failure_ends_failed_with_the_takes() {
    let (session, log, rows, feeds, _dir) = setup(Feeds::new(false, false));
    let engine = FakeEngine::new(Duration::ZERO);
    let snapshot = session
        .start(policy(), feeds.clone(), engine.clone(), MeetingRow::new())
        .unwrap();
    let dir = session.artifacts().dir(&snapshot.meeting_id);
    feeds.mic(0).send(Event::Pcm(vec![1; 160])).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    assert!(dir.join("microphone.wav").is_file());
    let cancelled = session.cancel().unwrap();
    assert_eq!(cancelled.state, State::Cancelled);
    assert!(session.snapshot().is_none());
    assert!(!dir.exists(), "cancel removes the takes");
    let row = rows.last();
    assert_eq!(row.status, MeetingStatus::Cancelled);
    assert!(row.audio_dir.is_none());
    assert_eq!(states(&log).last().unwrap().0, State::Cancelled);
    assert!(
        log.segments.lock().unwrap().is_empty()
            || session.finalizing(&snapshot.meeting_id).is_none(),
        "a cancelled meeting is never finalised"
    );

    // A take that cannot be written (here: the meeting directory cannot
    // be created because a file sits where the meetings root should be)
    // ends the meeting failed with the reason on the row and the state.
    let blocked = tempfile::tempdir().unwrap();
    std::fs::write(blocked.path().join("meetings"), b"not a directory").unwrap();
    let log = Arc::new(Log::default());
    let rows = Arc::new(Rows::default());
    let session = Session::new(
        MeetingArtifacts::new(blocked.path().join("meetings")),
        log.clone(),
        rows.clone(),
    );
    let feeds = Arc::new(Feeds::new(false, false));
    session
        .start(policy(), feeds.clone(), engine, MeetingRow::new())
        .unwrap();
    assert!(
        session.wait_idle(Duration::from_secs(5)),
        "the failure ends the meeting"
    );
    let row = rows.last();
    assert_eq!(row.status, MeetingStatus::Failed);
    assert_eq!(row.error_code.as_deref(), Some("capture"));
    assert!(
        row.error_message
            .as_deref()
            .unwrap()
            .contains("meeting directory")
    );
    let last = states(&log).last().unwrap().clone();
    assert_eq!(last.0, State::Failed);
    assert!(last.2.as_deref().unwrap().contains("meeting directory"));
}

/// The take manifests and the live windows count from the instant each
/// source opened, so a system track that opens later than the
/// microphone lands later on the live clock and on the final one alike,
/// by the same amount.
#[test]
fn a_source_that_opens_later_keeps_its_origin_live_and_in_the_takes() {
    let delay = Duration::from_millis(300);
    let (session, log, _rows, feeds, _dir) =
        setup(Feeds::new(false, false).with_system_delay(delay));
    let engine = FakeEngine::new(Duration::ZERO);
    let row = MeetingRow::new();
    let id = row.id.clone();
    session.start(policy(), feeds.clone(), engine, row).unwrap();
    let dir = session.artifacts().dir(&id);
    // Four seconds of speech on both tracks: enough for a live window.
    feeds.mic(0).send(Event::Pcm(tone(4000, 8000))).unwrap();
    feeds.system().send(Event::Pcm(tone(4000, 8000))).unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    session.stop().unwrap();
    assert!(session.wait_idle(Duration::from_secs(10)));

    let mic_takes = dettivo_audio::takes::read_sidecar(&dir.join("takes.json")).unwrap();
    let sys_takes = dettivo_audio::takes::read_sidecar(&dir.join("system-takes.json")).unwrap();
    let mic_origin = mic_takes.takes[0].start_offset_ms;
    let sys_origin = sys_takes.takes[0].start_offset_ms;
    assert!(mic_origin < 150, "microphone origin {mic_origin} ms");
    assert!(
        sys_origin >= mic_origin + 300,
        "system origin {sys_origin} ms against microphone {mic_origin} ms"
    );

    let segments = log.segments.lock().unwrap();
    let first = |source: Source| {
        segments
            .iter()
            .filter(|(_, s)| s.source == source)
            .map(|(_, s)| s.start_ms)
            .min()
            .unwrap_or_else(|| panic!("no live segment for {source:?} in {segments:?}"))
    };
    let (you, remote) = (first(Source::You), first(Source::Remote));
    assert!(
        remote >= you + 300,
        "the remote lane starts at {remote} ms, the you lane at {you} ms; the live clock must carry the {sys_origin} ms origin"
    );
    assert!(
        remote >= sys_origin && you >= mic_origin,
        "live segments start at or after their take's origin: you {you} >= {mic_origin}, remote {remote} >= {sys_origin}"
    );
}
