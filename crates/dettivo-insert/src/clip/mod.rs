//! The clipboard the `clipboard_paste` and `clipboard` backends use
//! (ADR 0091): a snapshot of every type the clipboard offers, the
//! dictated text set over it, and a restore of the snapshot that only
//! runs while the text Dettivo set is still the clipboard. Wayland goes
//! through data-control (`ext_data_control_v1`, else
//! `zwlr_data_control_v1`), X11 through the `CLIPBOARD` selection.

mod read;
mod wayland;
mod x11;

use std::time::Duration;

use crate::session::Session;

pub use wayland::WaylandClip;
pub use x11::X11Clip;

/// The most a snapshot keeps, over all its types.
pub const SNAPSHOT_BUDGET: usize = 32 * 1024 * 1024;

/// How long a snapshot may take to read every type.
pub const SNAPSHOT_DEADLINE: Duration = Duration::from_secs(1);

/// What the clipboard held: each offered type with its bytes, in the
/// order the owner offered them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClipboardSnapshot {
    /// `(mime type, bytes)` pairs.
    pub offers: Vec<(String, Vec<u8>)>,
}

/// Which content Dettivo last set: a restore runs only while the
/// clipboard still holds the generation it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Generation(pub u64);

/// What a restore did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restored {
    /// The snapshot is the clipboard again (an empty one cleared it).
    Restored,
    /// Something newer was copied since; the clipboard was left alone.
    Superseded,
}

/// Why a clipboard call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipError(pub String);

impl std::fmt::Display for ClipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A clipboard the backends can snapshot, set and restore.
pub trait Clip {
    /// Every type the clipboard offers, read within `deadline` and up to
    /// `budget` bytes in total; `None` when the clipboard is empty. A
    /// type that does not fit or does not answer in time is left out.
    fn snapshot(
        &self,
        budget: usize,
        deadline: Duration,
    ) -> Result<Option<ClipboardSnapshot>, ClipError>;
    /// Sets the clipboard to `text` and keeps serving it.
    fn set_text(&self, text: &str) -> Result<Generation, ClipError>;
    /// Offers `snap` again (an empty one clears the clipboard), unless
    /// the clipboard changed since `if_generation` was set.
    fn restore(
        &self,
        snap: ClipboardSnapshot,
        if_generation: Generation,
    ) -> Result<Restored, ClipError>;
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

/// The text types a dictation is offered as.
pub(crate) const TEXT_TYPES: &[&str] = &[
    "text/plain;charset=utf-8",
    "UTF8_STRING",
    "text/plain",
    "STRING",
    "TEXT",
];

/// X11 selection targets that describe the selection instead of holding
/// its content; a snapshot never reads them.
pub(crate) fn is_meta(mime: &str) -> bool {
    matches!(
        mime,
        "TARGETS" | "MULTIPLE" | "TIMESTAMP" | "SAVE_TARGETS" | "DELETE" | "INCR"
    )
}

/// Keeps `offer` when it fits in what is left of the budget.
pub(crate) fn admit(snap: &mut ClipboardSnapshot, left: &mut usize, mime: String, bytes: Vec<u8>) {
    if bytes.len() > *left {
        tracing::warn!(mime = %mime, bytes = bytes.len(), "clipboard type left out of the snapshot: over the budget");
        return;
    }
    *left -= bytes.len();
    snap.offers.push((mime, bytes));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_budget_admits_types_in_order_until_it_is_spent() {
        let mut snap = ClipboardSnapshot::default();
        let mut left = 10;
        admit(&mut snap, &mut left, "image/png".into(), vec![0; 8]);
        admit(&mut snap, &mut left, "image/jpeg".into(), vec![0; 8]);
        admit(&mut snap, &mut left, "text/plain".into(), vec![0; 2]);
        let kept: Vec<&str> = snap.offers.iter().map(|(m, _)| m.as_str()).collect();
        assert_eq!(kept, ["image/png", "text/plain"]);
        assert_eq!(left, 0);
        assert!(is_meta("TARGETS") && !is_meta("image/png"));
    }
}
