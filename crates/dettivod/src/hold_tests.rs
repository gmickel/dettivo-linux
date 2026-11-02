//! The hold key against a session whose start and discard are slow, as
//! a cold engine or `pause_media` make them: every edge counts from when
//! it arrived, not from when the key got to it.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::*;

/// A session that takes `slow` to start a take and to drop one.
struct Fake {
    slow: Duration,
    recording: Mutex<Option<String>>,
    started: Mutex<u32>,
    /// What `lock` answers: false when the take ended meanwhile.
    lockable: bool,
}

impl Fake {
    fn new(slow: Duration) -> Self {
        Self {
            slow,
            recording: Mutex::new(None),
            started: Mutex::new(0),
            lockable: true,
        }
    }
}

impl Takes for Fake {
    fn recording(&self) -> Option<String> {
        self.recording.lock().unwrap().clone()
    }
    fn start(&self, _: Instant) -> Result<String, JsonRpcError> {
        std::thread::sleep(self.slow);
        let mut n = self.started.lock().unwrap();
        *n += 1;
        let job = format!("job_{n}");
        *self.recording.lock().unwrap() = Some(job.clone());
        Ok(job)
    }
    fn discard(&self, _job: &str) {
        std::thread::sleep(self.slow);
        *self.recording.lock().unwrap() = None;
    }
    fn release_waiting(&self, _: Option<Instant>) {}
    fn lock(&self, _job: &str) -> bool {
        self.lockable
    }
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn action(key: &HoldKey, takes: &Fake, edge: Edge, at: Instant) -> String {
    key.on(takes, edge, at).unwrap().result.action
}

#[test]
fn a_hold_whose_start_outlasted_it_is_kept_and_its_repeat_ignored() {
    let takes = Fake::new(ms(600));
    let key = HoldKey::default();
    let t0 = Instant::now();
    assert_eq!(action(&key, &takes, Edge::Press, t0), "started");
    // The repeat and the release arrived while the start ran: the repeat
    // is the key repeating, the release ends a 400 ms hold.
    assert_eq!(action(&key, &takes, Edge::Press, t0 + ms(30)), "none");
    assert_eq!(action(&key, &takes, Edge::Release, t0 + ms(400)), "stopped");
}

#[test]
fn a_double_tap_behind_a_slow_discard_still_locks() {
    let takes = Fake::new(ms(500));
    let key = HoldKey::default();
    let t0 = Instant::now();
    assert_eq!(action(&key, &takes, Edge::Press, t0), "started");
    assert_eq!(
        action(&key, &takes, Edge::Release, t0 + ms(100)),
        "discarded"
    );
    // The second press arrived 200 ms after the tap, while the tapped
    // take was still being dropped.
    assert_eq!(action(&key, &takes, Edge::Press, t0 + ms(300)), "started");
    assert_eq!(action(&key, &takes, Edge::Release, t0 + ms(400)), "locked");
    assert!(key.locked("job_2"));
    assert_eq!(action(&key, &takes, Edge::Press, Instant::now()), "stopped");
    assert!(!key.locked("job_2"));
}

#[test]
fn a_lock_for_a_take_that_ended_meanwhile_leaves_the_key_idle() {
    let mut takes = Fake::new(Duration::ZERO);
    takes.lockable = false;
    let key = HoldKey::default();
    let t0 = Instant::now();
    action(&key, &takes, Edge::Press, t0);
    action(&key, &takes, Edge::Release, t0 + ms(100));
    action(&key, &takes, Edge::Press, t0 + ms(200));
    let r = key.on(&takes, Edge::Release, t0 + ms(300)).unwrap().result;
    assert_eq!((r.action.as_str(), r.hold.as_str()), ("none", "idle"));
    assert!(!key.locked("job_2"));
}

/// Edges from separate `dictation press` and `release` calls that wait
/// behind a slow start act in the order they arrived, so a double tap
/// made while the first take starts still locks the second.
#[test]
fn edges_queued_behind_a_slow_start_act_in_arrival_order() {
    let takes = Fake::new(ms(400));
    let key = HoldKey::default();
    let t0 = Instant::now();
    let edges = [
        (Edge::Press, 0),
        (Edge::Release, 100),
        (Edge::Press, 200),
        (Edge::Release, 300),
    ];
    let actions: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = edges
            .iter()
            .map(|&(edge, at)| {
                let (key, takes) = (&key, &takes);
                let handle = s.spawn(move || action(key, takes, edge, t0 + ms(at)));
                // Each arrives, and waits, before the next.
                std::thread::sleep(ms(40));
                handle
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert_eq!(actions, ["started", "discarded", "started", "locked"]);
}
