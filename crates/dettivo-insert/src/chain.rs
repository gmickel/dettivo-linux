//! Chain selection and the target guards (FR-I1, FR-I4, FR-I8): the
//! backends in order, a pin from `[insert] backend`, demotion of a
//! keystroke backend that cannot cover the text, and the rule that an
//! unverifiable target permits clipboard-only and never a paste. Pure
//! decisions over availability answers, so every rule has a unit test.

use dettivo_proto::capabilities::InsertMode;
use dettivo_proto::methods::insert::{BackendStatus, Target};

pub use crate::guards::{Guards, check_guards};

use crate::backend::{Availability, Backend, Kind};
use crate::settings::Settings;

/// Why the chain refuses before any backend runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A guard mismatched the focused window: `CONFLICT`, nothing typed.
    Conflict(String),
    /// The focused window is Dettivo's own: outcome `failed`,
    /// reason `target_is_self`.
    TargetIsSelf,
    /// No backend can run; the reason names the first blocker.
    NothingAvailable(String),
}

/// One backend with its availability, as the chain sees it.
pub struct Candidate<'a> {
    /// The backend.
    pub backend: &'a dyn Backend,
    /// Its availability in this session.
    pub availability: Availability,
}

impl Candidate<'_> {
    /// The `insert.target` row.
    pub fn status(&self) -> BackendStatus {
        BackendStatus {
            name: self.backend.name().to_string(),
            available: self.availability.is_available(),
            reason: self.availability.reason().map(str::to_string),
        }
    }
}

/// What the chain decided for one insertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// Backend names to try, in order.
    pub order: Vec<String>,
    /// Backends skipped with the reason, for the log.
    pub skipped: Vec<(String, String)>,
}

/// The facts about the target the selection depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TargetFacts {
    /// A probe saw the target; without it only clipboard-only may run.
    pub verified: bool,
    /// The target is an X client under XWayland.
    pub xwayland: bool,
}

impl TargetFacts {
    /// The facts for a probed target.
    pub fn of(target: Option<&Target>) -> Self {
        Self {
            verified: target.is_some(),
            xwayland: target.is_some_and(|t| t.xwayland),
        }
    }
}

