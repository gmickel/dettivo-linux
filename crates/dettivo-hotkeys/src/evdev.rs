//! The evdev backend (FR-H4): reads `/dev/input/event*` directly and
//! matches the chords, for a desktop with neither a compositor binding
//! nor a portal. Off by default; reading input devices needs the `input`
//! group, and every refusal names that requirement.

use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use evdev::{Device, EventSummary, KeyCode};

use crate::chord::Modifier;
use crate::{Action, Availability, Handler, HotkeyBackend, HotkeyEvent, Keys, Registration};

/// What a user has to do before the backend can read the keyboard.
pub const GROUP_REQUIREMENT: &str = "reading /dev/input/event* needs the `input` group: run `sudo usermod -aG input $USER` and log in again";

/// Matches chords against a stream of key events from one device: exact
/// modifier sets, press on the key going down, release on the same key
/// coming up. Autorepeat is ignored.
#[derive(Debug, Default)]
pub struct Matcher {
    held: Vec<Modifier>,
    active: Option<(Action, u16)>,
}

impl Matcher {
    /// Feeds one `EV_KEY` event (`code`, `value` 0 up, 1 down, 2 repeat).
    pub fn feed(&mut self, keys: &Keys, code: u16, value: i32) -> Option<HotkeyEvent> {
        if let Some(m) = Modifier::ALL
            .into_iter()
            .find(|m| m.evdev_codes().contains(&code))
        {
            match value {
                1 if !self.held.contains(&m) => self.held.push(m),
                0 => self.held.retain(|h| *h != m),
                _ => {}
            }
            return None;
        }
        match value {
            1 => {
                let action = Action::ALL.into_iter().find(|a| {
                    let chord = keys.chord(*a);
                    chord.key.evdev_code() == code && chord.modifiers_match(&self.held)
                })?;
                // While a chord is held only the cancel chord is heard,
                // and it leaves the held chord in place so its release
                // still arrives.
                if self.active.is_some() {
                    return (action == Action::Cancel).then_some(HotkeyEvent::Press(action));
                }
                self.active = Some((action, code));
                Some(HotkeyEvent::Press(action))
            }
            0 => match self.active {
                Some((action, key)) if key == code => {
                    self.active = None;
                    Some(HotkeyEvent::Release(action))
                }
                _ => None,
            },
            _ => None,
        }
    }
}

/// The backend.
pub struct EvdevBackend {
    keys: Keys,
    devices: Vec<PathBuf>,
    running: Mutex<Option<Running>>,
}

struct Running {
    stop: Arc<AtomicBool>,
    threads: Vec<std::thread::JoinHandle<()>>,
}

/// Every keyboard-like device under `/dev/input`.
fn keyboards() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir("/dev/input")
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("event"))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn is_keyboard(device: &Device) -> bool {
    device
        .supported_keys()
        .is_some_and(|keys| keys.contains(KeyCode::KEY_A) || keys.contains(KeyCode::KEY_F1))
}

/// Opens the configured devices (or every keyboard). `Err` names why
/// none could be opened, with the group requirement on a permission
/// refusal.
fn open_devices(configured: &[PathBuf]) -> Result<Vec<(PathBuf, Device)>, String> {
    let candidates = if configured.is_empty() {
        keyboards()
    } else {
        configured.to_vec()
    };
    if candidates.is_empty() {
        return Err("no input devices under /dev/input".into());
    }
    let mut opened = Vec::new();
    let mut denied = 0usize;
    let mut last_error = String::new();
    for path in &candidates {
        match Device::open(path) {
            Ok(d) if configured.is_empty() && !is_keyboard(&d) => {}
            Ok(d) => opened.push((path.clone(), d)),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    denied += 1;
                }
                last_error = format!("{}: {e}", path.display());
            }
        }
    }
    if opened.is_empty() {
        return Err(if denied > 0 {
            format!("cannot open {denied} input device(s): {GROUP_REQUIREMENT}")
        } else {
            format!("no keyboard could be opened ({last_error})")
        });
    }
    Ok(opened)
}

impl EvdevBackend {
    /// A backend over `devices` (empty: every keyboard) with `keys`.
    pub fn new(keys: Keys, devices: Vec<String>) -> Self {
        Self {
            keys,
            devices: devices.into_iter().map(PathBuf::from).collect(),
            running: Mutex::new(None),
        }
    }

    fn read_loop(
        path: &Path,
        mut device: Device,
        keys: Keys,
        handler: Handler,
        stop: Arc<AtomicBool>,
    ) {
        if let Err(e) = device.set_nonblocking(true) {
            tracing::warn!(device = %path.display(), error = %e, "evdev: cannot set non-blocking");
            return;
        }
        let mut matcher = Matcher::default();
        let fd = device.as_raw_fd();
        while !stop.load(Ordering::Relaxed) {
            let mut pfd = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: one valid pollfd for the device's own descriptor.
            #[allow(unsafe_code)]
            let ready = unsafe { libc::poll(&mut pfd, 1, POLL_INTERVAL.as_millis() as i32) };
            if ready <= 0 {
                continue;
            }
            let events = match device.fetch_events() {
                Ok(events) => events.collect::<Vec<_>>(),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => {
                    tracing::warn!(device = %path.display(), error = %e, "evdev: device gone");
                    return;
                }
            };
            for event in events {
                if let EventSummary::Key(_, code, value) = event.destructure() {
                    if let Some(hotkey) = matcher.feed(&keys, code.0, value) {
                        handler(hotkey);
                    }
                }
            }
        }
    }
}

