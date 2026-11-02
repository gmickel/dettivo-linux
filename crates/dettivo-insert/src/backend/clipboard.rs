//! The clipboard backends: `clipboard_paste` snapshots the clipboard,
//! sets the text, sends the app-aware paste keystroke through the first
//! keystroke-capable backend and restores the snapshot once the target
//! had time to take the paste; `clipboard` sets the text and stops there.
//! The clipboard itself is in [`crate::clip`].

use std::time::Duration;

use super::{Availability, Backend, Ctx, Failure, Kind, Performed};
use crate::clip::{
    Clip, ClipboardSnapshot, Restored, SNAPSHOT_BUDGET, SNAPSHOT_DEADLINE, clip_for,
};
use crate::session::Session;

fn availability(session: &Session) -> Availability {
    match clip_for(session) {
        Ok(_) => Availability::Available,
        Err(reason) => Availability::Unavailable(reason),
    }
}

/// Sets `text`, pastes through `ctx.keystroke`, then restores what the
/// clipboard held before (every type it offered) after the configured
/// delay, unless something newer was copied meanwhile. Shared by the two
/// backends and tested with a fake clip. The origin is checked right
/// before the paste keystroke (a clipboard that only copies types
/// nothing, so it needs no check); a keystroke that failed may have
/// pasted, and a restore that failed after a paste is a warning, never a
/// reason to type the text again. A clipboard that could not be read is
/// not restored, so nothing is replaced by an empty one.
pub fn paste_with(
    clip: &dyn Clip,
    text: &str,
    ctx: &Ctx<'_>,
    keystroke: bool,
) -> Result<Performed, Failure> {
    if !keystroke {
        clip.set_text(text).map_err(|e| Failure::Before(e.0))?;
        return Ok(Performed::copied());
    }
    let sender = ctx.keystroke.ok_or_else(|| {
        Failure::Before("no keystroke-capable backend for the paste keystroke".to_string())
    })?;
    let keys = ctx.settings.paste_keys_for(ctx.app_id);
    let previous = if ctx.settings.restore_clipboard {
        match clip.snapshot(SNAPSHOT_BUDGET, SNAPSHOT_DEADLINE) {
            Ok(snap) => Some(snap.unwrap_or_default()),
            Err(e) => {
                tracing::warn!(error = %e, "the clipboard will not be restored: it could not be read");
                None
            }
        }
    } else {
        None
    };
    ctx.recheck()?;
    let generation = clip.set_text(text).map_err(|e| Failure::Before(e.0))?;
    sender.paste_keystroke(keys, ctx).map_err(Failure::During)?;
    if let Some(previous) = previous {
        std::thread::sleep(Duration::from_millis(
            ctx.settings.clipboard_restore_delay_ms,
        ));
        restore(clip, previous, generation);
    }
    Ok(Performed {
        outcome: dettivo_proto::runtime::InsertionOutcome::Inserted,
        method: dettivo_proto::runtime::InsertionMethod::Paste,
        typed_chars: 0,
    })
}

