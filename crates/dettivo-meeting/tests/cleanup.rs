//! Regression coverage for lost microphones, damaged captures and recovery timing.

mod common;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use common::{FakeEngine, Feeds, policy, setup};
use dettivo_audio::takes::{TakeWriter, read_sidecar};
use dettivo_audio::{EndReason, Event};
use dettivo_meeting::checkpoint::{Checkpoint, CheckpointTake};
use dettivo_meeting::source::{Source, Sources};
use dettivo_meeting::{State, Track, recovery};
use dettivo_storage::meetings::{MeetingRow, MeetingStatus};
use dettivo_storage::retention::ArtifactPolicy;

fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while !condition() {
        assert!(Instant::now() < deadline, "condition did not converge");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[derive(Default)]
struct ReturningMic {
    available: AtomicBool,
    attempts: AtomicUsize,
    channels: Mutex<Vec<mpsc::Sender<Event>>>,
}

impl Sources for ReturningMic {
    fn open_microphone(&self, _: u64, reopen: bool) -> Result<Source, String> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        if reopen && !self.available.load(Ordering::SeqCst) {
            return Err("no microphone".into());
        }
        let (tx, events) = mpsc::channel();
        self.channels.lock().unwrap().push(tx);
        Ok(Source::Channel {
            events,
            on_stop: Box::new(|| {}),
        })
    }
    fn open_system(&self, _: u64) -> Result<Source, String> {
        Err("no monitor".into())
    }
}

#[test]
fn microphone_returns_after_reopen_failed_without_empty_takes() {
    let (session, log, rows, _, _dir) = setup(Feeds::new(false, true));
    let feeds = Arc::new(ReturningMic::default());
    let mut p = policy();
    p.live = false;
    let id = session
        .start(
            p,
            feeds.clone(),
            FakeEngine::new(Duration::ZERO),
            MeetingRow::new(),
        )
        .unwrap()
        .meeting_id;
    let first = feeds.channels.lock().unwrap()[0].clone();
    first.send(Event::Pcm(vec![1; 1600])).unwrap();
    first
        .send(Event::Ended {
            reason: EndReason::NoSource,
        })
        .unwrap();
    until(|| feeds.attempts.load(Ordering::SeqCst) >= 2);
    assert_eq!(
        read_sidecar(&session.artifacts().dir(&id).join("takes.json"))
            .unwrap()
            .takes
            .len(),
        1
    );
    feeds.available.store(true, Ordering::SeqCst);
    until(|| feeds.channels.lock().unwrap().len() == 2);
    let second = feeds.channels.lock().unwrap()[1].clone();
    second.send(Event::Pcm(vec![2; 800])).unwrap();
    second
        .send(Event::Level {
            rms: 0.2,
            peak: 0.3,
        })
        .unwrap();
    until(|| !log.levels.lock().unwrap().is_empty());
    session.stop().unwrap();
    assert!(session.wait_idle(Duration::from_secs(5)));
    let takes = read_sidecar(&session.artifacts().dir(&id).join("takes.json")).unwrap();
    assert_eq!(takes.takes.len(), 2);
    assert_eq!(takes.takes[1].samples, 800);
    assert!(takes.takes[1].gap_before);
    assert!(takes.takes[1].start_offset_ms >= 400);
    assert_eq!(feeds.attempts.load(Ordering::SeqCst), 3);
    assert_eq!(rows.last().status, MeetingStatus::Completed);
}

#[test]
fn damaged_capture_inputs_fail_without_cleanup() {
    for damage in ["manifest", "take", "missing_manifest", "system_manifest"] {
        let (session, log, rows, _, _dir) = setup(Feeds::new(false, true));
        let mut row = MeetingRow::new();
        row.status = MeetingStatus::Partial;
        row.microphone_takes = 1;
        row.system_audio = damage == "system_manifest";
        let dir = session.artifacts().dir(&row.id);
        let mut writer = TakeWriter::with_prefix(&dir, "microphone", Instant::now()).unwrap();
        writer.write(&[1; 1600]).unwrap();
        writer.finish().unwrap();
        row.audio_dir = Some(dir.to_string_lossy().into_owned());
        match damage {
            "manifest" => std::fs::write(dir.join("takes.json"), "{").unwrap(),
            "missing_manifest" => std::fs::remove_file(dir.join("takes.json")).unwrap(),
            "take" => std::fs::remove_file(dir.join("microphone.wav")).unwrap(),
            _ => {}
        }
        let mut p = policy();
        p.artifacts = ArtifactPolicy::None;
        session
            .finalize_recovered(p, FakeEngine::new(Duration::ZERO), row)
            .unwrap();
        until(|| {
            log.states
                .lock()
                .unwrap()
                .iter()
                .any(|s| matches!(s.state, State::Completed | State::Failed))
        });
        assert_eq!(rows.last().status, MeetingStatus::Failed, "{damage}");
        assert!(dir.is_dir(), "{damage}: retained damaged inputs");
        assert!(
            !log.states
                .lock()
                .unwrap()
                .iter()
                .any(|s| s.state == State::Completed)
        );
    }
}