impl HotkeyBackend for EvdevBackend {
    fn name(&self) -> &'static str {
        "evdev"
    }

    fn availability(&self) -> Availability {
        match open_devices(&self.devices) {
            Ok(_) => Availability::Available,
            Err(reason) => Availability::Unavailable(reason),
        }
    }

    fn start(&self, handler: Handler) -> Result<Registration, String> {
        let opened = open_devices(&self.devices)?;
        let stop = Arc::new(AtomicBool::new(false));
        let mut threads = Vec::new();
        for (path, device) in opened {
            let (keys, handler, stop) = (self.keys.clone(), handler.clone(), stop.clone());
            tracing::info!(device = %path.display(), "evdev: reading");
            threads.push(
                std::thread::Builder::new()
                    .name("hotkeys-evdev".into())
                    .spawn(move || Self::read_loop(&path, device, keys, handler, stop))
                    .map_err(|e| format!("evdev thread: {e}"))?,
            );
        }
        *self.running.lock().unwrap_or_else(|p| p.into_inner()) = Some(Running { stop, threads });
        Ok(Registration {
            backend: "evdev",
            bound: Action::ALL.to_vec(),
        })
    }

    fn stop(&self) {
        let running = self
            .running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        if let Some(r) = running {
            r.stop.store(true, Ordering::Relaxed);
            for t in r.threads {
                let _ = t.join();
            }
        }
    }
}

/// How often a reader checks the stop flag between events.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presses_and_releases_follow_the_chord_and_ignore_repeats() {
        let keys = Keys::defaults();
        let mut m = Matcher::default();
        // F9 alone: hold to talk.
        assert_eq!(
            m.feed(&keys, 67, 1),
            Some(HotkeyEvent::Press(Action::PushToTalk))
        );
        assert_eq!(m.feed(&keys, 67, 2), None);
        assert_eq!(
            m.feed(&keys, 67, 0),
            Some(HotkeyEvent::Release(Action::PushToTalk))
        );
        // Super+Ctrl+X toggles; Super+Ctrl+Shift+X re-inserts.
        assert_eq!(m.feed(&keys, 125, 1), None);
        assert_eq!(m.feed(&keys, 29, 1), None);
        assert_eq!(
            m.feed(&keys, 45, 1),
            Some(HotkeyEvent::Press(Action::Toggle))
        );
        assert_eq!(
            m.feed(&keys, 45, 0),
            Some(HotkeyEvent::Release(Action::Toggle))
        );
        assert_eq!(m.feed(&keys, 54, 1), None);
        assert_eq!(
            m.feed(&keys, 45, 1),
            Some(HotkeyEvent::Press(Action::ReinsertLast))
        );
        assert_eq!(
            m.feed(&keys, 45, 0),
            Some(HotkeyEvent::Release(Action::ReinsertLast))
        );
        // Extra modifiers or the wrong key match nothing.
        assert_eq!(m.feed(&keys, 67, 1), None);
        assert_eq!(m.feed(&keys, 67, 0), None);
        for code in [54, 29, 125] {
            assert_eq!(m.feed(&keys, code, 0), None);
        }
        assert_eq!(m.feed(&keys, 30, 1), None);
    }

    #[test]
    fn a_second_chord_while_one_is_held_is_ignored() {
        let keys = Keys::defaults();
        let mut m = Matcher::default();
        assert_eq!(
            m.feed(&keys, 67, 1),
            Some(HotkeyEvent::Press(Action::PushToTalk))
        );
        assert_eq!(m.feed(&keys, 125, 1), None);
        assert_eq!(m.feed(&keys, 29, 1), None);
        assert_eq!(m.feed(&keys, 45, 1), None);
        assert_eq!(m.feed(&keys, 45, 0), None);
        assert_eq!(
            m.feed(&keys, 67, 0),
            Some(HotkeyEvent::Release(Action::PushToTalk))
        );
    }

    /// dictation/F8: Super+Ctrl+Escape while F9 is held cancels, and the
    /// F9 release still arrives afterwards.
    #[test]
    fn the_cancel_chord_is_heard_while_push_to_talk_is_held() {
        let keys = Keys::defaults();
        let mut m = Matcher::default();
        assert_eq!(
            m.feed(&keys, 67, 1),
            Some(HotkeyEvent::Press(Action::PushToTalk))
        );
        assert_eq!(m.feed(&keys, 125, 1), None);
        assert_eq!(m.feed(&keys, 29, 1), None);
        assert_eq!(
            m.feed(&keys, 1, 1),
            Some(HotkeyEvent::Press(Action::Cancel))
        );
        assert_eq!(m.feed(&keys, 1, 0), None, "the held chord owns the release");
        assert_eq!(m.feed(&keys, 29, 0), None);
        assert_eq!(m.feed(&keys, 125, 0), None);
        assert_eq!(
            m.feed(&keys, 67, 0),
            Some(HotkeyEvent::Release(Action::PushToTalk))
        );
    }

    #[test]
    fn unreadable_devices_name_the_input_group_and_missing_ones_are_named() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("event99");
        let err = open_devices(std::slice::from_ref(&missing)).unwrap_err();
        assert!(err.contains("event99"), "{err}");
        let backend = EvdevBackend::new(Keys::defaults(), vec![missing.display().to_string()]);
        assert!(!backend.availability().is_available());
        // An unreadable device (permission denied) carries the requirement.
        let denied = dir.path().join("event0");
        std::fs::write(&denied, b"").unwrap();
        std::fs::set_permissions(&denied, std::os::unix::fs::PermissionsExt::from_mode(0o000))
            .unwrap();
        if std::fs::File::open(&denied).is_err() {
            let err = open_devices(&[denied]).unwrap_err();
            assert!(err.contains("input"), "{err}");
            assert!(err.contains("usermod"), "{err}");
        }
    }
}
