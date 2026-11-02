//! QA81-1: the compositor's lock reaches the ring. A fake compositor on a
//! socket pair speaks just enough of the Wayland wire protocol (the
//! registry, a roundtrip, `hyprland_lock_notify_v1`) to stand in for
//! Hyprland locking under Omarchy's Quickshell lock; no display, no
//! environment and no live session are involved.

use super::*;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::Mutex;
use std::time::Instant;

use crate::ready::MicReady;

const NOTIFIER: &str = "hyprland_lock_notifier_v1";

/// What the compositor tells a new lock notification.
#[derive(Clone, Copy, PartialEq)]
enum Start {
    /// Nothing: the session is unlocked.
    Unlocked,
    /// `locked` at once.
    Locked,
    /// `locked` while answering the client's next roundtrip.
    LockedAtNextSync,
}

fn message(object: u32, opcode: u16, args: &[u8]) -> Vec<u8> {
    let size = 8 + args.len() as u32;
    let mut m = object.to_ne_bytes().to_vec();
    m.extend((size << 16 | u32::from(opcode)).to_ne_bytes());
    m.extend(args);
    m
}

fn string(text: &str) -> Vec<u8> {
    let mut s = ((text.len() + 1) as u32).to_ne_bytes().to_vec();
    s.extend(text.as_bytes());
    s.push(0);
    while s.len() % 4 != 0 {
        s.push(0);
    }
    s
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_ne_bytes(bytes[at..at + 4].try_into().unwrap())
}

/// The compositor's side of the socket.
struct Compositor {
    writer: Mutex<UnixStream>,
    /// The client's lock notification object, once it asked for one.
    notification: Mutex<Option<u32>>,
}

impl Compositor {
    fn send(&self, bytes: &[u8]) {
        let _ = self.writer.lock().unwrap().write_all(bytes);
    }

    /// Sends `locked` (true) or `unlocked` to the client's notification.
    fn lock(&self, locked: bool) {
        let id = self.notification.lock().unwrap().expect("a notification");
        self.send(&message(id, if locked { 0 } else { 1 }, &[]));
    }

