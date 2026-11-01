//! The delivery step of a take: the text goes to the inserter, or, for a
//! stop that asked for the text alone (`dictation stop --print`),
//! nowhere, recorded as `printed`. The first key waits until
//! [`MODIFIER_SETTLE`] has passed since the stop (ADR 0090): the chord
//! that stopped the take may still be held, and a modifier held while
//! the backend types turns the text into shortcuts. No source reports
//! the held keys on every desktop, so the wait is fixed.

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use dettivo_proto::runtime::{InsertionMethod, InsertionOutcome, InsertionResult, TargetApp};

use crate::{Inserter, SessionTarget, no_context_pack};

/// How long after the stop the first key may be sent.
pub const MODIFIER_SETTLE: Duration = Duration::from_millis(250);

/// Where a take's text goes, decided once: a printing stop and the
/// worker's commitment to insert race for it, and the first one wins.
#[derive(Default)]
pub(crate) struct Route(AtomicU8);

const OPEN: u8 = 0;
const PRINT: u8 = 1;
const INSERT: u8 = 2;

impl Route {
    /// A printing stop asks for the text alone; false once the worker
    /// committed to inserting it.
    pub(crate) fn print(&self) -> bool {
        match self
            .0
            .compare_exchange(OPEN, PRINT, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => true,
            Err(now) => now == PRINT,
        }
    }

    /// The worker decides: true when a printing stop came first.
    pub(crate) fn printing(&self) -> bool {
        match self
            .0
            .compare_exchange(OPEN, INSERT, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => false,
            Err(now) => now == PRINT,
        }
    }
}

/// Hands `text` over, or records it as printed; `None` when there is no
/// text. `stopped` is when the worker saw the stop.
pub(crate) fn deliver(
    inserter: &dyn Inserter,
    text: &str,
    target: Option<&SessionTarget>,
    print: bool,
    stopped: Instant,
) -> Option<InsertionResult> {
    if text.is_empty() {
        return None;
    }
    if print {
        return Some(printed());
    }
    std::thread::sleep((stopped + MODIFIER_SETTLE).saturating_duration_since(Instant::now()));
    Some(inserter.insert(text, target))
}

/// The insertion a printed take records: nothing reached a window.
fn printed() -> InsertionResult {
    InsertionResult {
        outcome: InsertionOutcome::Printed,
        method: InsertionMethod::Printed,
        target_app: TargetApp {
            bundle_id: String::new(),
            name: String::new(),
        },
        context_pack: no_context_pack(),
        reason: None,
        backend: None,
    }
}