#[test]
fn failed_take_closure_ends_capture_failed_and_keeps_audio() {
    let (session, log, rows, feeds, _dir) = setup(Feeds::new(false, true));
    let mut p = policy();
    p.live = false;
    let id = session
        .start(
            p,
            feeds.clone(),
            FakeEngine::new(Duration::ZERO),
            MeetingRow::new(),
        )
        .unwrap()
        .meeting_id;
    feeds.mic(0).send(Event::Pcm(vec![1; 1600])).unwrap();
    feeds
        .mic(0)
        .send(Event::Level {
            rms: 0.2,
            peak: 0.3,
        })
        .unwrap();
    until(|| !log.levels.lock().unwrap().is_empty());
    let dir = session.artifacts().dir(&id);
    std::fs::remove_file(dir.join("takes.json")).unwrap();
    std::fs::create_dir(dir.join("takes.json")).unwrap();
    session.stop().unwrap();
    assert!(session.wait_idle(Duration::from_secs(5)));
    assert_eq!(rows.last().status, MeetingStatus::Failed);
    assert_eq!(rows.last().error_code.as_deref(), Some("capture"));
    assert!(dir.join("microphone.wav").is_file());
    assert!(
        !log.states
            .lock()
            .unwrap()
            .iter()
            .any(|s| s.state == State::Transcribing)
    );
}

#[test]
fn checkpoint_restores_three_take_offsets_and_complete_diarization_track() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Instant::now();
    let mut writer = TakeWriter::with_prefix(dir.path(), "microphone", clock).unwrap();
    for (i, offset) in [0, 3000, 7000].into_iter().enumerate() {
        writer
            .start_take_at(clock + Duration::from_millis(offset), i > 0)
            .unwrap();
        writer.write(&vec![(i + 1) as i16; 16000]).unwrap();
        writer.finish_take().unwrap();
    }
    let takes = writer.finish().unwrap();
    let mut checkpoint = Checkpoint::new("m", "2026-01-01T00:00:00Z");
    checkpoint.takes = takes
        .takes
        .iter()
        .map(|t| CheckpointTake::from_take(Track::Microphone, t))
        .collect();
    checkpoint.write(dir.path()).unwrap();
    std::fs::write(dir.path().join("takes.json"), "{").unwrap();
    let mut row = MeetingRow::new();
    let promoted = recovery::promote(dir.path(), &mut row);
    assert_eq!(promoted.duration_ms, 8000);
    assert_eq!(read_sidecar(&dir.path().join("takes.json")).unwrap(), takes);
    let pcm = dettivo_meeting::diarize::read_track(dir.path(), Track::Microphone).unwrap();
    assert_eq!(pcm.len(), 128000);
    assert_eq!(&pcm[..16000], vec![1; 16000]);
    assert_eq!(&pcm[48000..64000], vec![2; 16000]);
    assert_eq!(&pcm[112000..], vec![3; 16000]);
    assert!(pcm[16000..48000].iter().all(|s| *s == 0));
}

#[test]
fn journal_recovers_later_take_timing_but_unknown_orphans_remain_explicit() {
    for known in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let clock = Instant::now();
        let mut writer = TakeWriter::with_prefix(dir.path(), "microphone", clock).unwrap();
        writer.start_take_at(clock, false).unwrap();
        writer.write(&[1; 1600]).unwrap();
        writer.finish_take().unwrap();
        let first_sidecar = std::fs::read(dir.path().join("takes.json")).unwrap();
        writer
            .start_take_at(clock + Duration::from_secs(2), true)
            .unwrap();
        writer.write(&[2; 1600]).unwrap();
        writer.finish().unwrap();
        std::fs::write(dir.path().join("takes.json"), first_sidecar).unwrap();
        if known {
            dettivo_meeting::journal::append(
                &dir.path().join(dettivo_meeting::JOURNAL_FILE),
                &dettivo_meeting::journal::Entry {
                    at: "2026-01-01T00:00:02Z".into(),
                    offset_ms: 2000,
                    event: "gap".into(),
                    track: Some("microphone".into()),
                    take: Some("microphone-2.wav".into()),
                    detail: None,
                },
            )
            .unwrap();
        }
        let promoted = recovery::promote(dir.path(), &mut MeetingRow::new());
        if known {
            assert_eq!(promoted.duration_ms, 2100);
            assert_eq!(
                read_sidecar(&dir.path().join("takes.json")).unwrap().takes[1].start_offset_ms,
                2000
            );
        } else {
            assert!(
                promoted
                    .reason
                    .unwrap()
                    .contains("unknown take timing: microphone-2.wav")
            );
            assert!(
                dettivo_meeting::finalize::takes_in(dir.path()).is_err(),
                "orphan audio cannot be silently omitted"
            );
            assert!(dir.path().join("microphone-2.wav").is_file());
        }
    }
}