    /// Answers the client's requests: globals (with the lock notifier when
    /// `offer`), roundtrips, the bind and the notification, telling the
    /// notification what `start` says.
    fn serve(&self, mut reader: UnixStream, offer: bool, start: Start) {
        let (mut registry, mut notifier) = (0, 0);
        let mut lock_at_sync = false;
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        while let Ok(n @ 1..) = reader.read(&mut chunk) {
            buf.extend_from_slice(&chunk[..n]);
            while buf.len() >= 8 && buf.len() >= (word(&buf, 4) >> 16) as usize {
                let size = (word(&buf, 4) >> 16) as usize;
                let msg: Vec<u8> = buf.drain(..size).collect();
                let (object, opcode) = (word(&msg, 0), word(&msg, 4) & 0xffff);
                match (object, opcode) {
                    (1, 1) => {
                        registry = word(&msg, 8);
                        let mut global = 1u32.to_ne_bytes().to_vec();
                        global.extend(string("wl_seat"));
                        global.extend(7u32.to_ne_bytes());
                        self.send(&message(registry, 0, &global));
                        if offer {
                            let mut global = 2u32.to_ne_bytes().to_vec();
                            global.extend(string(NOTIFIER));
                            global.extend(1u32.to_ne_bytes());
                            self.send(&message(registry, 0, &global));
                        }
                    }
                    (1, 0) => {
                        if std::mem::take(&mut lock_at_sync) {
                            self.lock(true);
                        }
                        let callback = word(&msg, 8);
                        self.send(&message(callback, 0, &0u32.to_ne_bytes()));
                        self.send(&message(1, 1, &callback.to_ne_bytes()));
                    }
                    (o, 0) if o == registry => notifier = word(&msg, msg.len() - 4),
                    (o, 1) if o == notifier && notifier != 0 => {
                        let id = word(&msg, 8);
                        *self.notification.lock().unwrap() = Some(id);
                        match start {
                            Start::Unlocked => {}
                            Start::Locked => self.lock(true),
                            Start::LockedAtNextSync => lock_at_sync = true,
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// A client connection to a fake compositor serving on its own thread.
fn connect(offer: bool, start: Start) -> (Connection, Arc<Compositor>) {
    let (client, server) = UnixStream::pair().unwrap();
    let compositor = Arc::new(Compositor {
        writer: Mutex::new(server.try_clone().unwrap()),
        notification: Mutex::new(None),
    });
    let c = compositor.clone();
    std::thread::spawn(move || c.serve(server, offer, start));
    (Connection::from_socket(client).unwrap(), compositor)
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(3);
    while !done() {
        assert!(Instant::now() < until, "{what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// An Omarchy lock (Hyprland's lock notification) blocks the ring and an
/// unlock frees it, also when the session was locked before the daemon
/// connected; a compositor lost while locked keeps the ring blocked.
#[test]
fn the_compositor_lock_blocks_the_ring() {
    for start in [Start::Unlocked, Start::Locked] {
        let starts_locked = start == Start::Locked;
        let ready = Arc::new(MicReady::default());
        let (conn, compositor) = connect(true, start);
        let r = ready.clone();
        let following = std::thread::spawn(move || {
            follow(&conn, move |locked| {
                r.set_lock(LockSource::Compositor, Some(locked));
            })
        });
        wait_for("the compositor's lock is followed", || {
            ready.lock_signal() == "compositor"
        });
        wait_for("the starting lock state", || {
            ready.blocked() == starts_locked
        });
        compositor.lock(true);
        wait_for("a lock blocks the ring", || ready.blocked());
        compositor.lock(false);
        wait_for("an unlock frees it", || !ready.blocked());
        compositor.lock(true);
        wait_for("locked again", || ready.blocked());

        compositor
            .writer
            .lock()
            .unwrap()
            .shutdown(std::net::Shutdown::Both)
            .unwrap();
        assert!(matches!(following.join().unwrap(), Err(Stop::Lost(_))));
        // What the production watcher does once the connection is gone.
        ready.set_lock(LockSource::Compositor, None);
        assert!(ready.blocked(), "a lost watcher keeps the lock it last saw");
    }
}

/// A compositor that does not offer the protocol is reported as such and
/// nothing about the lock is claimed.
#[test]
fn a_compositor_without_lock_notifications_is_not_followed() {
    let ready = Arc::new(MicReady::default());
    let (conn, _compositor) = connect(false, Start::Unlocked);
    let r = ready.clone();
    let stop = follow(&conn, move |locked| {
        r.set_lock(LockSource::Compositor, Some(locked));
    });
    assert!(matches!(stop, Err(Stop::NotOffered)), "{stop:?}");
    assert_eq!(ready.lock_signal(), "none");
}

/// QA81-5: a daemon that starts under a lock never sees an unlock before
/// the compositor's first `locked`, however late inside the first
/// roundtrip it comes, and the ring waits for the compositor meanwhile.
#[test]
fn a_session_locked_at_start_never_opens_the_ring() {
    for start in [Start::Locked, Start::LockedAtNextSync] {
        let ready = Arc::new(MicReady::default());
        ready.lock_pending(LockSource::Compositor);
        assert!(ready.blocked(), "the ring waits for the first report");
        let reports = Arc::new(Mutex::new(Vec::new()));
        let (conn, _compositor) = connect(true, start);
        let (r, seen) = (ready.clone(), reports.clone());
        std::thread::spawn(move || {
            follow(&conn, move |locked| {
                r.set_lock(LockSource::Compositor, Some(locked));
                seen.lock().unwrap().push((locked, r.blocked()));
            })
        });
        wait_for("the first report", || !reports.lock().unwrap().is_empty());
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(*reports.lock().unwrap(), [(true, true)]);
    }
}

/// A fake compositor serving the first client that connects on `socket`.
fn compositor_on(socket: &Path, start: Start) -> std::sync::mpsc::Receiver<Arc<Compositor>> {
    let _ = std::fs::remove_file(socket);
    let listener = std::os::unix::net::UnixListener::bind(socket).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (server, _) = listener.accept().unwrap();
        // One client only: a later connection is refused, as by a
        // compositor that crashed.
        drop(listener);
        let compositor = Arc::new(Compositor {
            writer: Mutex::new(server.try_clone().unwrap()),
            notification: Mutex::new(None),
        });
        let _ = tx.send(compositor.clone());
        compositor.serve(server, true, start);
    });
    rx
}

/// fn-91 R3: the compositor restarts on the same display. While it is
/// down the socket refuses connections; the watcher keeps trying and
/// follows the new compositor's lock once it is up.
#[test]
fn the_lock_is_followed_again_after_a_compositor_restart() {
    let dir = tempfile::Builder::new()
        .prefix("dtv")
        .tempdir_in("/tmp")
        .unwrap();
    let socket = dir.path().join("wl");
    let ready = Arc::new(MicReady::default());
    ready.lock_pending(LockSource::Compositor);
    let first = compositor_on(&socket, Start::Unlocked);
    let (r, s) = (ready.clone(), socket.clone());
    std::thread::spawn(move || {
        follow_display(&s, move |locked| r.set_lock(LockSource::Compositor, locked))
    });
    let first = first.recv_timeout(Duration::from_secs(3)).unwrap();
    wait_for("the first compositor reports", || !ready.blocked());
    first.lock(true);
    wait_for("the first compositor's lock", || ready.blocked());

    // The crash: the connection drops, the socket file stays and refuses.
    first
        .writer
        .lock()
        .unwrap()
        .shutdown(std::net::Shutdown::Both)
        .unwrap();
    std::thread::sleep(Duration::from_millis(600));
    assert!(ready.blocked(), "a lost compositor keeps the last lock");

    let second = compositor_on(&socket, Start::Unlocked);
    second.recv_timeout(Duration::from_secs(3)).unwrap();
    wait_for("the new compositor's unlock frees the ring", || {
        !ready.blocked()
    });
}
