//! The transcription of a meeting (ADR 0030) over channel sources and the
//! fake engine: live windows of both tracks arrive as provisional then
//! final segments per source in time order, a restarted microphone take
//! carries a gap marker, an engine that lags more than two ticks has the
//! oldest audio skipped with a journal line and the finalisation still
//! covers it, the finalisation interleaves every take of both tracks
//! with progress per chunk and stores the transcript on the row, a
//! finalisation can be cancelled and leaves the meeting stopped with its
//! audio, a recovered meeting finalises the same way, the live path can
//! be off, and an engine without timestamps is refused by name.

mod common;

use std::sync::atomic::Ordering;
use std::time::Duration;

use common::{FakeEngine, Feeds, policy, setup, states, tone};
use dettivo_audio::{EndReason, Event};
use dettivo_meeting::{JOURNAL_FILE, Policy, State, journal};
use dettivo_storage::meetings::{MeetingRow, MeetingStatus};
use dettivo_transcribe::Stage;
use dettivo_transcribe::live::Source;

fn feed_speech(feeds: &Feeds, mic_ms: u64, sys_ms: u64) {
    // 100 ms chunks, as a capture delivers them.
    for _ in 0..mic_ms / 100 {
        feeds.mic(0).send(Event::Pcm(tone(100, 3000))).unwrap();
    }
    for _ in 0..sys_ms / 100 {
        feeds.system().send(Event::Pcm(tone(100, 3000))).unwrap();
    }
}

#[test]
fn live_segments_arrive_provisional_then_final_per_source_and_finalisation_covers_every_take() {
    let (session, log, rows, feeds, _dir) = setup(Feeds::new(false, false));
    let engine = FakeEngine::new(Duration::ZERO);
    let snapshot = session
        .start(policy(), feeds.clone(), engine.clone(), MeetingRow::new())
        .unwrap();
    let id = snapshot.meeting_id.clone();
    // Two ticks of speech on both tracks, then silence long enough for
    // the boundary gap to pass.
    feed_speech(&feeds, 1900, 1900);
    std::thread::sleep(Duration::from_millis(300));
    for _ in 0..30 {
        feeds.mic(0).send(Event::Pcm(vec![0; 1600])).unwrap();
        feeds.system().send(Event::Pcm(vec![0; 1600])).unwrap();
    }
    std::thread::sleep(Duration::from_millis(400));
    {
        let segments = log.segments.lock().unwrap();
        assert!(segments.iter().all(|(m, _)| *m == id));
        for source in [Source::You, Source::Remote] {
            let mine: Vec<_> = segments
                .iter()
                .filter(|(_, s)| s.source == source)
                .collect();
            assert!(
                mine.iter().any(|(_, s)| s.provisional) && mine.iter().any(|(_, s)| !s.provisional),
                "{source:?}: provisional then final: {mine:?}"
            );
            let first_final = mine.iter().position(|(_, s)| !s.provisional).unwrap();
            assert!(
                mine[..first_final].iter().all(|(_, s)| s.provisional),
                "{source:?}: provisional segments come first"
            );
            let finals: Vec<u64> = mine
                .iter()
                .filter(|(_, s)| !s.provisional)
                .map(|(_, s)| s.start_ms)
                .collect();
            assert!(
                finals.windows(2).all(|w| w[0] <= w[1]),
                "{source:?} in time order"
            );
            assert!(mine.iter().all(|(_, s)| s.text.starts_with("word")));
            assert!(
                mine.iter()
                    .any(|(_, s)| s.id.starts_with(&format!("{}-p", source.as_str())))
            );
            assert!(
                mine.iter()
                    .any(|(_, s)| s.id == format!("{}-0", source.as_str()))
            );
        }
    }
    let live = session.snapshot().unwrap();
    assert!(live.live_segment_count >= 2, "{live:?}");
    assert!(live.live_last_end_ms > 0);
    let checkpoint = dettivo_meeting::checkpoint::Checkpoint::read(&session.artifacts().dir(&id))
        .expect("a checkpoint with the tail");
    assert!(!checkpoint.segments.is_empty());
    assert!(checkpoint.next_sequence >= 1);

    // A device switch: the next microphone take carries the gap.
    feeds
        .mic(0)
        .send(Event::Ended {
            reason: EndReason::DeviceLost("usb-headset".into()),
        })
        .unwrap();
    std::thread::sleep(Duration::from_millis(150));
    for _ in 0..19 {
        feeds.mic(1).send(Event::Pcm(tone(100, 3000))).unwrap();
    }
    std::thread::sleep(Duration::from_millis(300));
    let calls_before_stop = engine.calls.load(Ordering::SeqCst);
    session.stop().unwrap();
    assert!(session.wait_idle(Duration::from_secs(10)));
    assert!(
        log.segments
            .lock()
            .unwrap()
            .iter()
            .any(|(_, s)| s.source == Source::You && s.gap_before_ms.is_some()),
        "the live segment after the switch carries the gap: {:?}",
        log.segments.lock().unwrap()
    );

    // The finalisation: three takes (two microphone, one system) through
    // the offline pipeline, progress per chunk, the transcript on the row.
    let row = rows.last();
    assert_eq!(row.status, MeetingStatus::Completed, "{row:?}");
    assert!(engine.calls.load(Ordering::SeqCst) >= calls_before_stop + 3);
    let sources: Vec<&str> = row
        .segments
        .iter()
        .map(|s| match s.source_type {
            dettivo_proto::methods::meetings::SegmentSource::System => "remote",
            _ => "you",
        })
        .collect();
    assert!(
        sources.contains(&"you") && sources.contains(&"remote"),
        "{sources:?}"
    );
    assert!(
        row.segments
            .windows(2)
            .all(|w| w[0].start_ms <= w[1].start_ms),
        "interleaved on the meeting clock"
    );
    assert!(
        row.segments.iter().any(|s| s.gap_before_ms.is_some()),
        "the second take's segment carries the gap: {:?}",
        row.segments
    );
    assert_eq!(row.final_text.split(' ').count(), row.segments.len());
    assert_eq!(row.chunks_completed, 3);
    assert_eq!(row.chunks_total, 3);
    let progress = log.progress.lock().unwrap();
    let done: Vec<u32> = progress
        .iter()
        .filter(|(_, p)| p.stage == Stage::Transcribing)
        .map(|(_, p)| p.chunks_done)
        .collect();
    assert_eq!(done, [0, 1, 2, 3], "{progress:?}");
    assert_eq!(progress.last().unwrap().1.stage, Stage::Done);
    assert!(progress.iter().all(|(j, _)| j == "job_meeting_1"));
    let seen = states(&log);
    let last = seen.last().unwrap();
    assert_eq!(last.0, State::Completed);
    let transcribing = log
        .states
        .lock()
        .unwrap()
        .iter()
        .find(|c| c.state == State::Transcribing)
        .cloned()
        .unwrap();
    assert!(transcribing.is_finalizing);
    assert!(transcribing.live_segment_count >= 2);
    let entries = journal::read(&session.artifacts().dir(&id).join(JOURNAL_FILE));
    let events: Vec<&str> = entries.iter().map(|e| e.event.as_str()).collect();
    assert!(events.contains(&"finalize") && events.contains(&"finalized"));
}

