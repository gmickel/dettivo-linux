//! The clipboard backends: `clipboard_paste` sets the clipboard, sends
//! the app-aware paste keystroke through the first keystroke-capable
//! backend and restores the previous clipboard once the target had time
//! to take the paste; `clipboard` sets it and stops there. Wayland goes
//! through `wl-clipboard-rs`, X11 through `x11-clipboard`.

use std::io::Read;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use super::{Availability, Backend, Ctx, Failure, Kind, Performed};
use crate::session::Session;

/// A clipboard the backends can read, write and restore.
pub trait Clip {
    /// The current text, `None` when the clipboard is empty or not text.
    fn read(&self) -> Result<Option<String>, String>;
    /// Sets the clipboard to `text` and keeps serving it.
    fn write(&self, text: &str) -> Result<(), String>;
    /// Clears the clipboard.
    fn clear(&self) -> Result<(), String>;
}

/// The Wayland clipboard through the `wlr-data-control` protocol.
pub struct WaylandClip;

impl Clip for WaylandClip {
    fn read(&self) -> Result<Option<String>, String> {
        use wl_clipboard_rs::paste::{ClipboardType, Error, MimeType, Seat, get_contents};
        match get_contents(ClipboardType::Regular, Seat::Unspecified, MimeType::Text) {
            Ok((mut reader, _)) => {
                let mut out = Vec::new();
                reader
                    .read_to_end(&mut out)
                    .map_err(|e| format!("clipboard read: {e}"))?;
                Ok(Some(String::from_utf8_lossy(&out).into_owned()))
            }
            Err(Error::NoSeats | Error::ClipboardEmpty | Error::NoMimeType) => Ok(None),
            Err(e) => Err(format!("clipboard read: {e}")),
        }
    }

    fn write(&self, text: &str) -> Result<(), String> {
        use wl_clipboard_rs::copy::{MimeType, Options, Source};
        // `prepare_copy` asserts the foreground option: the serving
        // happens on our own thread, not in a forked child.
        let mut options = Options::new();
        options.foreground(true);
        let prepared = options
            .prepare_copy(Source::Bytes(text.as_bytes().into()), MimeType::Text)
            .map_err(|e| format!("clipboard write: {e}"))?;
        // Serving stops on its own once another client takes the
        // clipboard; the thread lives exactly that long.
        std::thread::spawn(move || {
            let _ = prepared.serve();
        });
        Ok(())
    }

    fn clear(&self) -> Result<(), String> {
        use wl_clipboard_rs::copy::{ClipboardType, Seat, clear};
        clear(ClipboardType::Regular, Seat::All).map_err(|e| format!("clipboard clear: {e}"))
    }
}

/// The X11 `CLIPBOARD` selection. One owner connection per process:
/// the selection stays served for as long as the daemon runs.
pub struct X11Clip;

static X11_CLIPBOARD: OnceLock<Result<Arc<Mutex<x11_clipboard::Clipboard>>, String>> =
    OnceLock::new();

fn x11_clipboard() -> Result<Arc<Mutex<x11_clipboard::Clipboard>>, String> {
    X11_CLIPBOARD
        .get_or_init(|| {
            x11_clipboard::Clipboard::new()
                .map(|c| Arc::new(Mutex::new(c)))
                .map_err(|e| format!("x11 clipboard: {e}"))
        })
        .clone()
}

impl Clip for X11Clip {
    fn read(&self) -> Result<Option<String>, String> {
        let clipboard = x11_clipboard()?;
        let guard = clipboard.lock().unwrap_or_else(|p| p.into_inner());
        let atoms = &guard.getter.atoms;
        match guard.load(
            atoms.clipboard,
            atoms.utf8_string,
            atoms.property,
            Duration::from_millis(300),
        ) {
            Ok(bytes) if bytes.is_empty() => Ok(None),
            Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
            Err(x11_clipboard::error::Error::Timeout) => Ok(None),
            Err(e) => Err(format!("x11 clipboard read: {e}")),
        }
    }

    fn write(&self, text: &str) -> Result<(), String> {
        let clipboard = x11_clipboard()?;
        let guard = clipboard.lock().unwrap_or_else(|p| p.into_inner());
        let atoms = &guard.setter.atoms;
        guard
            .store(atoms.clipboard, atoms.utf8_string, text.as_bytes())
            .map_err(|e| format!("x11 clipboard write: {e}"))
    }

