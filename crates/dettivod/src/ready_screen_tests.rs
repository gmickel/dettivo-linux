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
    /// `offer`), roundtrips, the bind and the notification, sending
    /// `locked` at once when the session `starts_locked`.
    fn serve(&self, mut reader: UnixStream, offer: bool, starts_locked: bool) {
        let (mut registry, mut notifier) = (0, 0);
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
                        let callback = word(&msg, 8);
                        self.send(&message(callback, 0, &0u32.to_ne_bytes()));
                        self.send(&message(1, 1, &callback.to_ne_bytes()));
                    }
                    (o, 0) if o == registry => notifier = word(&msg, msg.len() - 4),
                    (o, 1) if o == notifier && notifier != 0 => {
                        let id = word(&msg, 8);
                        *self.notification.lock().unwrap() = Some(id);
                        if starts_locked {
                            self.lock(true);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// A client connection to a fake compositor serving on its own thread.
fn connect(offer: bool, starts_locked: bool) -> (Connection, Arc<Compositor>) {
    let (client, server) = UnixStream::pair().unwrap();
    let compositor = Arc::new(Compositor {
        writer: Mutex::new(server.try_clone().unwrap()),
        notification: Mutex::new(None),
    });
    let c = compositor.clone();
    std::thread::spawn(move || c.serve(server, offer, starts_locked));
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
/// connected; a lost compositor leaves the lock to logind.
#[test]
fn the_compositor_lock_blocks_the_ring() {
    for starts_locked in [false, true] {
        let ready = Arc::new(MicReady::default());
        let (conn, compositor) = connect(true, starts_locked);
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

        compositor
            .writer
            .lock()
            .unwrap()
            .shutdown(std::net::Shutdown::Both)
            .unwrap();
        assert!(matches!(following.join().unwrap(), Err(Stop::Lost(_))));
    }
}

/// A compositor that does not offer the protocol is reported as such and
/// nothing about the lock is claimed.
#[test]
fn a_compositor_without_lock_notifications_is_not_followed() {
    let ready = Arc::new(MicReady::default());
    let (conn, _compositor) = connect(false, false);
    let r = ready.clone();
    let stop = follow(&conn, move |locked| {
        r.set_lock(LockSource::Compositor, Some(locked));
    });
    assert!(matches!(stop, Err(Stop::NotOffered)), "{stop:?}");
    assert_eq!(ready.lock_signal(), "none");
}
