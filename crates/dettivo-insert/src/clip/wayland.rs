//! The Wayland clipboard through data-control (`wl-clipboard-rs` takes
//! `ext_data_control_v1` when the compositor has it, else
//! `zwlr_data_control_v1`). What Dettivo offers is served from a thread
//! of its own until another client takes the clipboard; that thread's end
//! is how a restore learns something newer was copied.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use wl_clipboard_rs::copy::{self, MimeSource, Options, Source};
use wl_clipboard_rs::paste::{self, ClipboardType, Error, MimeType, Seat};

use super::{
    Clip, ClipError, ClipboardSnapshot, Generation, Restored, admit, is_meta, read::bounded,
};

/// The Wayland clipboard.
pub struct WaylandClip;

/// What Dettivo last set: its generation and whether it is still the
/// clipboard (false once another client took it).
struct Owned {
    generation: u64,
    live: Option<Arc<AtomicBool>>,
}

static OWNED: Mutex<Owned> = Mutex::new(Owned {
    generation: 0,
    live: None,
});

/// Serves `sources` until another client takes the clipboard; the flag
/// it returns says whether it still serves.
fn serve(sources: Vec<MimeSource>, omit_text_types: bool) -> Result<Arc<AtomicBool>, ClipError> {
    let mut options = Options::new();
    // The serving happens on our own thread, not in a forked child.
    options
        .foreground(true)
        .omit_additional_text_mime_types(omit_text_types);
    let prepared = options
        .prepare_copy_multi(sources)
        .map_err(|e| ClipError(format!("clipboard write: {e}")))?;
    let live = Arc::new(AtomicBool::new(true));
    let flag = live.clone();
    std::thread::Builder::new()
        .name("clipboard-serve".into())
        .spawn(move || {
            let _ = prepared.serve();
            flag.store(false, Ordering::Release);
        })
        .map_err(|e| ClipError(format!("clipboard thread: {e}")))?;
    Ok(live)
}

impl Clip for WaylandClip {
    fn snapshot(
        &self,
        budget: usize,
        deadline: Duration,
    ) -> Result<Option<ClipboardSnapshot>, ClipError> {
        let until = Instant::now() + deadline;
        let types = match paste::get_mime_types_ordered(ClipboardType::Regular, Seat::Unspecified) {
            Ok(types) => types,
            Err(Error::NoSeats | Error::ClipboardEmpty | Error::NoMimeType) => return Ok(None),
            Err(e) => return Err(ClipError(format!("clipboard types: {e}"))),
        };
        let mut snap = ClipboardSnapshot::default();
        let mut left = budget;
        for mime in types.into_iter().filter(|m| !is_meta(m)) {
            let read = paste::get_contents(
                ClipboardType::Regular,
                Seat::Unspecified,
                MimeType::Specific(&mime),
            )
            .map_err(|e| e.to_string())
            .and_then(|(pipe, _)| bounded(pipe, until, left));
            match read {
                Ok(bytes) => {
                    admit(&mut snap, &mut left, mime, bytes);
                }
                Err(reason) => {
                    tracing::warn!(mime = %mime, reason = %reason, "clipboard type left out of the snapshot")
                }
            }
        }
        if snap.offers.is_empty() {
            return Err(ClipError("no clipboard type could be read".into()));
        }
        Ok(Some(snap))
    }

    fn set_text(&self, text: &str) -> Result<Generation, ClipError> {
        let live = serve(
            vec![MimeSource {
                source: Source::Bytes(text.as_bytes().into()),
                mime_type: copy::MimeType::Text,
            }],
            false,
        )?;
        let mut owned = OWNED.lock().unwrap_or_else(|p| p.into_inner());
        owned.generation += 1;
        owned.live = Some(live);
        Ok(Generation(owned.generation))
    }

    fn restore(
        &self,
        snap: ClipboardSnapshot,
        if_generation: Generation,
    ) -> Result<Restored, ClipError> {
        let mut owned = OWNED.lock().unwrap_or_else(|p| p.into_inner());
        let ours = owned.generation == if_generation.0
            && owned
                .live
                .as_ref()
                .is_some_and(|l| l.load(Ordering::Acquire));
        if !ours {
            return Ok(Restored::Superseded);
        }
        owned.generation += 1;
        if snap.offers.is_empty() {
            owned.live = None;
            copy::clear(copy::ClipboardType::Regular, copy::Seat::All)
                .map_err(|e| ClipError(format!("clipboard clear: {e}")))?;
            return Ok(Restored::Restored);
        }
        let sources = snap
            .offers
            .into_iter()
            .map(|(mime, bytes)| MimeSource {
                source: Source::Bytes(bytes.into()),
                mime_type: copy::MimeType::Specific(mime),
            })
            .collect();
        owned.live = Some(serve(sources, true)?);
        Ok(Restored::Restored)
    }
}