#[test]
fn a_lagging_engine_skips_windows_with_a_journal_line_and_finalisation_still_covers_them() {
    let (session, log, rows, feeds, _dir) = setup(Feeds::new(false, true));
    let engine = FakeEngine::new(Duration::from_millis(400));
    let snapshot = session
        .start(policy(), feeds.clone(), engine.clone(), MeetingRow::new())
        .unwrap();
    // Six seconds of speech arrive at once: more than two ticks past a
    // full window, so the oldest audio is skipped and the newest window
    // is what the engine sees.
    feeds.mic(0).send(Event::Pcm(tone(6000, 3000))).unwrap();
    std::thread::sleep(Duration::from_millis(900));
    session.stop().unwrap();
    assert!(session.wait_idle(Duration::from_secs(10)));
    let dir = session.artifacts().dir(&snapshot.meeting_id);
    let entries = journal::read(&dir.join(JOURNAL_FILE));
    let skip = entries
        .iter()
        .find(|e| e.event == "live_skip")
        .expect("a skip line");
    assert!(
        skip.detail.as_deref().unwrap().starts_with("0-3450 ms"),
        "{skip:?}"
    );
    let lengths = engine.lengths.lock().unwrap().clone();
    assert!(
        lengths.contains(&3000),
        "the full window after the skip: {lengths:?}"
    );
    assert!(
        lengths.last().copied().unwrap_or(0) >= 5900,
        "the finalisation read the whole take: {lengths:?}"
    );
    let row = rows.last();
    assert_eq!(row.status, MeetingStatus::Completed);
    assert!(!row.segments.is_empty());
    let live_gap = log
        .segments
        .lock()
        .unwrap()
        .iter()
        .any(|(_, s)| s.gap_before_ms.is_some());
    assert!(live_gap, "the live view shows the skip as a gap");
}