/// Which backends may deliver `text` in `mode` to a target with `facts`.
pub fn choose(
    candidates: &[Candidate<'_>],
    settings: &Settings,
    text: &str,
    mode: InsertMode,
    facts: TargetFacts,
) -> Result<Choice, Refusal> {
    let mut order = Vec::new();
    let mut skipped = Vec::new();
    let pin = settings.pin();
    for candidate in candidates {
        let name = candidate.backend.name().to_string();
        if let Some(pin) = pin {
            if pin != name {
                continue;
            }
        }
        if let Some(reason) = candidate.availability.reason() {
            skipped.push((name, reason.to_string()));
            continue;
        }
        let kind = candidate.backend.kind();
        if mode == InsertMode::ClipboardOnly && kind != Kind::ClipboardOnly {
            skipped.push((name, "clipboard_only mode".into()));
            continue;
        }
        if !facts.verified && kind != Kind::ClipboardOnly {
            skipped.push((name, "target cannot be verified; clipboard only".into()));
            continue;
        }
        if facts.xwayland && !candidate.backend.reaches_xwayland() {
            skipped.push((
                name,
                "XWayland target: its keymap does not reach X clients".into(),
            ));
            continue;
        }
        if let Err(unmappable) = candidate.backend.covers(text) {
            skipped.push((name, format!("demoted: {unmappable}")));
            continue;
        }
        order.push(name);
    }
    if order.is_empty() {
        let reason = match (pin, skipped.first()) {
            (Some(pin), Some((_, reason))) if skipped.len() == 1 => {
                format!("pinned backend {pin} is unavailable: {reason}")
            }
            (Some(pin), _) if skipped.is_empty() => {
                format!("pinned backend {pin} is not a backend name")
            }
            (_, Some((name, reason))) => format!("no backend available; {name}: {reason}"),
            (_, None) => "no backend in the chain".to_string(),
        };
        return Err(Refusal::NothingAvailable(reason));
    }
    Ok(Choice { order, skipped })
}

/// The first available keystroke-capable backend that reaches the
/// target, for the paste keystroke of the clipboard path.
pub fn keystroke_sender<'a>(
    candidates: &'a [Candidate<'a>],
    facts: TargetFacts,
) -> Option<&'a dyn Backend> {
    candidates
        .iter()
        .find(|c| {
            c.availability.is_available()
                && c.backend.kind() == Kind::Keystroke
                && (!facts.xwayland || c.backend.reaches_xwayland())
        })
        .map(|c| c.backend)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Ctx, Failure, Performed};
    use crate::keymap::Unmappable;
    use crate::session::Session;

    struct Fake {
        name: &'static str,
        kind: Kind,
        available: bool,
        covers: bool,
    }

    const VERIFIED: TargetFacts = TargetFacts {
        verified: true,
        xwayland: false,
    };

    impl Backend for Fake {
        fn name(&self) -> &'static str {
            self.name
        }
        fn kind(&self) -> Kind {
            self.kind
        }
        fn availability(&self, _: &Session) -> Availability {
            if self.available {
                Availability::Available
            } else {
                Availability::Unavailable(format!("{} is off", self.name))
            }
        }
        fn covers(&self, _: &str) -> Result<(), Unmappable> {
            if self.covers {
                Ok(())
            } else {
                Err(Unmappable {
                    index: 0,
                    ch: '\u{7}',
                })
            }
        }
        fn reaches_xwayland(&self) -> bool {
            self.name != "virtual_keyboard"
        }
        fn insert(&self, _: &str, _: &Ctx<'_>) -> Result<Performed, Failure> {
            unreachable!()
        }
    }

    fn fakes() -> Vec<Fake> {
        vec![
            Fake {
                name: "virtual_keyboard",
                kind: Kind::Keystroke,
                available: true,
                covers: true,
            },
            Fake {
                name: "libei",
                kind: Kind::Keystroke,
                available: false,
                covers: true,
            },
            Fake {
                name: "ydotool",
                kind: Kind::Keystroke,
                available: false,
                covers: true,
            },
            Fake {
                name: "xdotool",
                kind: Kind::Keystroke,
                available: true,
                covers: true,
            },
            Fake {
                name: "clipboard_paste",
                kind: Kind::ClipboardPaste,
                available: true,
                covers: true,
            },
            Fake {
                name: "clipboard",
                kind: Kind::ClipboardOnly,
                available: true,
                covers: true,
            },
        ]
    }

    fn candidates(fakes: &[Fake]) -> Vec<Candidate<'_>> {
        let session = Session::default();
        fakes
            .iter()
            .map(|f| Candidate {
                backend: f,
                availability: f.availability(&session),
            })
            .collect()
    }

    #[test]
    fn the_order_is_fr_i1_with_unavailable_backends_skipped_and_named() {
        let fakes = fakes();
        let choice = choose(
            &candidates(&fakes),
            &Settings::default(),
            "hi",
            InsertMode::Raw,
            VERIFIED,
        )
        .unwrap();
        assert_eq!(
            choice.order,
            [
                "virtual_keyboard",
                "xdotool",
                "clipboard_paste",
                "clipboard"
            ]
        );
        assert_eq!(
            choice.skipped[0],
            ("libei".to_string(), "libei is off".to_string())
        );
        assert_eq!(
            keystroke_sender(&candidates(&fakes), VERIFIED)
                .unwrap()
                .name(),
            "virtual_keyboard"
        );
        let xwayland = TargetFacts {
            verified: true,
            xwayland: true,
        };
        let choice = choose(
            &candidates(&fakes),
            &Settings::default(),
            "hi",
            InsertMode::Raw,
            xwayland,
        )
        .unwrap();
        assert_eq!(choice.order[0], "xdotool");
        assert!(
            choice
                .skipped
                .iter()
                .any(|(n, r)| n == "virtual_keyboard" && r.contains("XWayland"))
        );
        assert_eq!(
            keystroke_sender(&candidates(&fakes), xwayland)
                .unwrap()
                .name(),
            "xdotool"
        );
    }

    #[test]
    fn a_pin_selects_one_backend_and_an_unavailable_pin_fails_with_its_reason() {
        let fakes = fakes();
        let mut settings = Settings {
            backend: "xdotool".into(),
            ..Settings::default()
        };
        let choice = choose(
            &candidates(&fakes),
            &settings,
            "hi",
            InsertMode::Raw,
            VERIFIED,
        )
        .unwrap();
        assert_eq!(choice.order, ["xdotool"]);
        settings.backend = "libei".into();
        let err = choose(
            &candidates(&fakes),
            &settings,
            "hi",
            InsertMode::Raw,
            VERIFIED,
        )
        .unwrap_err();
        assert_eq!(
            err,
            Refusal::NothingAvailable("pinned backend libei is unavailable: libei is off".into())
        );
        settings.backend = "typewriter".into();
        assert!(matches!(
            choose(&candidates(&fakes), &settings, "hi", InsertMode::Raw, VERIFIED),
            Err(Refusal::NothingAvailable(r)) if r.contains("not a backend name")
        ));
    }

    #[test]
    fn keymap_coverage_demotes_a_backend_for_that_text() {
        let mut fakes = fakes();
        fakes[0].covers = false;
        let choice = choose(
            &candidates(&fakes),
            &Settings::default(),
            "\u{7}",
            InsertMode::Raw,
            VERIFIED,
        )
        .unwrap();
        assert_eq!(choice.order[0], "xdotool");
        assert!(
            choice
                .skipped
                .iter()
                .any(|(n, r)| n == "virtual_keyboard" && r.starts_with("demoted"))
        );
    }

    #[test]
    fn an_unverifiable_target_permits_clipboard_only_and_clipboard_only_mode_too() {
        let fakes = fakes();
        let choice = choose(
            &candidates(&fakes),
            &Settings::default(),
            "hi",
            InsertMode::Raw,
            TargetFacts::default(),
        )
        .unwrap();
        assert_eq!(choice.order, ["clipboard"]);
        let choice = choose(
            &candidates(&fakes),
            &Settings::default(),
            "hi",
            InsertMode::ClipboardOnly,
            VERIFIED,
        )
        .unwrap();
        assert_eq!(choice.order, ["clipboard"]);
    }
}
