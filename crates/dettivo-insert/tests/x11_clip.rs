//! The X11 clipboard against a private owner on a disposable display.
//! These run only inside `scripts/qa/xvfb-session.sh` with
//! `DETTIVO_X11_CLIP_TEST=1`: they take the `CLIPBOARD` selection, which
//! must never be the clipboard of a desktop someone is using.
//!
//! `DETTIVO_X11_CLIP_TEST=1 scripts/qa/xvfb-session.sh cargo test -p dettivo-insert --test x11_clip`

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use dettivo_insert::clip::{Clip, ClipboardSnapshot, Restored, X11Clip};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, CreateWindowAux, EventMask,
    PropMode, Property, SELECTION_NOTIFY_EVENT, SelectionNotifyEvent, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{CURRENT_TIME, NONE};

/// One clipboard at a time: the tests share the display's selection.
static SELECTION: Mutex<()> = Mutex::new(());

fn private_display() -> Option<MutexGuard<'static, ()>> {
    let private = std::env::var("DETTIVO_X11_CLIP_TEST").as_deref() == Ok("1")
        && std::env::var_os("WAYLAND_DISPLAY").is_none()
        && std::env::var_os("DISPLAY").is_some();
    if !private {
        eprintln!("skip: run under scripts/qa/xvfb-session.sh with DETTIVO_X11_CLIP_TEST=1");
        return None;
    }
    Some(SELECTION.lock().unwrap_or_else(|p| p.into_inner()))
}

fn connect() -> (RustConnection, Window) {
    let (conn, screen) = x11rb::connect(None).unwrap();
    let root = conn.setup().roots[screen].root;
    let window = conn.generate_id().unwrap();
    conn.create_window(
        0,
        window,
        root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        0,
        &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )
    .unwrap();
    (conn, window)
}

fn atom(conn: &RustConnection, name: &str) -> Atom {
    conn.intern_atom(false, name.as_bytes())
        .unwrap()
        .reply()
        .unwrap()
        .atom
}

