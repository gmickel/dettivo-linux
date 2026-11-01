//! R3 against the mock portal on a private bus: the backend registers
//! its app id before the session (the mock refuses a session without
//! one), binds the four actions under ids that carry their chords,
//! `Activated` and `Deactivated` arrive as presses and releases, a rebind
//! leaves only the new ids, a portal without the registry or without
//! triggers is still bound, a denied bind is reported once and never
//! retried, and an absent portal is unavailable with its reason.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use dettivo_hotkeys::mock::{MockOptions, MockPortal, PrivateBus};
use dettivo_hotkeys::portal::{NO_PORTAL, PortalBackend, probe_at};
use dettivo_hotkeys::{Action, HotkeyBackend, HotkeyEvent, Identity, Keys};

fn bus() -> Option<PrivateBus> {
    match PrivateBus::start() {
        Ok(b) => Some(b),
        Err(e) => {
            eprintln!("skip: {e}");
            None
        }
    }
}

#[test]
fn signals_become_presses_and_releases_for_the_bound_actions() {
    let Some(bus) = bus() else { return };
    let portal = MockPortal::serve(bus.address(), false).unwrap();
    let backend = PortalBackend::at(Keys::defaults(), bus.address());
    assert!(backend.availability().is_available());
    let (tx, rx) = mpsc::channel();
    let registration = backend
        .start(Arc::new(move |e| {
            let _ = tx.send(e);
        }))
        .unwrap();
    assert_eq!(registration.backend, "portal");
    assert_eq!(registration.bound, Action::ALL.to_vec());
    assert_eq!(registration.identity, Some(Identity::Registered));
    assert!(!registration.compositor_bound);
    assert_eq!(portal.registered(), ["dettivo"]);
    assert_eq!(
        portal.bound(),
        [
            "dictation-hold-F9",
            "dictation-toggle-SUPER_CTRL_X",
            "dictation-cancel-SUPER_CTRL_ESCAPE",
            "dictation-reinsert-SUPER_CTRL_SHIFT_X",
        ]
    );
    portal.press("dictation-hold-F9").unwrap();
    portal.release("dictation-hold-F9").unwrap();
    portal.press("dictation-toggle-SUPER_CTRL_X").unwrap();
    portal.press("push_to_talk").unwrap();
    portal.press("dictation-cancel-SUPER_CTRL_ESCAPE").unwrap();
    let wait = Duration::from_secs(5);
    assert_eq!(
        rx.recv_timeout(wait).unwrap(),
        HotkeyEvent::Press(Action::PushToTalk)
    );
    assert_eq!(
        rx.recv_timeout(wait).unwrap(),
        HotkeyEvent::Release(Action::PushToTalk)
    );
    assert_eq!(
        rx.recv_timeout(wait).unwrap(),
        HotkeyEvent::Press(Action::Toggle)
    );
    assert_eq!(
        rx.recv_timeout(wait).unwrap(),
        HotkeyEvent::Press(Action::Cancel)
    );
    backend.stop();
    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
}

fn start(backend: &PortalBackend) -> (dettivo_hotkeys::Registration, mpsc::Receiver<HotkeyEvent>) {
    let (tx, rx) = mpsc::channel();
    let registration = backend
        .start(Arc::new(move |e| {
            let _ = tx.send(e);
        }))
        .unwrap();
    (registration, rx)
}

#[test]
fn a_rebind_registers_the_new_chord_under_a_new_id_and_drops_the_old() {
    let Some(bus) = bus() else { return };
    let portal = MockPortal::serve(bus.address(), false).unwrap();
    let first = PortalBackend::at(Keys::defaults(), bus.address());
    let _ = start(&first);
    assert!(portal.bound().contains(&"dictation-hold-F9".to_string()));
    first.stop();
    let keys = Keys::parse(
        "F10",
        Keys::DEFAULT_TOGGLE,
        Keys::DEFAULT_CANCEL,
        Keys::DEFAULT_REINSERT,
    )
    .unwrap();
    let second = PortalBackend::at(keys, bus.address());
    let (_, rx) = start(&second);
    let bound = portal.bound();
    assert!(
        bound.contains(&"dictation-hold-F10".to_string()),
        "{bound:?}"
    );
    assert!(!bound.iter().any(|id| id.ends_with("-F9")), "{bound:?}");
    portal.press("dictation-hold-F9").unwrap();
    portal.press("dictation-hold-F10").unwrap();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        HotkeyEvent::Press(Action::PushToTalk)
    );
    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
    second.stop();
}

#[test]
fn an_older_portal_and_hyprlands_empty_triggers_still_bind() {
    let cases = [
        (
            MockOptions {
                registry: false,
                ..MockOptions::default()
            },
            Identity::Unit,
            false,
        ),
        (
            MockOptions {
                empty_triggers: true,
                ..MockOptions::default()
            },
            Identity::Registered,
            true,
        ),
    ];
    for (options, identity, compositor_bound) in cases {
        let Some(bus) = bus() else { return };
        let _portal = MockPortal::serve_with(bus.address(), options).unwrap();
        let backend = PortalBackend::at(Keys::defaults(), bus.address());
        let (registration, _rx) = start(&backend);
        assert_eq!(registration.identity, Some(identity), "{options:?}");
        assert_eq!(
            registration.compositor_bound, compositor_bound,
            "{options:?}"
        );
        assert_eq!(registration.bound, Action::ALL.to_vec());
        backend.stop();
    }
}

#[test]
fn a_denied_bind_is_reported_once_and_not_retried() {
    let Some(bus) = bus() else { return };
    let portal = MockPortal::serve(bus.address(), true).unwrap();
    let backend = PortalBackend::at(Keys::defaults(), bus.address());
    let err = backend.start(Arc::new(|_| {})).unwrap_err();
    assert!(err.contains("denied"), "{err}");
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(portal.bind_calls(), 1);
    backend.stop();
}

#[test]
fn an_absent_portal_is_unavailable_with_its_reason() {
    let Some(bus) = bus() else { return };
    let availability = probe_at(Some(bus.address()));
    assert_eq!(availability.reason(), Some(NO_PORTAL), "{availability:?}");
    let backend = PortalBackend::at(Keys::defaults(), bus.address());
    let err = backend.start(Arc::new(|_| {})).unwrap_err();
    assert_eq!(err, NO_PORTAL);
    let nowhere = probe_at(Some("unix:path=/nonexistent/bus"));
    assert!(!nowhere.is_available());
}