#[test]
fn a_cancelled_finalisation_leaves_the_meeting_stopped_with_its_audio() {
    let (session, log, rows, feeds, _dir) = setup(Feeds::new(false, true));
    let engine = FakeEngine::new(Duration::from_millis(300));
    // Three chunks of eight seconds, so the cancel lands between two.
    let snapshot = session
        .start(
            Policy {
                live: false,
                transcribe: dettivo_transcribe::Settings {
                    chunk_seconds: 8,
                    ..dettivo_transcribe::Settings::default()
                },
                ..policy()
            },
            feeds.clone(),
            engine.clone(),
            MeetingRow::new(),
        )
        .unwrap();
    let id = snapshot.meeting_id.clone();
    feeds.mic(0).send(Event::Pcm(tone(20_000, 3000))).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    session.stop().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while session.finalizing(&id).is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "finalisation never started"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let f = session.finalizing(&id).unwrap();
    assert_eq!(f.job_id, "job_meeting_1");
    assert!(
        session.snapshot().is_none(),
        "the slot is free while finalising"
    );
    std::thread::sleep(Duration::from_millis(100));
    assert!(session.cancel_finalize(&id));
    assert!(session.wait_idle(Duration::from_secs(5)));
    let row = rows.last();
    assert_eq!(row.status, MeetingStatus::Stopped, "{row:?}");
    assert!(row.segments.is_empty());
    assert!(
        session
            .artifacts()
            .dir(&id)
            .join("microphone.wav")
            .is_file()
    );
    assert!(log.segments.lock().unwrap().is_empty(), "live was off");
    let last = states(&log).last().unwrap().clone();
    assert_eq!(last.0, State::Stopped);
    assert!(last.2.as_deref().unwrap().contains("cancelled"));
    assert!(!session.cancel_finalize(&id), "nothing left to cancel");
}

/// meetings/F2 (fn-43): the final row is stored before anything is
/// cleaned up. A store that refuses it keeps the takes and the
/// finalisation checkpoint, ends the meeting failed with the storage
/// error and announces no completion; a retry once the store answers
/// finishes from the takes and only then applies the audio policy.
#[test]
fn a_store_that_refuses_the_final_row_keeps_the_takes_and_ends_failed_not_completed() {
    let (session, log, rows, feeds, _dir) = setup(Feeds::new(false, false));
    let engine = FakeEngine::new(Duration::ZERO);
    let policy = Policy {
        keep_audio: false,
        ..policy()
    };
    let snapshot = session
        .start(
            policy.clone(),
            feeds.clone(),
            engine.clone(),
            MeetingRow::new(),
        )
        .unwrap();
    let id = snapshot.meeting_id.clone();
    feeds.mic(0).send(Event::Pcm(tone(1000, 3000))).unwrap();
    feeds.system().send(Event::Pcm(tone(1000, 3000))).unwrap();
    std::thread::sleep(Duration::from_millis(400));
    rows.refuse_completed.store(true, Ordering::SeqCst);
    session.stop().unwrap();
    assert!(session.wait_idle(Duration::from_secs(10)));
    let dir = session.artifacts().dir(&id);
    let takes = || {
        std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "wav"))
            .count()
    };
    assert!(takes() >= 2, "the takes stay for a retry");
    assert!(
        dettivo_meeting::checkpoint::Checkpoint::read(&dir).is_ok(),
        "the finalisation checkpoint stays"
    );
    let seen = states(&log);
    assert!(
        !seen.iter().any(|(s, _, _)| *s == State::Completed),
        "no completion is announced: {seen:?}"
    );
    let last = seen.last().unwrap();
    assert_eq!(last.0, State::Failed, "{seen:?}");
    assert!(
        last.2
            .as_deref()
            .unwrap_or("")
            .contains("database is locked"),
        "the reason names the storage error: {seen:?}"
    );
    let row = rows.last();
    assert_eq!(row.status, MeetingStatus::Failed);
    assert_eq!(row.error_code.as_deref(), Some("storage"));
    assert_eq!(row.audio_dir.as_deref(), Some(dir.to_str().unwrap()));
    // The store answers again: the retry finishes from the takes.
    rows.refuse_completed.store(false, Ordering::SeqCst);
    session
        .finalize_recovered(policy, engine, rows.last())
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while rows.last().status != MeetingStatus::Completed {
        assert!(std::time::Instant::now() < deadline, "{:?}", rows.last());
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(session.wait_idle(Duration::from_secs(5)));
    assert_eq!(takes(), 0, "the audio policy runs once the row is stored");
    assert!(!rows.last().segments.is_empty());
}

