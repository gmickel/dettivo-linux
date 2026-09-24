//! The delivery step of one insertion: the chosen backends in order, each
//! handed the origin check, and the rule for a failure. The next backend
//! runs only when the one before it delivered nothing; a backend that
//! may have typed a prefix ends the insertion as a partial delivery, and
//! a focus that moved before the first key ends it as a changed target.
//! Pure over the backend trait, so the rule has unit tests.

use std::time::Instant;

use dettivo_proto::methods::insert::Target;
use dettivo_proto::runtime::{InsertionBackend, InsertionMethod, InsertionOutcome};

use crate::backend::{Ctx, Failure, Kind};
use crate::chain::Candidate;

/// The identity of the window the chain decided on, compared again right
/// before a backend delivers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Origin {
    app_id: String,
    pid: Option<u32>,
    window: Option<String>,
}

impl Origin {
    /// The origin of a probe's answer; `None` when nothing was focused.
    pub(crate) fn of(target: Option<&Target>, window: Option<&str>) -> Option<Self> {
        target.map(|t| Self {
            app_id: t.app_id.clone(),
            pid: t.pid,
            window: window.map(str::to_string),
        })
    }
}

/// Whether the focus left the origin: `Err` names where it went, with
/// nothing that could identify a document.
pub(crate) fn moved(then: Option<&Origin>, now: Option<&Origin>) -> Result<(), String> {
    match (then, now) {
        (None, None) => Ok(()),
        (Some(a), Some(b)) if a == b => Ok(()),
        (Some(_), Some(b)) => Err(format!("focus moved to {}", b.app_id)),
        (Some(_), None) => Err("focus left the window".to_string()),
        (None, Some(b)) => Err(format!("focus arrived at {}", b.app_id)),
    }
}

/// What the delivery step concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Delivered {
    /// How it concluded.
    pub outcome: InsertionOutcome,
    /// The contract method name.
    pub method: InsertionMethod,
    /// Why, when it did not insert.
    pub reason: Option<String>,
    /// The backend that performed, with its cost.
    pub backend: Option<InsertionBackend>,
    /// The record `undo` keeps: the backend's name and the characters it
    /// typed, when it typed any.
    pub typed: Option<(&'static str, usize)>,
}