fn restore(clip: &dyn Clip, previous: ClipboardSnapshot, generation: crate::clip::Generation) {
    let types = previous.offers.len();
    match clip.restore(previous, generation) {
        Ok(Restored::Restored) => tracing::debug!(types, "clipboard restored"),
        Ok(Restored::Superseded) => {
            tracing::info!("clipboard not restored: something newer was copied")
        }
        Err(e) => {
            tracing::warn!(error = %e, "the pasted text stays on the clipboard: restore failed")
        }
    }
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
    use crate::clip::{ClipError, Generation};
    use crate::settings::{PasteKeys, Settings};
    use std::sync::{Arc, Mutex};

    type Offers = Vec<(String, Vec<u8>)>;

    /// A clipboard with generations: `copy` is another app copying.
    #[derive(Default)]
    struct FakeClip {
        state: Mutex<(Offers, u64)>,
        unreadable: bool,
    }

    impl FakeClip {
        fn holding(offers: &[(&str, &[u8])]) -> Arc<Self> {
            let clip = Arc::new(Self::default());
            clip.copy(offers);
            clip
        }
        fn copy(&self, offers: &[(&str, &[u8])]) {
            let mut s = self.state.lock().unwrap();
            s.0 = offers
                .iter()
                .map(|(m, b)| (m.to_string(), b.to_vec()))
                .collect();
            s.1 += 1;
        }
        fn offers(&self) -> Offers {
            self.state.lock().unwrap().0.clone()
        }
    }

    impl Clip for FakeClip {
        fn snapshot(&self, _: usize, _: Duration) -> Result<Option<ClipboardSnapshot>, ClipError> {
            if self.unreadable {
                return Err(ClipError("no clipboard type could be read".into()));
            }
            let offers = self.offers();
            Ok((!offers.is_empty()).then_some(ClipboardSnapshot {
                offers,
                ..Default::default()
            }))
        }
        fn set_text(&self, text: &str) -> Result<Generation, ClipError> {
            self.copy(&[("text/plain;charset=utf-8", text.as_bytes())]);
            Ok(Generation(self.state.lock().unwrap().1))
        }
        fn restore(&self, snap: ClipboardSnapshot, g: Generation) -> Result<Restored, ClipError> {
            let mut s = self.state.lock().unwrap();
            if s.1 != g.0 {
                return Ok(Restored::Superseded);
            }
            s.0 = snap.offers;
            s.1 += 1;
            Ok(Restored::Restored)
        }
    }

    /// Paste keys that record what they sent; `copy_meanwhile` makes
    /// another app copy right after the paste.
    struct FakeKeys {
        sent: Mutex<Vec<PasteKeys>>,
        copy_meanwhile: Option<Arc<FakeClip>>,
    }

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
            self.sent.lock().unwrap().push(keys);
            if let Some(clip) = &self.copy_meanwhile {
                clip.copy(&[("text/plain", b"copied by the user")]);
            }
            Ok(())
        }
    }

    fn paste(clip: &FakeClip, keys: &FakeKeys, app: &str, keystroke: bool) -> Performed {
        let session = Session::default();
        let settings = Settings {
            clipboard_restore_delay_ms: 0,
            ..Settings::default()
        };
        let ctx = Ctx {
            session: &session,
            settings: &settings,
            app_id: app,
            keystroke: Some(keys),
            recheck: None,
        };
        paste_with(clip, "hello", &ctx, keystroke).unwrap()
    }

    fn keys(copy_meanwhile: Option<Arc<FakeClip>>) -> FakeKeys {
        FakeKeys {
            sent: Mutex::new(Vec::new()),
            copy_meanwhile,
        }
    }

    /// fn-85 R1: an image (offered as two types) is the clipboard again
    /// after the paste, in every type, and the terminal got its keystroke.
    #[test]
    fn the_paste_restores_every_type_the_clipboard_offered() {
        let image: &[(&str, &[u8])] = &[("image/png", b"\x89PNG"), ("image/bmp", b"BM")];
        let clip = FakeClip::holding(image);
        let k = keys(None);
        let done = paste(&clip, &k, "foot", true);
        assert_eq!(
            done.outcome,
            dettivo_proto::runtime::InsertionOutcome::Inserted
        );
        assert_eq!(*k.sent.lock().unwrap(), vec![PasteKeys::CtrlShiftV]);
        let restored: Offers = image
            .iter()
            .map(|(m, b)| (m.to_string(), b.to_vec()))
            .collect();
        assert_eq!(clip.offers(), restored);
    }

    /// fn-85 R1: a copy made during the restore delay wins.
    #[test]
    fn a_copy_during_the_delay_wins_over_the_restore() {
        let clip = FakeClip::holding(&[("image/png", b"\x89PNG")]);
        let k = keys(Some(clip.clone()));
        paste(&clip, &k, "firefox", true);
        assert_eq!(
            clip.offers(),
            vec![("text/plain".to_string(), b"copied by the user".to_vec())]
        );
    }

    /// An empty clipboard is empty again; one that could not be read is
    /// not replaced, so the text stays rather than an empty clipboard.
    #[test]
    fn an_empty_clipboard_is_cleared_and_an_unreadable_one_is_left_alone() {
        let empty = Arc::new(FakeClip::default());
        paste(&empty, &keys(None), "firefox", true);
        assert!(empty.offers().is_empty());

        let unreadable = FakeClip {
            unreadable: true,
            ..FakeClip::default()
        };
        paste(&unreadable, &keys(None), "firefox", true);
        assert_eq!(
            unreadable.offers(),
            vec![("text/plain;charset=utf-8".to_string(), b"hello".to_vec())]
        );
    }

    #[test]
    fn clipboard_only_never_sends_a_keystroke_or_restores() {
        let clip = FakeClip::holding(&[("image/png", b"\x89PNG")]);
        let k = keys(None);
        let done = paste(&clip, &k, "foot", false);
        assert_eq!(
            done.method,
            dettivo_proto::runtime::InsertionMethod::ClipboardOnly
        );
        assert!(k.sent.lock().unwrap().is_empty());
        assert_eq!(
            clip.offers(),
            vec![("text/plain;charset=utf-8".to_string(), b"hello".to_vec())]
        );
    }
}