    fn clear(&self) -> Result<(), String> {
        self.write("")
    }
}

/// The clipboard for a session: Wayland first, then X11.
pub fn clip_for(session: &Session) -> Result<Box<dyn Clip>, String> {
    if session.is_wayland() {
        Ok(Box::new(WaylandClip))
    } else if session.is_x11() {
        Ok(Box::new(X11Clip))
    } else {
        Err("no Wayland or X11 display for a clipboard".into())
    }
}

fn availability(session: &Session) -> Availability {
    match clip_for(session) {
        Ok(_) => Availability::Available,
        Err(reason) => Availability::Unavailable(reason),
    }
}

/// Sets `text`, pastes through `ctx.keystroke`, then restores the previous
/// clipboard after the configured delay unless the user changed it
/// meanwhile. Shared by the two backends and tested with a fake clip.
/// The origin is checked right before the paste keystroke (a clipboard
/// that only copies types nothing, so it needs no check); a keystroke
/// that failed may have pasted, and a restore that failed after a paste
/// is a warning, never a reason to type the text again.
pub fn paste_with(
    clip: &dyn Clip,
    text: &str,
    ctx: &Ctx<'_>,
    keystroke: bool,
) -> Result<Performed, Failure> {
    let previous = clip.read().unwrap_or(None);
    if !keystroke {
        clip.write(text).map_err(Failure::Before)?;
        return Ok(Performed::copied());
    }
    let sender = ctx.keystroke.ok_or_else(|| {
        Failure::Before("no keystroke-capable backend for the paste keystroke".to_string())
    })?;
    let keys = ctx.settings.paste_keys_for(ctx.app_id);
    ctx.recheck()?;
    clip.write(text).map_err(Failure::Before)?;
    sender.paste_keystroke(keys, ctx).map_err(Failure::During)?;
    if ctx.settings.restore_clipboard {
        std::thread::sleep(Duration::from_millis(
            ctx.settings.clipboard_restore_delay_ms,
        ));
        let now = clip.read().unwrap_or(None);
        if now.as_deref() == Some(text) {
            let restored = match previous {
                Some(p) => clip.write(&p),
                None => clip.clear(),
            };
            if let Err(e) = restored {
                tracing::warn!(error = %e, "the pasted text stays on the clipboard: restore failed");
            }
        }
    }
    Ok(Performed {
        outcome: dettivo_proto::runtime::InsertionOutcome::Inserted,
        method: dettivo_proto::runtime::InsertionMethod::Paste,
        typed_chars: 0,
    })
}

/// Clipboard plus the app-aware paste keystroke.
pub struct ClipboardPaste;

impl Backend for ClipboardPaste {
    fn name(&self) -> &'static str {
        "clipboard_paste"
    }

    fn kind(&self) -> Kind {
        Kind::ClipboardPaste
    }

    fn availability(&self, session: &Session) -> Availability {
        availability(session)
    }

    fn insert(&self, text: &str, ctx: &Ctx<'_>) -> Result<Performed, Failure> {
        let clip = clip_for(ctx.session).map_err(Failure::Before)?;
        paste_with(clip.as_ref(), text, ctx, true)
    }
}

/// Clipboard only: the user pastes.
pub struct ClipboardOnly;

