//! libei through the RemoteDesktop portal (GNOME, KDE): the portal hands
//! over an EI socket, the seat is bound for text or keyboard emulation,
//! and the text goes through `ei_text` (libei 1.6) or, on an older
//! server, through keycodes looked up in the server's own keymap. The
//! portal session is created on first use and kept for the process; a
//! portal that has no RemoteDesktop interface reports why.

use std::future::Future;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use futures_util::StreamExt;
use reis::event::{DeviceCapability, EiEvent};

use super::{Availability, Backend, Ctx, Failure, Kind, Performed};
use crate::session::Session;

/// How long the portal may take to answer (a permission dialog counts).
const PORTAL_TIMEOUT: Duration = Duration::from_secs(60);

/// The reason the portal gave when it refused, kept so the next call
/// reports it without asking again within the session.
static DENIED: OnceLock<Mutex<Option<String>>> = OnceLock::new();

/// Runs an async portal job on its own thread with its own runtime, so
/// the caller may sit inside the daemon's runtime or none at all.
fn on_own_runtime<T: Send, F: Future<Output = Result<T, String>>>(
    job: impl FnOnce() -> F + Send,
) -> Result<T, String> {
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("libei runtime: {e}"))?;
                runtime.block_on(job())
            })
            .join()
            .unwrap_or_else(|_| Err("libei thread panicked".to_string()))
    })
}

/// The portal's RemoteDesktop interface version, when it exists.
async fn portal_version() -> Result<u32, String> {
    let connection = zbus::Connection::session()
        .await
        .map_err(|e| format!("session bus: {e}"))?;
    let proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.RemoteDesktop",
    )
    .await
    .map_err(|e| format!("portal proxy: {e}"))?;
    proxy
        .get_property::<u32>("version")
        .await
        .map_err(|_| "the desktop portal has no RemoteDesktop interface".to_string())
}

async fn eis_socket() -> Result<UnixStream, String> {
    use ashpd::desktop::remote_desktop::{
        ConnectToEISOptions, DeviceType, RemoteDesktop, SelectDevicesOptions, StartOptions,
    };
    use ashpd::desktop::{CreateSessionOptions, PersistMode};
    let remote = RemoteDesktop::new()
        .await
        .map_err(|e| format!("RemoteDesktop portal: {e}"))?;
    let session = remote
        .create_session(CreateSessionOptions::default())
        .await
        .map_err(|e| format!("portal session: {e}"))?;
    remote
        .select_devices(
            &session,
            SelectDevicesOptions::default()
                .set_devices(enumflags2::BitFlags::from(DeviceType::Keyboard))
                .set_persist_mode(PersistMode::ExplicitlyRevoked),
        )
        .await
        .map_err(|e| format!("portal select_devices: {e}"))?;
    remote
        .start(&session, None, StartOptions::default())
        .await
        .map_err(|e| format!("portal start: {e}"))?
        .response()
        .map_err(|e| format!("portal denied the session: {e}"))?;
    let fd = remote
        .connect_to_eis(&session, ConnectToEISOptions::default())
        .await
        .map_err(|e| format!("portal ConnectToEIS: {e}"))?;
    Ok(UnixStream::from(fd))
}

/// Types `text` over the EI socket. `ctx.recheck()` runs once the seat's
/// device is up and right before the first key; `started` is set as the
/// first key goes out, so a caller that times out knows whether a prefix
/// may have reached the target.
async fn type_over_ei(
    stream: UnixStream,
    text: &str,
    ctx: &Ctx<'_>,
    started: &AtomicBool,
) -> Result<usize, Failure> {
    let context =
        reis::ei::Context::new(stream).map_err(|e| Failure::Before(format!("ei context: {e}")))?;
    let (connection, mut events) = context
        .handshake_tokio("dettivo", reis::ei::handshake::ContextType::Sender)
        .await
        .map_err(|e| Failure::Before(format!("ei handshake: {e}")))?;
    let mut typed = 0usize;
    let sequence = 1u32;
    let failed = |e: String, started: &AtomicBool| {
        if started.load(Ordering::SeqCst) {
            Failure::During(e)
        } else {
            Failure::Before(e)
        }
    };
    while let Some(event) = events.next().await {
        let event = event.map_err(|e| failed(format!("ei: {e}"), started))?;
        match event {
            EiEvent::SeatAdded(seat) => {
                seat.seat
                    .bind_capabilities(DeviceCapability::Keyboard | DeviceCapability::Text);
                let _ = connection.flush();
            }
            EiEvent::DeviceResumed(resumed) => {
                let device = resumed.device;
                let serial = connection.serial();
                if let Some(ei_text) = device.interface::<reis::ei::Text>() {
                    ctx.recheck()?;
                    device.device().start_emulating(serial, sequence);
                    started.store(true, Ordering::SeqCst);
                    ei_text.utf8(text);
                    device.device().frame(serial, 0);
                    device.device().stop_emulating(serial);
                    let _ = connection.flush();
                    typed = text.chars().count();
                    break;
                }
                if let (Some(keyboard), Some(keymap)) =
                    (device.interface::<reis::ei::Keyboard>(), device.keymap())
                {
                    let compiled = compiled_keymap(keymap).map_err(Failure::Before)?;
                    ctx.recheck()?;
                    device.device().start_emulating(serial, sequence);
                    for ch in text.chars() {
                        let (code, shift) = keycode_for(&compiled, ch).ok_or_else(|| {
                            failed(
                                format!("U+{:04X} is not on the server keymap", ch as u32),
                                started,
                            )
                        })?;
                        started.store(true, Ordering::SeqCst);
                        if let Some(shift) = shift {
                            keyboard.key(shift, reis::ei::keyboard::KeyState::Press);
                        }
                        keyboard.key(code, reis::ei::keyboard::KeyState::Press);
                        keyboard.key(code, reis::ei::keyboard::KeyState::Released);
                        if let Some(shift) = shift {
                            keyboard.key(shift, reis::ei::keyboard::KeyState::Released);
                        }
                        typed += 1;
                    }
                    device.device().frame(serial, 0);
                    device.device().stop_emulating(serial);
                    let _ = connection.flush();
                    break;
                }
            }
            EiEvent::Disconnected(d) => {
                return Err(failed(format!("ei disconnected: {:?}", d.reason), started));
            }
            _ => {}
        }
    }
    Ok(typed)
}