/// Runs the chosen backends over `text` in `order`.
pub(crate) fn deliver(
    candidates: &[Candidate<'_>],
    order: &[String],
    text: &str,
    clipboard_only_requested: bool,
    ctx: &Ctx<'_>,
) -> Delivered {
    let mut last_error = String::new();
    for name in order {
        let backend = candidates
            .iter()
            .find(|c| c.backend.name() == name)
            .map(|c| c.backend)
            .expect("chosen backend exists");
        let started = Instant::now();
        match backend.insert(text, ctx) {
            Ok(performed) => {
                let latency_ms = started.elapsed().as_millis() as u64;
                tracing::info!(backend = %name, chars = text.chars().count(), latency_ms, outcome = ?performed.outcome, "insertion done");
                let method = if performed.outcome == InsertionOutcome::CopiedToClipboard
                    && backend.kind() == Kind::ClipboardOnly
                    && !clipboard_only_requested
                {
                    InsertionMethod::FallbackCopy
                } else {
                    performed.method
                };
                let undo_supported = backend.undo_supported() && performed.typed_chars > 0;
                return Delivered {
                    outcome: performed.outcome,
                    method,
                    reason: None,
                    backend: Some(InsertionBackend {
                        name: backend.name().to_string(),
                        latency_ms,
                        undo_supported,
                    }),
                    typed: Some((backend.name(), performed.typed_chars)),
                };
            }
            Err(Failure::Before(e)) => {
                tracing::warn!(backend = %name, error = %e, "backend failed before delivering, trying the next");
                last_error = format!("{name}: {e}");
            }
            Err(Failure::During(e)) => {
                tracing::warn!(backend = %name, error = %e, "backend failed while delivering; the text is not typed again");
                return failed(format!("partial_delivery: {name}: {e}"));
            }
            Err(Failure::FocusMoved(e)) => {
                tracing::info!(backend = %name, reason = %e, "insertion refused: the focus moved before delivery");
                return failed(format!("target_changed: {e}"));
            }
        }
    }
    failed(last_error)
}

fn failed(reason: String) -> Delivered {
    Delivered {
        outcome: InsertionOutcome::Failed,
        method: InsertionMethod::FallbackCopy,
        reason: Some(reason),
        backend: None,
        typed: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use crate::backend::{Availability, Backend, Performed};
    use crate::session::Session;
    use crate::settings::Settings;

    /// A backend whose `insert` answers from a script and records the
    /// texts it was handed.
    struct Scripted {
        name: &'static str,
        answer: Result<Performed, Failure>,
        handed: Mutex<Vec<String>>,
    }

    impl Backend for Scripted {
        fn name(&self) -> &'static str {
            self.name
        }
        fn kind(&self) -> Kind {
            Kind::Keystroke
        }
        fn availability(&self, _: &Session) -> Availability {
            Availability::Available
        }
        fn insert(&self, text: &str, ctx: &Ctx<'_>) -> Result<Performed, Failure> {
            ctx.recheck()?;
            self.handed.lock().unwrap().push(text.to_string());
            self.answer.clone()
        }
    }

    fn scripted(name: &'static str, answer: Result<Performed, Failure>) -> Scripted {
        Scripted {
            name,
            answer,
            handed: Mutex::new(Vec::new()),
        }
    }

    fn run(
        first: &Scripted,
        second: &Scripted,
        recheck: Option<crate::backend::Recheck<'_>>,
    ) -> Delivered {
        let session = Session::default();
        let settings = Settings::default();
        let ctx = Ctx {
            session: &session,
            settings: &settings,
            app_id: "foot",
            keystroke: None,
            recheck,
        };
        let candidates = [
            Candidate {
                backend: first,
                availability: Availability::Available,
            },
            Candidate {
                backend: second,
                availability: Availability::Available,
            },
        ];
        deliver(
            &candidates,
            &["first".to_string(), "second".to_string()],
            "hello",
            false,
            &ctx,
        )
    }

    /// A backend that stopped before delivering hands the text to the
    /// next one; one that may have typed a prefix ends the insertion as
    /// a partial delivery and the next backend never sees the text.
    #[test]
    fn the_next_backend_runs_only_when_nothing_was_delivered() {
        let first = scripted("first", Err(Failure::Before("no seat".into())));
        let second = scripted("second", Ok(Performed::typed(5)));
        let done = run(&first, &second, None);
        assert_eq!(done.outcome, InsertionOutcome::Inserted);
        assert_eq!(
            done.backend.as_ref().map(|b| b.name.as_str()),
            Some("second")
        );
        assert_eq!(done.typed, Some(("second", 5)));
        assert_eq!(second.handed.lock().unwrap().len(), 1);

        let first = scripted(
            "first",
            Err(Failure::During("xdotool did not finish within 20s".into())),
        );
        let second = scripted("second", Ok(Performed::typed(5)));
        let done = run(&first, &second, None);
        assert_eq!(done.outcome, InsertionOutcome::Failed);
        assert_eq!(
            done.reason.as_deref(),
            Some("partial_delivery: first: xdotool did not finish within 20s")
        );
        assert!(done.backend.is_none() && done.typed.is_none());
        assert!(
            second.handed.lock().unwrap().is_empty(),
            "the text was never typed again"
        );
    }

    /// A focus that moved before the first key ends the insertion with the
    /// target named as changed, and no later backend types into the new
    /// window.
    #[test]
    fn a_focus_that_moved_before_delivery_refuses_every_backend() {
        let first = scripted("first", Ok(Performed::typed(5)));
        let second = scripted("second", Ok(Performed::typed(5)));
        let check = || Err("focus moved to kitty".to_string());
        let done = run(&first, &second, Some(&check));
        assert_eq!(done.outcome, InsertionOutcome::Failed);
        assert_eq!(
            done.reason.as_deref(),
            Some("target_changed: focus moved to kitty")
        );
        assert!(first.handed.lock().unwrap().is_empty());
        assert!(second.handed.lock().unwrap().is_empty());
    }

    #[test]
    fn the_origin_compares_the_window_not_only_the_process() {
        let a = Origin {
            app_id: "foot".into(),
            pid: Some(42),
            window: Some("0x1".into()),
        };
        let b = Origin {
            window: Some("0x2".into()),
            ..a.clone()
        };
        assert_eq!(moved(Some(&a), Some(&a)), Ok(()));
        assert_eq!(moved(None, None), Ok(()));
        assert_eq!(moved(Some(&a), Some(&b)), Err("focus moved to foot".into()));
        assert_eq!(moved(Some(&a), None), Err("focus left the window".into()));
        assert_eq!(moved(None, Some(&a)), Err("focus arrived at foot".into()));
    }
}
