//! The transcript so far (ADR 0071) over channel sources and the fake
//! engine: a poller that follows the cursor from the first second of a
//! recording through the stop, the finalisation and the completed row
//! reads every live final exactly once and in order, then exactly one
//! reset to the stored transcript; and reads at a high rate during a
//! recording change neither the takes nor the checkpoint.

mod common;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use common::{FakeEngine, Feeds, Rows, policy, setup, tone};
use dettivo_audio::Event;
use dettivo_meeting::machine::Session;
use dettivo_proto::methods::meetings_segments::{Cursor, TranscriptSegment, TranscriptSource};
use dettivo_storage::meetings::{MeetingRow, MeetingStatus};

/// What a poller collected.
#[derive(Default)]
struct Poller {
    cursor: Option<String>,
    live: Vec<TranscriptSegment>,
    stored: Vec<TranscriptSegment>,
    resets: u32,
    seen: Vec<(String, TranscriptSource)>,
}

/// The status a client would read beside the transcript.
fn status(session: &Session, rows: &Rows, id: &str) -> String {
    if let Some(s) = session.snapshot().filter(|s| s.meeting_id == id) {
        return s.state.as_str().to_string();
    }
    if session.finalizing(id).is_some() {
        return "transcribing".into();
    }
    format!("{:?}", rows.last().status).to_lowercase()
}

impl Poller {
    /// One poll with the cursor; true once the stored transcript of the
    /// completed meeting answered.
    fn poll(&mut self, session: &Session, rows: &Rows, id: &str) -> bool {
        let since = self.cursor.as_deref().map(|c| Cursor::parse(c).unwrap());
        let status = status(session, rows, id);
        let read = session
            .read_segments(id, since, || Ok::<_, ()>(rows.last().segments))
            .unwrap();
        assert!(read.provisional.iter().all(|p| p.provisional));
        assert!(read.segments.iter().all(|s| !s.provisional));
        let into = match read.transcript {
            TranscriptSource::Live => {
                assert!(!read.reset, "the live cursor never resets");
                assert!(self.stored.is_empty(), "live after stored: {:?}", self.seen);
                &mut self.live
            }
            TranscriptSource::Stored => {
                if read.reset {
                    self.resets += 1;
                    self.stored.clear();
                }
                &mut self.stored
            }
        };
        for s in read.segments {
            assert_eq!(s.index as usize, into.len(), "no final skipped or repeated");
            into.push(s);
        }
        self.cursor = Some(read.cursor);
        let done = status == "completed" && read.transcript == TranscriptSource::Stored;
        self.seen.push((status, read.transcript));
        done
    }
}

fn speech(feeds: &Feeds, ms: u64) {
    for _ in 0..ms / 100 {
        feeds.mic(0).send(Event::Pcm(tone(100, 3000))).unwrap();
        feeds.system().send(Event::Pcm(tone(100, 3000))).unwrap();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn silence(feeds: &Feeds, ms: u64) {
    for _ in 0..ms / 100 {
        feeds.mic(0).send(Event::Pcm(vec![0; 1600])).unwrap();
        feeds.system().send(Event::Pcm(vec![0; 1600])).unwrap();
    }
}

fn key(s: &TranscriptSegment) -> (u64, u64, String, String) {
    (s.start_ms, s.end_ms, s.text.clone(), s.source.clone())
}

/// R2, R10: recording, stopping, stopped, transcribing, completed, one
/// poller on the cursor throughout.
#[test]
fn the_cursor_reads_every_final_once_from_the_recording_to_the_completed_row() {
    let (session, _log, rows, feeds, _dir) = setup(Feeds::new(false, false));
    // A slow engine keeps the stop's flush and the finalisation long
    // enough to be polled.
    let engine = FakeEngine::new(Duration::from_millis(20));
    let id = session
        .start(policy(), feeds.clone(), engine, MeetingRow::new())
        .unwrap()
        .meeting_id;
    let stopped = AtomicBool::new(false);
    let mut poller = Poller::default();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            for _ in 0..4 {
                speech(&feeds, 1900);
                silence(&feeds, 2000);
            }
            speech(&feeds, 1500);
            session.stop().unwrap();
            stopped.store(true, Ordering::SeqCst);
        });
        let deadline = Instant::now() + Duration::from_secs(60);
        while !poller.poll(&session, &rows, &id) {
            assert!(Instant::now() < deadline, "{:?}", poller.seen);
            std::thread::sleep(Duration::from_millis(2));
        }
    });
    assert!(stopped.load(Ordering::SeqCst));
    assert!(session.wait_idle(Duration::from_secs(10)));
    let statuses: HashSet<&str> = poller.seen.iter().map(|(s, _)| s.as_str()).collect();
    for wanted in ["recording", "stopping", "transcribing", "completed"] {
        assert!(statuses.contains(wanted), "{wanted}: {:?}", poller.seen);
    }
    assert!(
        poller
            .seen
            .iter()
            .any(|(s, t)| s == "transcribing" && *t == TranscriptSource::Live),
        "the live transcript answers while the finalisation runs"
    );
    assert_eq!(poller.resets, 1, "one switch to the stored transcript");
    // Every final of the live transcript, the ones the stop hardened
    // included, arrived once: the stand-in row the stop stored holds
    // exactly them.
    let ids: HashSet<&str> = poller.live.iter().map(|s| s.segment_id.as_str()).collect();
    assert_eq!(ids.len(), poller.live.len());
    assert!(poller.live.iter().any(|s| s.source == "you"));
    assert!(poller.live.iter().any(|s| s.source == "remote"));
    let stand_in = rows
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|r| r.status == MeetingStatus::Stopped)
        .cloned()
        .unwrap();
    let mut got: Vec<_> = poller.live.iter().map(key).collect();
    let mut want: Vec<_> = stand_in
        .segments
        .iter()
        .map(|s| key(&TranscriptSegment::stored(s)))
        .collect();
    got.sort();
    want.sort();
    assert_eq!(got, want);
    // The stored transcript is the completed row's, whole.
    let completed = rows.last();
    assert_eq!(completed.status, MeetingStatus::Completed);
    let stored: Vec<TranscriptSegment> = completed
        .segments
        .iter()
        .map(TranscriptSegment::stored)
        .collect();
    assert_eq!(poller.stored, stored);
    assert_eq!(
        poller.cursor.as_deref(),
        Some(format!("stored:{}", stored.len()).as_str())
    );
}

