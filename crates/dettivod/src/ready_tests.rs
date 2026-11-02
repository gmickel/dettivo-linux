use super::*;
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc::{self, Sender};

use dettivo_audio::{EndReason, Event};

/// Counts the microphone captures open at once, as PipeWire would list
/// them, and the most it ever saw.
#[derive(Default)]
struct Microphone {
    live: AtomicUsize,
    most: AtomicUsize,
    opened: AtomicUsize,
    last: Mutex<Option<Sender<Event>>>,
}

impl Microphone {
    /// Opens a capture: counted until its stop is acknowledged.
    fn open(self: &Arc<Self>) -> Result<Source, String> {
        let live = self.live.fetch_add(1, Ordering::SeqCst) + 1;
        self.most.fetch_max(live, Ordering::SeqCst);
        self.opened.fetch_add(1, Ordering::SeqCst);
        let (tx, events) = mpsc::channel();
        *self.last.lock().unwrap() = Some(tx.clone());
        let mic = self.clone();
        let stopped = AtomicBool::new(false);
        Ok(Source::Channel {
            events,
            on_stop: Arc::new(move || {
                if !stopped.swap(true, Ordering::SeqCst) {
                    mic.live.fetch_sub(1, Ordering::SeqCst);
                    let _ = tx.send(Event::Ended {
                        reason: EndReason::Stopped,
                    });
                }
            }),
        })
    }

    /// The newest capture reports an event (a moved default, a lost device).
    fn report(&self, event: Event) {
        if let Some(tx) = self.last.lock().unwrap().as_ref() {
            tx.send(event).unwrap();
        }
        std::thread::sleep(Duration::from_millis(30));
    }

    fn live(&self) -> usize {
        self.live.load(Ordering::SeqCst)
    }
}

const MEETING: &str = "a meeting is capturing";

/// R4a: the meeting's start closes the ring inside the reservation before
/// the meeting opens its capture, a tick during the meeting keeps it
/// closed, and it reopens once the meeting's capture ended; the
/// microphone is never captured twice.
#[test]
fn the_meeting_handoff_never_captures_the_microphone_twice() {
    let mic = Arc::new(Microphone::default());
    let ready = MicReady::default();
    let reservation = Mutex::new(());
    ready.reconcile(Ok(()), || mic.open());
    assert_eq!((ready.status(), mic.live()), (Status::Open, 1));

    // meetings.start: one reservation around the close and the open.
    let meeting = {
        let _r = reservation.lock().unwrap();
        ready.close(MEETING);
        mic.open().unwrap()
    };
    assert_eq!(ready.status(), Status::Closed(MEETING.into()));
    ready.reconcile(closed_because(true, false, true, false), || mic.open());
    assert_eq!(mic.live(), 1, "the meeting's capture alone");

    meeting.stop();
    ready.reconcile(closed_because(true, false, false, false), || mic.open());
    assert_eq!((ready.status(), mic.live()), (Status::Open, 1));
    assert_eq!(mic.most.load(Ordering::SeqCst), 1);
}

/// R5: turning the setting off and locking the session close the ring;
/// R4: with the setting off nothing is open.
#[test]
fn disable_and_lock_close_the_ring() {
    let mic = Arc::new(Microphone::default());
    let ready = MicReady::default();
    ready.reconcile(closed_because(false, false, false, false), || mic.open());
    assert_eq!((ready.status(), mic.live()), (Status::Off, 0));
    ready.reconcile(Ok(()), || mic.open());
    ready.reconcile(closed_because(false, false, false, false), || mic.open());
    assert_eq!((ready.status(), mic.live()), (Status::Off, 0), "disabled");
    ready.reconcile(Ok(()), || mic.open());
    ready.reconcile(closed_because(true, true, false, false), || mic.open());
    assert_eq!(
        (ready.status(), mic.live()),
        (Status::Closed("the session is locked".into()), 0)
    );
}

/// A take that taps the ring keeps it through a lock; the ring closes
/// once the take stopped.
#[test]
fn a_take_keeps_the_ring_until_it_stops() {
    let mic = Arc::new(Microphone::default());
    let ready = MicReady::default();
    ready.reconcile(Ok(()), || mic.open());
    let take = ready.take(Instant::now()).expect("open and idle");
    let locked = closed_because(true, true, false, true);
    ready.reconcile(locked.clone(), || mic.open());
    assert_eq!(mic.live(), 1, "the take still records");
    take.stop();
    ready.reconcile(locked, || mic.open());
    assert_eq!(mic.live(), 0);
}

/// R6: a moved default and a lost device reopen the ring on the next
/// tick without anyone asking; a capture that cannot open is reported
/// and retried later, not on every tick.
#[test]
fn a_moved_or_lost_device_reopens_and_a_failure_is_reported() {
    let mic = Arc::new(Microphone::default());
    let ready = MicReady::default();
    ready.reconcile(Ok(()), || mic.open());
    mic.report(Event::DeviceChanged {
        from: Some("usb".into()),
        to: Some("laptop".into()),
    });
    ready.reconcile(Ok(()), || mic.open());
    assert_eq!(mic.opened.load(Ordering::SeqCst), 2);
    assert_eq!((ready.status(), mic.live()), (Status::Open, 1));

    mic.report(Event::Ended {
        reason: EndReason::DeviceLost("laptop".into()),
    });
    let mut tries = 0;
    ready.reconcile(Ok(()), || {
        tries += 1;
        Err("audio device \"laptop\" is not present".into())
    });
    assert_eq!(
        ready.status().wire(),
        MicrophoneReady {
            state: "failed".into(),
            reason: Some("audio device \"laptop\" is not present".into()),
        }
    );
    ready.reconcile(Ok(()), || {
        tries += 1;
        mic.open()
    });
    assert_eq!(tries, 1, "the next try waits for the retry");
}

#[test]
fn the_ring_waits_for_the_first_reason_that_applies() {
    assert_eq!(closed_because(false, true, true, true), Err(None));
    for (blocked, meeting, own, why) in [
        (true, true, true, "the session is locked"),
        (false, true, true, MEETING),
        (false, false, true, "a dictation records on its own capture"),
    ] {
        assert_eq!(
            closed_because(true, blocked, meeting, own),
            Err(Some(why.into()))
        );
    }
    assert_eq!(closed_because(true, false, false, false), Ok(()));
}
