use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

/// A source the test feeds; its stop answers `Ended` like a capture.
fn channel() -> (Sender<Event>, Source, Arc<AtomicBool>) {
    let (tx, events) = mpsc::channel();
    let stopped = Arc::new(AtomicBool::new(false));
    let (ack, flag) = (tx.clone(), stopped.clone());
    let source = Source::Channel {
        events,
        on_stop: Arc::new(move || {
            if !flag.swap(true, Ordering::SeqCst) {
                let _ = ack.send(Event::Ended {
                    reason: EndReason::Stopped,
                });
            }
        }),
    };
    (tx, source, stopped)
}

fn settle() {
    std::thread::sleep(Duration::from_millis(30));
}

#[test]
fn a_take_gets_the_pre_roll_then_live_audio_until_its_stop() {
    let (mic, source, _) = channel();
    let feed = ReadyFeed::open(source).unwrap();
    let sent = Instant::now();
    mic.send(Event::pcm(vec![1; 16_000])).unwrap();
    settle();
    let take = feed
        .take(sent - Duration::from_millis(300))
        .expect("the feed is idle");
    let backlog = match take.events().recv().unwrap() {
        Event::Pcm { pcm, .. } => pcm,
        other => panic!("{other:?}"),
    };
    assert!(
        (4_800..=5_300).contains(&backlog.len()),
        "the last 300 ms of the second the ring held, trimmed to the sample: {}",
        backlog.len()
    );
    let live = Event::pcm(vec![2; 320]);
    mic.send(live.clone()).unwrap();
    assert_eq!(
        take.events().recv().unwrap(),
        live,
        "forwarded with its capture stamp"
    );
    assert!(feed.take(Instant::now()).is_none(), "one take at a time");
    take.stop();
    assert_eq!(
        take.events().recv().unwrap(),
        Event::Ended {
            reason: EndReason::Stopped
        }
    );
    mic.send(Event::pcm(vec![3; 320])).unwrap();
    settle();
    assert!(take.events().try_recv().is_err(), "nothing after the stop");
    assert!(!feed.tapped());
    let next = feed.take(Instant::now()).expect("free again");
    take.stop();
    let next_chunk = Event::pcm(vec![4; 320]);
    mic.send(next_chunk.clone()).unwrap();
    assert_eq!(
        next.events().recv().unwrap(),
        next_chunk,
        "a stale stop leaves the next take alone"
    );
}

#[test]
fn close_waits_for_the_capture_to_acknowledge_and_an_ended_capture_is_stale() {
    let (mic, source, stopped) = channel();
    let feed = ReadyFeed::open(source).unwrap();
    assert_eq!(feed.stale(), None);
    mic.send(Event::DeviceChanged {
        from: Some("a".into()),
        to: Some("b".into()),
    })
    .unwrap();
    settle();
    assert_eq!(feed.stale().as_deref(), Some("the default source moved"));
    feed.close();
    assert!(stopped.load(Ordering::SeqCst));

    let (mic, source, _) = channel();
    let feed = ReadyFeed::open(source).unwrap();
    mic.send(Event::Ended {
        reason: EndReason::NoSource,
    })
    .unwrap();
    settle();
    assert!(feed.stale().is_some_and(|s| s.contains("NoSource")));
    assert!(feed.take(Instant::now()).is_none());
}