/// The files in the meeting directory the capture owns, with what
/// matters of the checkpoint (its write time aside).
fn capture_state(session: &Session, id: &str) -> (Vec<(String, u64)>, String, usize) {
    let dir = session.artifacts().dir(id);
    let mut files: Vec<(String, u64)> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                e.metadata().unwrap().len(),
            )
        })
        .filter(|(name, _)| name.ends_with(".wav") || name.ends_with("takes.json"))
        .collect();
    files.sort();
    let mut checkpoint = dettivo_meeting::checkpoint::Checkpoint::read(&dir).unwrap();
    checkpoint.updated_at.clear();
    let journal = std::fs::read_to_string(dir.join(dettivo_meeting::JOURNAL_FILE)).unwrap();
    let events = journal
        .lines()
        .filter(|l| !l.contains("\"checkpoint\""))
        .count();
    (files, serde_json::to_string(&checkpoint).unwrap(), events)
}

/// R7: thousands of reads while a meeting records write nothing and
/// slow nothing: the takes, the checkpoint and the journal stay as they
/// were, and every sample fed during the reads lands in the takes.
#[test]
fn reads_at_a_high_rate_leave_the_takes_and_the_checkpoint_unchanged() {
    let (session, _log, rows, feeds, _dir) = setup(Feeds::new(false, false));
    let id = session
        .start(
            policy(),
            feeds.clone(),
            FakeEngine::new(Duration::ZERO),
            MeetingRow::new(),
        )
        .unwrap()
        .meeting_id;
    speech(&feeds, 1900);
    silence(&feeds, 2000);
    // Two checkpoint intervals with nothing new: the capture is at rest.
    std::thread::sleep(Duration::from_millis(900));
    let before = capture_state(&session, &id);
    let reads = AtomicU64::new(0);
    let hammer = |until: &AtomicBool| {
        while !until.load(Ordering::SeqCst) {
            let read = session
                .read_segments(&id, None, || Ok::<_, ()>(rows.last().segments))
                .unwrap();
            assert_eq!(read.transcript, TranscriptSource::Live);
            reads.fetch_add(1, Ordering::Relaxed);
        }
    };
    let done = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| hammer(&done));
        scope.spawn(|| hammer(&done));
        std::thread::sleep(Duration::from_millis(700));
        done.store(true, Ordering::SeqCst);
    });
    assert!(reads.load(Ordering::Relaxed) > 1000, "{reads:?}");
    assert_eq!(capture_state(&session, &id), before);
    // Audio fed while the reads run lands in the takes whole.
    let done = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| hammer(&done));
        speech(&feeds, 3000);
        done.store(true, Ordering::SeqCst);
    });
    session.stop().unwrap();
    assert!(session.wait_idle(Duration::from_secs(20)));
    let dir = session.artifacts().dir(&id);
    for sidecar in ["takes.json", "system-takes.json"] {
        let takes = dettivo_audio::takes::read_sidecar(&dir.join(sidecar)).unwrap();
        let samples: u64 = takes.takes.iter().map(|t| t.samples).sum();
        assert_eq!(samples, (1900 + 2000 + 3000) * 16, "{sidecar}");
    }
}
