//! The delivery step of a take: the text goes to the inserter, or, for a
//! stop that asked for the text alone (`dictation stop --print`),
//! nowhere, recorded as `printed`. The first key waits until
//! [`MODIFIER_SETTLE`] has passed since the stop (ADR 0090): the chord
//! that stopped the take may still be held, and a modifier held while
//! the backend types turns the text into shortcuts. No source reports
//! the held keys on every desktop, so the wait is fixed.

use std::time::{Duration, Instant};

use dettivo_proto::runtime::{InsertionMethod, InsertionOutcome, InsertionResult, TargetApp};

use crate::{Inserter, SessionTarget, no_context_pack};

/// How long after the stop the first key may be sent.
pub const MODIFIER_SETTLE: Duration = Duration::from_millis(250);

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
