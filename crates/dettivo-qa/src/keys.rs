//! Real key presses for the hotkey drives: a virtual keyboard through
//! `/dev/uinput`, so the compositor sees the chord the way it sees the
//! user's keyboard and its own bindings fire. Writing to `/dev/uinput`
//! needs permission (the `input` group or a udev rule); the scenario
//! reports the lack of it as the reason for a skip.

use std::time::Duration;

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, EventType, InputEvent, KeyCode};

/// Whether this process may create a virtual keyboard.
pub fn available() -> Result<(), String> {
    let path = std::path::Path::new("/dev/uinput");
    if !path.exists() {
        return Err("/dev/uinput does not exist (modprobe uinput)".into());
    }
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map(|_| ())
        .map_err(|e| {
            format!("/dev/uinput is not writable ({e}); add a udev rule or the input group")
        })
}

/// A virtual keyboard with the keys a drive needs.
pub struct Keyboard {
    device: VirtualDevice,
}

impl Keyboard {
    /// Creates the device with `keys` and waits for the compositor to
    /// pick it up.
    pub fn new(keys: &[KeyCode]) -> Result<Self, String> {
        let mut set = AttributeSet::<KeyCode>::new();
        // The standard block (Escape to the keypad) makes udev and libinput
        // classify the device as a keyboard; without it the compositor
        // ignores the presses.
        for code in 1..=88u16 {
            set.insert(KeyCode(code));
        }
        for k in keys {
            set.insert(*k);
        }
        let device = VirtualDevice::builder()
            .map_err(|e| format!("uinput: {e}"))?
            .name("dettivo-qa-keyboard")
            .with_keys(&set)
            .map_err(|e| format!("uinput keys: {e}"))?
            .build()
            .map_err(|e| format!("uinput device: {e}"))?;
        // libinput and the compositor add the device asynchronously.
        std::thread::sleep(Duration::from_millis(1200));
        Ok(Self { device })
    }

    fn emit(&mut self, key: KeyCode, down: bool) -> Result<(), String> {
        let events = [
            InputEvent::new(EventType::KEY.0, key.0, i32::from(down)),
            InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0),
        ];
        self.device
            .emit(&events)
            .map_err(|e| format!("uinput emit: {e}"))
    }

    /// Presses a key (and holds it).
    pub fn press(&mut self, key: KeyCode) -> Result<(), String> {
        self.emit(key, true)?;
        std::thread::sleep(Duration::from_millis(40));
        Ok(())
    }

    /// Releases a key.
    pub fn release(&mut self, key: KeyCode) -> Result<(), String> {
        self.emit(key, false)?;
        std::thread::sleep(Duration::from_millis(40));
        Ok(())
    }

    /// Press and release.
    pub fn tap(&mut self, key: KeyCode) -> Result<(), String> {
        self.press(key)?;
        self.release(key)
    }

    /// Holds `modifiers`, presses `key` and keeps everything down.
    pub fn chord_down(&mut self, modifiers: &[KeyCode], key: KeyCode) -> Result<(), String> {
        for m in modifiers {
            self.press(*m)?;
        }
        self.press(key)
    }

    /// Releases `key`, then the modifiers in reverse order.
    pub fn chord_up(&mut self, modifiers: &[KeyCode], key: KeyCode) -> Result<(), String> {
        self.release(key)?;
        for m in modifiers.iter().rev() {
            self.release(*m)?;
        }
        Ok(())
    }

    /// A whole chord: down, then up.
    pub fn chord(&mut self, modifiers: &[KeyCode], key: KeyCode) -> Result<(), String> {
        self.chord_down(modifiers, key)?;
        self.chord_up(modifiers, key)
    }
}
