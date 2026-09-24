//! A capture cannot advance after the authoritative row write fails.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::{FakeEngine, Feeds, Log, policy};
use dettivo_meeting::machine::Session;
use dettivo_meeting::{Archive, State};
use dettivo_storage::meetings::{MeetingArtifacts, MeetingRow, MeetingStatus};

struct RefusingArchive {
    refuse: MeetingStatus,
    rows: Mutex<Vec<MeetingRow>>,
    completed: AtomicUsize,
    settled: AtomicUsize,
}

impl Archive for RefusingArchive {
    fn started(&self, row: &MeetingRow) -> Result<(), String> {
        self.updated(row)
    }
    fn updated(&self, row: &MeetingRow) -> Result<(), String> {
        if row.status == self.refuse {
            return Err("disk full".into());
        }
        self.rows.lock().unwrap().push(row.clone());
        Ok(())
    }
    fn completed(&self, _: &mut MeetingRow) {
        self.completed.fetch_add(1, Ordering::SeqCst);
    }
    fn settled(&self, _: &MeetingRow, _: bool, _: dettivo_storage::retention::ArtifactPolicy) {
        self.settled.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn failed_capture_or_transcribing_write_does_not_finalize_or_announce_success() {
    for refuse in [MeetingStatus::Stopped, MeetingStatus::Transcribing] {
        let dir = tempfile::tempdir().unwrap();
        let log = Arc::new(Log::default());
        let archive = Arc::new(RefusingArchive {
            refuse,
            rows: Mutex::default(),
            completed: AtomicUsize::new(0),
            settled: AtomicUsize::new(0),
        });
        let session = Session::new(
            MeetingArtifacts::new(dir.path().join("meetings")),
            log.clone(),
            archive.clone(),
        );
        let mut p = policy();
        p.live = false;
        let id = session
            .start(
                p,
                Arc::new(Feeds::new(false, true)),
                FakeEngine::new(Duration::ZERO),
                MeetingRow::new(),
            )
            .unwrap()
            .meeting_id;
        session.stop().unwrap();
        assert!(session.wait_idle(Duration::from_secs(5)));
        assert_eq!(archive.completed.load(Ordering::SeqCst), 0, "{refuse:?}");
        assert_eq!(archive.settled.load(Ordering::SeqCst), 0);
        assert_eq!(
            archive.rows.lock().unwrap().last().unwrap().status,
            MeetingStatus::Failed
        );
        assert!(
            session
                .artifacts()
                .dir(&id)
                .join("microphone.wav")
                .is_file()
        );
        let states = log.states.lock().unwrap();
        assert_eq!(states.last().unwrap().state, State::Failed);
        assert!(!states.iter().any(|s| s.state == State::Completed));
    }
}

#[test]
fn completion_reads_temporary_audio_before_every_retention_policy() {
    use dettivo_storage::retention::ArtifactPolicy;
    struct ReadingArchive {
        audio_seen: AtomicUsize,
    }
    impl Archive for ReadingArchive {
        fn started(&self, _: &MeetingRow) -> Result<(), String> {
            Ok(())
        }
        fn updated(&self, _: &MeetingRow) -> Result<(), String> {
            Ok(())
        }
        fn settled(&self, row: &MeetingRow, _: bool, _: ArtifactPolicy) {
            if row
                .audio_dir
                .as_ref()
                .is_some_and(|dir| std::path::Path::new(dir).join("microphone.wav").is_file())
            {
                self.audio_seen.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
    for artifacts in [
        ArtifactPolicy::Keep,
        ArtifactPolicy::AudioOnly,
        ArtifactPolicy::None,
    ] {
        for (keep_audio, samples) in [(false, 0), (false, 1600), (true, 1600)] {
            let dir = tempfile::tempdir().unwrap();
            let archive = Arc::new(ReadingArchive {
                audio_seen: AtomicUsize::new(0),
            });
            let log = Arc::new(Log::default());
            let session = Session::new(
                MeetingArtifacts::new(dir.path().join("meetings")),
                log.clone(),
                archive.clone(),
            );
            let mut p = policy();
            p.live = false;
            p.artifacts = artifacts;
            p.keep_audio = keep_audio;
            let feeds = Arc::new(Feeds::new(false, true));
            let id = session
                .start(
                    p,
                    feeds.clone(),
                    FakeEngine::new(Duration::ZERO),
                    MeetingRow::new(),
                )
                .unwrap()
                .meeting_id;
            feeds
                .mic(0)
                .send(dettivo_audio::Event::Pcm(vec![1; samples]))
                .unwrap();
            feeds
                .mic(0)
                .send(dettivo_audio::Event::Level {
                    rms: 0.1,
                    peak: 0.2,
                })
                .unwrap();
            while log.levels.lock().unwrap().is_empty() {
                std::thread::sleep(Duration::from_millis(5));
            }
            session.stop().unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while archive.audio_seen.load(Ordering::SeqCst) == 0
                || session
                    .artifacts()
                    .dir(&id)
                    .join("microphone.wav")
                    .is_file()
                    != (keep_audio && artifacts != ArtifactPolicy::None)
            {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            assert_eq!(
                archive.audio_seen.load(Ordering::SeqCst),
                1,
                "{artifacts:?}, keep_audio={keep_audio}"
            );
            assert_eq!(
                session
                    .artifacts()
                    .dir(&id)
                    .join("microphone.wav")
                    .is_file(),
                keep_audio && artifacts != ArtifactPolicy::None
            );
        }
    }
}