impl Backend for ClipboardOnly {
    fn name(&self) -> &'static str {
        "clipboard"
    }

    fn kind(&self) -> Kind {
        Kind::ClipboardOnly
    }

    fn availability(&self, session: &Session) -> Availability {
        availability(session)
    }

    fn insert(&self, text: &str, ctx: &Ctx<'_>) -> Result<Performed, Failure> {
        let clip = clip_for(ctx.session).map_err(Failure::Before)?;
        paste_with(clip.as_ref(), text, ctx, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{PasteKeys, Settings};
    use std::cell::RefCell;

    struct FakeClip {
        content: RefCell<Option<String>>,
        writes: RefCell<Vec<String>>,
    }

    impl Clip for FakeClip {
        fn read(&self) -> Result<Option<String>, String> {
            Ok(self.content.borrow().clone())
        }
        fn write(&self, text: &str) -> Result<(), String> {
            *self.content.borrow_mut() = Some(text.to_string());
            self.writes.borrow_mut().push(text.to_string());
            Ok(())
        }
        fn clear(&self) -> Result<(), String> {
            *self.content.borrow_mut() = None;
            self.writes.borrow_mut().push(String::new());
            Ok(())
        }
    }

    struct FakeKeys(std::sync::Mutex<Vec<PasteKeys>>);

    impl Backend for FakeKeys {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn kind(&self) -> Kind {
            Kind::Keystroke
        }
        fn availability(&self, _: &Session) -> Availability {
            Availability::Available
        }
        fn insert(&self, _: &str, _: &Ctx<'_>) -> Result<Performed, Failure> {
            unreachable!()
        }
        fn paste_keystroke(&self, keys: PasteKeys, _: &Ctx<'_>) -> Result<(), String> {
            self.0.lock().unwrap().push(keys);
            Ok(())
        }
    }

    fn ctx<'a>(
        session: &'a Session,
        settings: &'a Settings,
        keys: &'a FakeKeys,
        app: &'a str,
    ) -> Ctx<'a> {
        Ctx {
            session,
            settings,
            app_id: app,
            keystroke: Some(keys),
            recheck: None,
        }
    }

    #[test]
    fn paste_sends_the_terminal_keystroke_and_restores_the_clipboard() {
        let clip = FakeClip {
            content: RefCell::new(Some("before".into())),
            writes: RefCell::new(Vec::new()),
        };
        let keys = FakeKeys(std::sync::Mutex::new(Vec::new()));
        let session = Session::default();
        let settings = Settings {
            clipboard_restore_delay_ms: 0,
            ..Settings::default()
        };
        let done = paste_with(
            &clip,
            "hello",
            &ctx(&session, &settings, &keys, "foot"),
            true,
        )
        .unwrap();
        assert_eq!(
            done.outcome,
            dettivo_proto::runtime::InsertionOutcome::Inserted
        );
        assert_eq!(*keys.0.lock().unwrap(), vec![PasteKeys::CtrlShiftV]);
        assert_eq!(
            *clip.writes.borrow(),
            vec!["hello".to_string(), "before".to_string()]
        );
    }

    #[test]
    fn a_clipboard_the_user_changed_is_left_alone_and_empty_restores_to_empty() {
        let clip = FakeClip {
            content: RefCell::new(None),
            writes: RefCell::new(Vec::new()),
        };
        let keys = FakeKeys(std::sync::Mutex::new(Vec::new()));
        let session = Session::default();
        let settings = Settings {
            clipboard_restore_delay_ms: 0,
            ..Settings::default()
        };
        paste_with(
            &clip,
            "x",
            &ctx(&session, &settings, &keys, "firefox"),
            true,
        )
        .unwrap();
        assert_eq!(*keys.0.lock().unwrap(), vec![PasteKeys::CtrlV]);
        assert_eq!(*clip.content.borrow(), None, "cleared back to empty");
        *clip.content.borrow_mut() = Some("user".into());
        struct Changing<'a>(&'a FakeClip);
        impl Clip for Changing<'_> {
            fn read(&self) -> Result<Option<String>, String> {
                Ok(Some("user changed".into()))
            }
            fn write(&self, t: &str) -> Result<(), String> {
                self.0.write(t)
            }
            fn clear(&self) -> Result<(), String> {
                self.0.clear()
            }
        }
        let before = clip.writes.borrow().len();
        paste_with(
            &Changing(&clip),
            "y",
            &ctx(&session, &settings, &keys, "firefox"),
            true,
        )
        .unwrap();
        assert_eq!(clip.writes.borrow().len(), before + 1, "no restore write");
    }

    #[test]
    fn clipboard_only_never_sends_a_keystroke() {
        let clip = FakeClip {
            content: RefCell::new(None),
            writes: RefCell::new(Vec::new()),
        };
        let keys = FakeKeys(std::sync::Mutex::new(Vec::new()));
        let session = Session::default();
        let settings = Settings::default();
        let done = paste_with(&clip, "z", &ctx(&session, &settings, &keys, "foot"), false).unwrap();
        assert_eq!(
            done.method,
            dettivo_proto::runtime::InsertionMethod::ClipboardOnly
        );
        assert!(keys.0.lock().unwrap().is_empty());
        assert_eq!(*clip.content.borrow(), Some("z".into()));
    }
}