#[test]
fn a_recovered_meeting_finalises_from_its_takes() {
    // A kill mid-meeting: the worker is cancelled once the checkpoint has
    // flushed the takes, the directory is promoted the way the next
    // daemon start does it, and the recovery finalises from the takes.
    let (session, log, rows, feeds, _dir) = setup(Feeds::new(false, false));
    let engine = FakeEngine::new(Duration::ZERO);
    let snapshot = session
        .start(policy(), feeds.clone(), engine.clone(), MeetingRow::new())
        .unwrap();
    let id = snapshot.meeting_id.clone();
    feeds.mic(0).send(Event::Pcm(tone(1000, 3000))).unwrap();
    feeds.system().send(Event::Pcm(tone(1000, 3000))).unwrap();
    std::thread::sleep(Duration::from_millis(400));
    let dir = session.artifacts().dir(&id);
    let kept = tempfile::tempdir().unwrap();
    let copy = kept.path().join(&id);
    std::fs::create_dir_all(&copy).unwrap();
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        std::fs::copy(entry.path(), copy.join(entry.file_name())).unwrap();
    }
    session.cancel().unwrap();
    let (session, log2, rows2, _feeds2, _dir2) = setup(Feeds::new(false, false));
    let dir = session.artifacts().dir(&id);
    std::fs::create_dir_all(dir.parent().unwrap()).unwrap();
    std::fs::rename(&copy, &dir).unwrap();
    let mut row = rows.last();
    row.id = id.clone();
    let promoted = dettivo_meeting::recovery::promote(&dir, &mut row);
    assert!(promoted.microphone_takes >= 1);
    assert_eq!(row.status, MeetingStatus::Partial);
    let job = session
        .finalize_recovered(policy(), engine.clone(), row)
        .unwrap();
    assert_eq!(job, "job_recover_1");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while rows2.last().status != MeetingStatus::Completed {
        assert!(std::time::Instant::now() < deadline, "{:?}", rows2.last());
        std::thread::sleep(Duration::from_millis(20));
    }
    let row = rows2.last();
    assert!(!row.is_partial);
    assert!(!row.segments.is_empty(), "{row:?}");
    assert!(
        log2.progress
            .lock()
            .unwrap()
            .iter()
            .all(|(j, _)| j == "job_recover_1")
    );
    let seen = states(&log2);
    assert!(
        seen.iter()
            .any(|(s, p, _)| *s == State::Transcribing && *p == State::Partial)
    );
    assert_eq!(seen.last().unwrap().0, State::Completed);
    assert!(session.wait_idle(Duration::from_secs(5)));
    drop(log);
}

#[test]
fn an_engine_without_timestamps_is_refused_by_name_live_and_at_finalisation() {
    let (session, log, rows, feeds, _dir) = setup(Feeds::new(false, true));
    let engine = FakeEngine::without_timestamps();
    let snapshot = session
        .start(policy(), feeds.clone(), engine.clone(), MeetingRow::new())
        .unwrap();
    feeds.mic(0).send(Event::Pcm(tone(2000, 3000))).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    session.stop().unwrap();
    assert!(session.wait_idle(Duration::from_secs(5)));
    let dir = session.artifacts().dir(&snapshot.meeting_id);
    let entries = journal::read(&dir.join(JOURNAL_FILE));
    let unavailable = entries
        .iter()
        .find(|e| e.event == "live_unavailable")
        .expect("the live path was refused");
    assert!(
        unavailable
            .detail
            .as_deref()
            .unwrap()
            .contains("provider fake"),
        "{unavailable:?}"
    );
    assert!(log.segments.lock().unwrap().is_empty());
    let row = rows.last();
    assert_eq!(row.status, MeetingStatus::Failed);
    assert_eq!(row.error_code.as_deref(), Some("transcription"));
    assert!(
        row.error_message
            .as_deref()
            .unwrap()
            .contains("provider fake"),
        "{row:?}"
    );
    assert!(dir.join("microphone.wav").is_file(), "the takes stay");
    assert_eq!(states(&log).last().unwrap().0, State::Failed);
}
