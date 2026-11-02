//! The logind lock source against a stand-in logind on a private bus.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use dettivo_hotkeys::mock::PrivateBus;
use zbus::interface;
use zbus::zvariant::OwnedObjectPath;

use super::*;

const SESSION: &str = "/org/freedesktop/login1/session/t1";

struct User;

#[interface(name = "org.freedesktop.login1.User")]
impl User {
    #[zbus(property)]
    fn display(&self) -> (String, OwnedObjectPath) {
        ("t1".into(), OwnedObjectPath::try_from(SESSION).unwrap())
    }
}

/// A session whose `LockedHint` answers after `delay`, noting the answer.
struct Session {
    locked: bool,
    delay: Duration,
    answered: Arc<AtomicBool>,
}

#[interface(name = "org.freedesktop.login1.Session")]
impl Session {
    #[zbus(property)]
    async fn locked_hint(&self) -> bool {
        tokio::time::sleep(self.delay).await;
        self.answered.store(true, Ordering::SeqCst);
        self.locked
    }

    #[zbus(property)]
    fn active(&self) -> bool {
        true
    }
}

fn ring(r: &MicReady) -> &MicReady {
    r
}

async fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// QA81-5 for logind: the ring stays closed from the start of the watch
/// until logind's first read, at once or late, and then follows it; a
/// bus without logind leaves no lock source (`none`, ring free to open).
#[tokio::test(flavor = "multi_thread")]
async fn the_ring_waits_for_the_first_logind_read() {
    // (logind serves, starts locked, first reply delay)
    let cases = [
        (true, true, Duration::ZERO),
        (true, true, Duration::from_millis(400)),
        (true, false, Duration::from_millis(400)),
        (false, false, Duration::ZERO),
    ];
    for (serves, locked, delay) in cases {
        let bus = match PrivateBus::start() {
            Ok(b) => b,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        let answered = Arc::new(AtomicBool::new(false));
        let _logind = if serves {
            let session = Session {
                locked,
                delay,
                answered: answered.clone(),
            };
            Some(
                zbus::connection::Builder::address(bus.address())
                    .unwrap()
                    .name(LOGIND)
                    .unwrap()
                    .serve_at("/org/freedesktop/login1/user/self", User)
                    .unwrap()
                    .serve_at(SESSION, session)
                    .unwrap()
                    .build()
                    .await
                    .unwrap(),
            )
        } else {
            None
        };
        let ready = Arc::new(MicReady::default());
        let client = zbus::connection::Builder::address(bus.address())
            .unwrap()
            .build();
        let watching = tokio::spawn(follow(ready.clone(), ring, client));
        if serves {
            while !answered.load(Ordering::SeqCst) {
                assert!(
                    ready.blocked(),
                    "the ring opened before logind's first read ({delay:?})"
                );
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            if locked {
                tokio::time::sleep(Duration::from_millis(100)).await;
                assert!(ready.blocked(), "a locked session keeps the ring closed");
            } else {
                wait_for("the unlock", || !ready.blocked()).await;
            }
            assert_eq!(ready.lock_signal(), "logind");
            watching.abort();
        } else {
            watching.await.unwrap();
            assert_eq!(ready.lock_signal(), "none");
            assert!(!ready.blocked(), "without logind nothing holds the ring");
        }
    }
}
