//! R3 against the mock portal on a private bus: the backend binds the
//! four actions, `Activated` and `Deactivated` arrive as presses and
//! releases, a denied bind is reported once and never retried, and an
//! absent portal is unavailable with its reason.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use dettivo_hotkeys::mock::{MockPortal, PrivateBus};
use dettivo_hotkeys::portal::{NO_PORTAL, PortalBackend, probe_at};
use dettivo_hotkeys::{Action, HotkeyBackend, HotkeyEvent, Keys};

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
    assert_eq!(
        portal.bound(),
        Action::ALL
            .iter()
            .map(|a| a.id().to_string())
            .collect::<Vec<_>>()
    );
    portal.press("push_to_talk").unwrap();
    portal.release("push_to_talk").unwrap();
    portal.press("toggle").unwrap();
    portal.press("unknown_id").unwrap();
    portal.press("cancel").unwrap();
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