fn compiled_keymap(keymap: &reis::event::Keymap) -> Result<xkbcommon::xkb::Keymap, String> {
    use std::io::Read;
    let mut file = std::fs::File::from(
        keymap
            .fd
            .try_clone()
            .map_err(|e| format!("keymap fd: {e}"))?,
    );
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|e| format!("keymap read: {e}"))?;
    crate::keymap::compile(text.trim_end_matches('\0'))
}

/// The evdev code (and the Shift code when level 1 is needed) for `ch`
/// on a compiled keymap.
fn keycode_for(keymap: &xkbcommon::xkb::Keymap, ch: char) -> Option<(u32, Option<u32>)> {
    use xkbcommon::xkb;
    let keysym = xkb::Keysym::from_char(ch);
    let range = keymap.min_keycode().raw()..=keymap.max_keycode().raw();
    let shift = range.clone().find(|code| {
        keymap
            .key_get_syms_by_level(xkb::Keycode::new(*code), 0, 0)
            .contains(&xkb::Keysym::Shift_L)
    });
    for level in 0..=1u32 {
        for code in range.clone() {
            if keymap
                .key_get_syms_by_level(xkb::Keycode::new(code), 0, level)
                .contains(&keysym)
            {
                return Some((code - 8, (level == 1).then_some(shift? - 8)));
            }
        }
    }
    None
}

/// The backend.
pub struct Libei;

impl Backend for Libei {
    fn name(&self) -> &'static str {
        "libei"
    }

    fn kind(&self) -> Kind {
        Kind::Keystroke
    }

    fn availability(&self, session: &Session) -> Availability {
        if !session.has_session_bus {
            return Availability::Unavailable("no session bus for the desktop portal".into());
        }
        if let Some(reason) = DENIED
            .get_or_init(|| Mutex::new(None))
            .lock()
            .ok()
            .and_then(|d| d.clone())
        {
            return Availability::Unavailable(reason);
        }
        match on_own_runtime(|| async {
            tokio::time::timeout(Duration::from_secs(2), portal_version())
                .await
                .map_err(|_| "the desktop portal did not answer".to_string())?
        }) {
            Ok(version) if version >= 2 => Availability::Available,
            Ok(version) => Availability::Unavailable(format!(
                "RemoteDesktop portal version {version} has no ConnectToEIS (needs 2)"
            )),
            Err(reason) => Availability::Unavailable(reason),
        }
    }

    fn insert(&self, text: &str, ctx: &Ctx<'_>) -> Result<Performed, Failure> {
        // The portal handshake can take a while and the focus may move
        // meanwhile: the origin is checked again once the device is up,
        // right before the first key, inside `type_over_ei`.
        let started = AtomicBool::new(false);
        let result = on_own_runtime(|| async {
            Ok(tokio::time::timeout(PORTAL_TIMEOUT, async {
                let stream = eis_socket().await.map_err(Failure::Before)?;
                type_over_ei(stream, text, ctx, &started).await
            })
            .await
            .unwrap_or_else(|_| {
                let reason = "the portal session timed out".to_string();
                Err(if started.load(Ordering::SeqCst) {
                    Failure::During(reason)
                } else {
                    Failure::Before(reason)
                })
            }))
        });
        match result.unwrap_or_else(|runtime| Err(Failure::Before(runtime))) {
            Ok(typed) => Ok(Performed::typed(typed)),
            Err(failure) => {
                if failure.reason().contains("denied") {
                    if let Ok(mut d) = DENIED.get_or_init(|| Mutex::new(None)).lock() {
                        *d = Some(failure.reason().to_string());
                    }
                }
                Err(failure)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keycodes_come_from_the_server_keymap_with_shift_when_needed() {
        let batch = crate::keymap::named(&[("Shift_L", 42), ("a", 30)]);
        let text = batch.text.replace("{[a]}", "{[a, A]}");
        let keymap = crate::keymap::compile(&text).unwrap();
        assert_eq!(keycode_for(&keymap, 'a'), Some((30, None)));
        assert_eq!(keycode_for(&keymap, 'A'), Some((30, Some(42))));
        assert_eq!(keycode_for(&keymap, 'é'), None);
    }

    #[test]
    fn availability_needs_a_session_bus() {
        assert!(
            Libei
                .availability(&Session::default())
                .reason()
                .unwrap()
                .contains("session bus")
        );
    }
}