/// How the private owner answers one target.
enum Answer {
    /// A property of this type and format.
    Plain(&'static str, u8, Vec<u8>),
    /// The bytes, then a second `SelectionNotify` for the same request.
    Twice(Vec<u8>),
    /// An `INCR` transfer of these chunks. While it is unfinished, every
    /// later reply is followed by `late` written to its property, as an
    /// owner that queues its chunks would.
    Incr(Vec<Vec<u8>>, Vec<u8>),
}

/// A clipboard owner on a connection of its own.
struct PrivateOwner {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for PrivateOwner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn own(answers: Vec<(&'static str, Answer)>) -> PrivateOwner {
    let (conn, window) = connect();
    let clipboard = atom(&conn, "CLIPBOARD");
    conn.set_selection_owner(window, clipboard, CURRENT_TIME)
        .unwrap();
    let owner = conn
        .get_selection_owner(clipboard)
        .unwrap()
        .reply()
        .unwrap();
    assert_eq!(owner.owner, window, "the private owner holds the clipboard");
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let thread = std::thread::spawn(move || serve(conn, answers, &stopping));
    PrivateOwner {
        stop,
        thread: Some(thread),
    }
}

/// The unfinished transfer: requestor, property, chunks left, late bytes.
type Transfer = (Window, Atom, Vec<Vec<u8>>, Vec<u8>);

fn serve(conn: RustConnection, answers: Vec<(&'static str, Answer)>, stop: &AtomicBool) {
    let targets = atom(&conn, "TARGETS");
    let incr = atom(&conn, "INCR");
    let atoms: Vec<Atom> = answers.iter().map(|(n, _)| atom(&conn, n)).collect();
    let mut transfer: Option<Transfer> = None;
    while !stop.load(Ordering::Acquire) {
        let Ok(Some(event)) = conn.poll_for_event() else {
            std::thread::sleep(Duration::from_millis(2));
            continue;
        };
        match event {
            Event::SelectionRequest(r) => {
                let notify = SelectionNotifyEvent {
                    response_type: SELECTION_NOTIFY_EVENT,
                    sequence: 0,
                    time: r.time,
                    requestor: r.requestor,
                    selection: r.selection,
                    target: r.target,
                    property: r.property,
                };
                let mut times = 1;
                if r.target == targets {
                    let mut list = vec![targets];
                    list.extend(&atoms);
                    let _ = conn.change_property32(
                        PropMode::REPLACE,
                        r.requestor,
                        r.property,
                        AtomEnum::ATOM,
                        &list,
                    );
                } else if let Some(i) = atoms.iter().position(|a| *a == r.target) {
                    match &answers[i].1 {
                        Answer::Plain(type_name, format, bytes) => {
                            let type_ = atom(&conn, type_name);
                            let units = bytes.len() / usize::from(format / 8);
                            let _ = conn.change_property(
                                PropMode::REPLACE,
                                r.requestor,
                                r.property,
                                type_,
                                *format,
                                units as u32,
                                bytes,
                            );
                        }
                        Answer::Twice(bytes) => {
                            let _ = conn.change_property8(
                                PropMode::REPLACE,
                                r.requestor,
                                r.property,
                                r.target,
                                bytes,
                            );
                            times = 2;
                        }
                        Answer::Incr(chunks, late) => {
                            let total: usize = chunks.iter().map(Vec::len).sum();
                            let _ = conn.change_window_attributes(
                                r.requestor,
                                &ChangeWindowAttributesAux::new()
                                    .event_mask(EventMask::PROPERTY_CHANGE),
                            );
                            let _ = conn.change_property32(
                                PropMode::REPLACE,
                                r.requestor,
                                r.property,
                                incr,
                                &[total as u32],
                            );
                            let mut queue = chunks.clone();
                            queue.reverse();
                            transfer = Some((r.requestor, r.property, queue, late.clone()));
                            for _ in 0..times {
                                let _ = conn.send_event(
                                    false,
                                    r.requestor,
                                    EventMask::NO_EVENT,
                                    notify,
                                );
                            }
                            let _ = conn.flush();
                            continue;
                        }
                    }
                } else {
                    let _ = conn.send_event(
                        false,
                        r.requestor,
                        EventMask::NO_EVENT,
                        SelectionNotifyEvent {
                            property: NONE,
                            ..notify
                        },
                    );
                    let _ = conn.flush();
                    continue;
                }
                if let Some((w, p, left, late)) = &mut transfer
                    && !left.is_empty()
                    && !late.is_empty()
                {
                    let _ =
                        conn.change_property8(PropMode::REPLACE, *w, *p, AtomEnum::STRING, late);
                    late.clear();
                }
                for _ in 0..times {
                    let _ = conn.send_event(false, r.requestor, EventMask::NO_EVENT, notify);
                }
                let _ = conn.flush();
            }
            Event::PropertyNotify(p) if p.state == Property::DELETE => {
                if let Some((w, prop, left, _)) = &mut transfer
                    && *w == p.window
                    && *prop == p.atom
                {
                    let chunk = left.pop().unwrap_or_default();
                    let _ = conn.change_property8(
                        PropMode::REPLACE,
                        *w,
                        *prop,
                        AtomEnum::STRING,
                        &chunk,
                    );
                    let _ = conn.flush();
                }
            }
            _ => {}
        }
    }
}

/// The selection converted to `target` by a fresh requestor: the reply's
/// type name, format and bytes, `None` when refused.
fn read(target: &str) -> Option<(String, u8, Vec<u8>)> {
    let (conn, window) = connect();
    let clipboard = atom(&conn, "CLIPBOARD");
    let property = atom(&conn, "TEST_READ");
    let target = atom(&conn, target);
    conn.convert_selection(window, clipboard, target, property, CURRENT_TIME)
        .unwrap();
    conn.flush().unwrap();
    let until = Instant::now() + Duration::from_secs(2);
    let notify = loop {
        assert!(Instant::now() < until, "the clipboard owner did not answer");
        match conn.poll_for_event().unwrap() {
            Some(Event::SelectionNotify(n)) => break n,
            Some(_) => {}
            None => std::thread::sleep(Duration::from_millis(2)),
        }
    };
    if notify.property == NONE {
        return None;
    }
    let reply = conn
        .get_property(true, window, property, AtomEnum::ANY, 0, 1 << 20)
        .unwrap()
        .reply()
        .unwrap();
    let name = conn
        .get_atom_name(reply.type_)
        .unwrap()
        .reply()
        .unwrap()
        .name;
    Some((String::from_utf8(name).unwrap(), reply.format, reply.value))
}

fn snapshot(budget: usize) -> ClipboardSnapshot {
    X11Clip
        .snapshot(budget, Duration::from_secs(1))
        .unwrap()
        .expect("the private owner offers types")
}

fn bytes_of<'a>(snap: &'a ClipboardSnapshot, mime: &str) -> Option<&'a [u8]> {
    snap.offers
        .iter()
        .find(|(m, _)| m == mime)
        .map(|(_, b)| b.as_slice())
}

#[test]
fn a_repeated_reply_never_lands_in_the_next_target() {
    let Some(_display) = private_display() else {
        return;
    };
    let _owner = own(vec![
        ("x-test/a", Answer::Twice(b"AAAA".to_vec())),
        ("x-test/b", Answer::Plain("x-test/b", 8, b"BBBB".to_vec())),
    ]);
    let snap = snapshot(1 << 20);
    assert_eq!(bytes_of(&snap, "x-test/a"), Some(&b"AAAA"[..]));
    assert_eq!(bytes_of(&snap, "x-test/b"), Some(&b"BBBB"[..]));
}

#[test]
fn an_abandoned_incr_transfer_never_overwrites_the_next_target() {
    let Some(_display) = private_display() else {
        return;
    };
    let chunk = vec![b'x'; 600];
    let _owner = own(vec![
        (
            "x-test/big",
            Answer::Incr(vec![chunk; 5], b"LATE CHUNK".to_vec()),
        ),
        (
            "x-test/small",
            Answer::Plain("x-test/small", 8, b"SMALL".to_vec()),
        ),
    ]);
    let snap = snapshot(1000);
    assert_eq!(bytes_of(&snap, "x-test/big"), None, "over the budget");
    assert_eq!(bytes_of(&snap, "x-test/small"), Some(&b"SMALL"[..]));
}

#[test]
fn a_property_over_the_budget_is_left_out_and_the_rest_is_kept() {
    let Some(_display) = private_display() else {
        return;
    };
    let _owner = own(vec![
        (
            "x-test/huge",
            Answer::Plain("x-test/huge", 8, vec![b'h'; 64 * 1024]),
        ),
        (
            "x-test/small",
            Answer::Plain("x-test/small", 8, b"SMALL".to_vec()),
        ),
    ]);
    let snap = snapshot(1000);
    assert_eq!(bytes_of(&snap, "x-test/huge"), None, "over the budget");
    assert_eq!(bytes_of(&snap, "x-test/small"), Some(&b"SMALL"[..]));
}

#[test]
fn a_restore_replays_each_target_in_its_type_and_format() {
    let Some(_display) = private_display() else {
        return;
    };
    let list: Vec<u8> = [1u32, 2, 3].iter().flat_map(|v| v.to_ne_bytes()).collect();
    let owner = own(vec![
        ("x-test/list", Answer::Plain("INTEGER", 32, list.clone())),
        (
            "TEXT",
            Answer::Plain("UTF8_STRING", 8, "héllo".as_bytes().to_vec()),
        ),
    ]);
    let snap = snapshot(1 << 20);
    drop(owner);
    let generation = X11Clip.set_text("dictated").unwrap();
    assert_eq!(
        X11Clip.restore(snap, generation).unwrap(),
        Restored::Restored
    );
    assert_eq!(read("x-test/list"), Some(("INTEGER".into(), 32, list)));
    assert_eq!(
        read("TEXT"),
        Some(("UTF8_STRING".into(), 8, "héllo".as_bytes().to_vec()))
    );
}

#[test]
fn string_is_served_in_latin1_or_not_at_all() {
    let Some(_display) = private_display() else {
        return;
    };
    X11Clip.set_text("café").unwrap();
    assert_eq!(
        read("STRING"),
        Some(("STRING".into(), 8, b"caf\xe9".to_vec()))
    );
    assert_eq!(
        read("UTF8_STRING"),
        Some(("UTF8_STRING".into(), 8, "café".as_bytes().to_vec()))
    );
    assert_eq!(
        read("TEXT"),
        Some(("UTF8_STRING".into(), 8, "café".as_bytes().to_vec()))
    );
    X11Clip.set_text("naïve – done").unwrap();
    assert_eq!(read("STRING"), None, "an en dash has no Latin-1 form");
}

#[test]
fn a_restore_never_takes_back_a_copy_made_after_the_dictation() {
    let Some(_display) = private_display() else {
        return;
    };
    let (conn, window) = connect();
    let clipboard = atom(&conn, "CLIPBOARD");
    for round in 0..200 {
        let generation = X11Clip.set_text("dictated").unwrap();
        // The user copies: the selection is theirs from here on.
        conn.set_selection_owner(window, clipboard, CURRENT_TIME)
            .unwrap();
        let copied = conn
            .get_selection_owner(clipboard)
            .unwrap()
            .reply()
            .unwrap();
        assert_eq!(copied.owner, window);
        let restored = X11Clip
            .restore(ClipboardSnapshot::default(), generation)
            .unwrap();
        let now = conn
            .get_selection_owner(clipboard)
            .unwrap()
            .reply()
            .unwrap();
        assert_eq!(
            (restored, now.owner),
            (Restored::Superseded, window),
            "round {round}: the user's copy was taken back"
        );
    }
}
