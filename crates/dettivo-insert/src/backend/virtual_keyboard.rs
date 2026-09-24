//! The in-process Wayland virtual keyboard (`zwp_virtual_keyboard_v1`,
//! ADR 0007): per insertion a generated keymap covering exactly the text
//! is uploaded and its keycodes are pressed and released with a small
//! delay; a text beyond the keycode budget goes in batches with a fresh
//! keymap each. Keystrokes (paste) use a keymap of named keys and
//! the modifier masks xkbcommon resolves from it.

use std::io::Write;
use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

use wayland_client::protocol::wl_keyboard::KeymapFormat;
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1, zwp_virtual_keyboard_v1,
};

use super::{Availability, Backend, Ctx, Failure, Kind, Performed};
use crate::keymap::{self, Batch, Key, Unmappable};
use crate::session::Session;
use crate::settings::PasteKeys;

/// Time the compositor gets to apply a new keymap before keys arrive.
const KEYMAP_SETTLE: Duration = Duration::from_millis(30);

#[derive(Default)]
struct Globals {
    seat: Option<wl_seat::WlSeat>,
    manager: Option<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for Globals {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_seat" if state.seat.is_none() => {
                    state.seat = Some(registry.bind(name, version.min(7), qh, ()));
                }
                "zwp_virtual_keyboard_manager_v1" => {
                    state.manager = Some(registry.bind(name, 1, qh, ()));
                }
                _ => {}
            }
        }
    }
}

delegate_noop!(Globals: ignore wl_seat::WlSeat);
delegate_noop!(Globals: ignore zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1);
delegate_noop!(Globals: ignore zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1);

/// One connection with one virtual keyboard.
pub struct Typist {
    conn: Connection,
    queue: EventQueue<Globals>,
    globals: Globals,
    keyboard: zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
    started: Instant,
    delay: Duration,
}

fn memfd(text: &str) -> Result<(OwnedFd, u32), String> {
    let name = c"dettivo-keymap";
    // SAFETY: memfd_create takes a NUL-terminated name and flags and returns
    // a new descriptor or -1; the descriptor is owned immediately.
    #[allow(unsafe_code)]
    let raw = unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC) };
    if raw < 0 {
        return Err(format!("memfd_create: {}", std::io::Error::last_os_error()));
    }
    // SAFETY: raw is a fresh descriptor this process owns.
    #[allow(unsafe_code)]
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    let mut file = std::fs::File::from(fd);
    file.write_all(text.as_bytes())
        .and_then(|()| file.write_all(b"\0"))
        .map_err(|e| format!("keymap write: {e}"))?;
    let size = text.len() as u32 + 1;
    Ok((OwnedFd::from(file), size))
}

impl Typist {
    /// Connects to the Wayland display and creates the virtual keyboard.
    pub fn connect(delay: Duration) -> Result<Self, String> {
        let conn = Connection::connect_to_env().map_err(|e| format!("wayland: {e}"))?;
        let mut queue = conn.new_event_queue();
        let qh = queue.handle();
        let _registry = conn.display().get_registry(&qh, ());
        let mut globals = Globals::default();
        queue
            .roundtrip(&mut globals)
            .map_err(|e| format!("wayland roundtrip: {e}"))?;
        let seat = globals
            .seat
            .clone()
            .ok_or_else(|| "the compositor advertises no wl_seat".to_string())?;
        let manager = globals.manager.clone().ok_or_else(|| {
            "the compositor does not implement zwp_virtual_keyboard_manager_v1".to_string()
        })?;
        let keyboard = manager.create_virtual_keyboard(&seat, &qh, ());
        Ok(Self {
            conn,
            queue,
            globals,
            keyboard,
            started: Instant::now(),
            delay,
        })
    }

    fn roundtrip(&mut self) -> Result<(), String> {
        self.queue
            .roundtrip(&mut self.globals)
            .map(|_| ())
            .map_err(|e| format!("wayland roundtrip: {e}"))
    }

    fn now(&self) -> u32 {
        self.started.elapsed().as_millis() as u32
    }

    /// Uploads a keymap and waits for the compositor to take it.
    pub fn upload(&mut self, batch: &Batch) -> Result<xkbcommon::xkb::Keymap, String> {
        let compiled = keymap::compile(&batch.text)?;
        let (fd, size) = memfd(&batch.text)?;
        self.keyboard
            .keymap(KeymapFormat::XkbV1.into(), fd.as_fd(), size);
        self.keyboard.modifiers(0, 0, 0, 0);
        self.roundtrip()?;
        std::thread::sleep(KEYMAP_SETTLE);
        Ok(compiled)
    }

    fn key(&mut self, key: &Key, pressed: bool) {
        self.keyboard
            .key(self.now(), key.evdev(), u32::from(pressed));
        let _ = self.conn.flush();
    }

    /// Presses and releases one key with the inter-key delay.
    pub fn tap(&mut self, key: &Key) {
        self.key(key, true);
        std::thread::sleep(self.delay);
        self.key(key, false);
        std::thread::sleep(self.delay);
    }

    /// Holds `modifiers` (named keys, with their xkb masks) around
    /// `taps`.
    pub fn combo(&mut self, modifiers: &[(&Key, u32)], taps: &[&Key], repeat: usize) {
        let mask = modifiers.iter().fold(0, |m, (_, bit)| m | bit);
        for (key, _) in modifiers {
            self.key(key, true);
        }
        self.keyboard.modifiers(mask, 0, 0, 0);
        let _ = self.conn.flush();
        std::thread::sleep(self.delay);
        for _ in 0..repeat {
            for key in taps {
                self.tap(key);
            }
        }
        for (key, _) in modifiers.iter().rev() {
            self.key(key, false);
        }
        self.keyboard.modifiers(0, 0, 0, 0);
        let _ = self.conn.flush();
        std::thread::sleep(self.delay);
    }

    /// Types every batch of `text`, running `ctx.recheck()` before each
    /// batch: a focus that moved before the first key is nothing
    /// delivered, one that moved between batches is a partial delivery
    /// naming the characters typed so far.
    pub fn type_text(&mut self, text: &str, ctx: &Ctx<'_>) -> Result<usize, Failure> {
        let batches =
            keymap::batches(text, keymap::BUDGET).map_err(|e| Failure::Before(e.to_string()))?;
        let mut typed = 0usize;
        let during = |e: String, typed: usize| {
            if typed == 0 {
                Failure::Before(e)
            } else {
                Failure::During(format!("{e} after {typed} characters"))
            }
        };
        for batch in &batches {
            if let Err(Failure::FocusMoved(why)) = ctx.recheck() {
                return Err(during(format!("focus moved: {why}"), typed).into_focus_moved());
            }
            self.upload(batch).map_err(|e| during(e, typed))?;
            for position in 0..batch.sequence.len() {
                let key = batch.key(position).clone();
                self.tap(&key);
                typed += 1;
            }
            self.roundtrip().map_err(|e| during(e, typed))?;
        }
        Ok(typed)
    }

    /// Finishes: a final roundtrip so every event reached the compositor.
    pub fn finish(mut self) -> Result<(), String> {
        self.roundtrip()?;
        self.keyboard.destroy();
        let _ = self.conn.flush();
        Ok(())
    }
}

/// The named keys every keystroke keymap carries, on their evdev codes.
const NAMED: &[(&str, u32)] = &[
    ("Control_L", 29),
    ("Shift_L", 42),
    ("v", 47),
    ("Insert", 110),
    ("Left", 105),
    ("Delete", 111),
];

fn named_keymap(typist: &mut Typist) -> Result<(Batch, u32, u32), String> {
    let batch = keymap::named(NAMED);
    let compiled = typist.upload(&batch)?;
    let mask = |name: &str| -> Result<u32, String> {
        let index = compiled.mod_get_index(name);
        if index == xkbcommon::xkb::MOD_INVALID {
            return Err(format!("keymap has no {name} modifier"));
        }
        Ok(1 << index)
    };
    Ok((batch, mask("Control")?, mask("Shift")?))
}

/// The backend.
pub struct VirtualKeyboard;

impl Backend for VirtualKeyboard {
    fn name(&self) -> &'static str {
        "virtual_keyboard"
    }

    fn kind(&self) -> Kind {
        Kind::Keystroke
    }

    fn availability(&self, session: &Session) -> Availability {
        if !session.is_wayland() {
            return Availability::Unavailable(
                "no Wayland display (WAYLAND_DISPLAY is unset)".into(),
            );
        }
        match Typist::connect(Duration::ZERO) {
            Ok(typist) => {
                let _ = typist.finish();
                Availability::Available
            }
            Err(reason) => Availability::Unavailable(reason),
        }
    }

    fn covers(&self, text: &str) -> Result<(), Unmappable> {
        keymap::batches(text, keymap::BUDGET).map(|_| ())
    }

    fn reaches_xwayland(&self) -> bool {
        false
    }

    fn insert(&self, text: &str, ctx: &Ctx<'_>) -> Result<Performed, Failure> {
        let mut typist = Typist::connect(ctx.key_delay()).map_err(Failure::Before)?;
        let typed = typist.type_text(text, ctx)?;
        typist
            .finish()
            .map_err(|e| Failure::During(format!("{e} after {typed} characters")))?;
        Ok(Performed::typed(typed))
    }

    fn paste_keystroke(&self, keys: PasteKeys, ctx: &Ctx<'_>) -> Result<(), String> {
        let mut typist = Typist::connect(ctx.key_delay().max(Duration::from_millis(5)))?;
        let (batch, control, shift) = named_keymap(&mut typist)?;
        let key = |name: &str| {
            batch
                .keys
                .iter()
                .find(|k| k.keysym == name)
                .expect("named key")
        };
        match keys {
            PasteKeys::CtrlV => typist.combo(&[(key("Control_L"), control)], &[key("v")], 1),
            PasteKeys::CtrlShiftV => typist.combo(
                &[(key("Control_L"), control), (key("Shift_L"), shift)],
                &[key("v")],
                1,
            ),
            PasteKeys::ShiftInsert => typist.combo(&[(key("Shift_L"), shift)], &[key("Insert")], 1),
        }
        typist.finish()
    }
}
