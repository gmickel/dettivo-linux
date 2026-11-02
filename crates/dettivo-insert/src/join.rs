//! Continuation-aware joins (ADR 0091): a dictation that lands in the
//! window the previous one landed in, within [`JOIN_WINDOW`], with no
//! input in between, continues it. It starts with a space unless the
//! previous text ended with a newline or a space, and keeps its capital
//! only when the previous text ended a sentence; otherwise a leading
//! function word is lowercased. The decision reads only Dettivo's own
//! last delivery and the input-idle signal, never the target's text, and
//! without that signal nothing is joined.

use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::idle::InputActivity;

/// How long after a delivery the next dictation may continue it.
pub const JOIN_WINDOW: Duration = Duration::from_secs(30);

/// Input this close before a take starts may be the hotkey that started
/// it: one burst of input inside it is the chord, a second one is typing.
pub const HOTKEY_GRACE: Duration = Duration::from_secs(1);

/// How late after the press the chord's own burst may be recorded.
const CHORD_LATENESS: Duration = Duration::from_millis(50);

/// The words lowercased when a take continues a sentence.
const FUNCTION_WORDS: &[&str] = &["The", "And", "But", "So", "Or", "A", "An"];

/// The window a delivery landed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// The app id.
    pub app_id: String,
    /// The owning pid.
    pub pid: Option<u32>,
    /// The window's identity as the probe knows it.
    pub window: String,
}

struct Last {
    text: String,
    place: Place,
    at: Instant,
    /// Input arrived between this delivery and the next take's start;
    /// true until a start says otherwise.
    typed_since: bool,
}

/// Dettivo's last delivery and the input signal the join reads.
pub struct Continuation {
    activity: Arc<dyn InputActivity>,
    last: Mutex<Option<Last>>,
}

impl Continuation {
    /// Over an input-activity source.
    pub fn new(activity: Arc<dyn InputActivity>) -> Self {
        Self {
            activity,
            last: Mutex::new(None),
        }
    }

    /// A take started at `at`: whether the user gave any input since the
    /// last delivery, leaving out the hotkey that started the take. Input
    /// before [`HOTKEY_GRACE`] is typing; inside it, the burst of input
    /// that pressed the chord is left out, and any other burst is typing.
    pub fn take_started(&self, at: Instant) {
        let mut last = self.last.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(l) = last.as_mut() {
            let grace = at.checked_sub(HOTKEY_GRACE).unwrap_or(at).max(l.at);
            let chord_end = (at + CHORD_LATENESS).min(Instant::now().max(at));
            let before = self.activity.inputs_between(l.at, grace);
            let inside = self.activity.inputs_between(grace, chord_end);
            l.typed_since = !matches!((before, inside), (Some(0), Some(0 | 1)));
        }
    }

    /// The text to deliver into `place`: joined to the last delivery when
    /// it continues it, else `text` as it is.
    pub fn text_for<'a>(&self, place: Option<&Place>, text: &'a str) -> Cow<'a, str> {
        let last = self.last.lock().unwrap_or_else(|p| p.into_inner());
        match (last.as_ref(), place) {
            (Some(l), Some(p))
                if l.place == *p && l.at.elapsed() <= JOIN_WINDOW && !l.typed_since =>
            {
                Cow::Owned(join(&l.text, text))
            }
            _ => Cow::Borrowed(text),
        }
    }

    /// Records what reached `place` at `at`, when its last key or the
    /// paste landed; `None` (nothing typed into a known window) forgets
    /// the last delivery.
    pub fn delivered(&self, place: Option<Place>, text: &str, at: Instant) {
        *self.last.lock().unwrap_or_else(|p| p.into_inner()) = place.map(|place| Last {
            text: text.to_string(),
            place,
            at,
            typed_since: true,
        });
    }
}

/// `text` continuing `previous`.
pub fn join(previous: &str, text: &str) -> String {
    let space = if previous.ends_with(['\n', ' ']) {
        ""
    } else {
        " "
    };
    let body = if previous.trim_end().ends_with(['.', '!', '?']) {
        Cow::Borrowed(text)
    } else {
        lower_leading_function_word(text)
    };
    format!("{space}{body}")
}

fn lower_leading_function_word(text: &str) -> Cow<'_, str> {
    let end = text
        .find(|c: char| !c.is_alphabetic())
        .unwrap_or(text.len());
    let word = &text[..end];
    if FUNCTION_WORDS.contains(&word) {
        Cow::Owned(format!("{}{}", word.to_lowercase(), &text[end..]))
    } else {
        Cow::Borrowed(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_continuation_gets_one_space_and_its_capital_only_after_a_sentence_end() {
        for (previous, text, joined) in [
            (
                "I wrote the report",
                "And then I left.",
                " and then I left.",
            ),
            (
                "I wrote the report.",
                "And then I left.",
                " And then I left.",
            ),
            ("Done!", "The rest waits.", " The rest waits."),
            ("first line\n", "The second.", "the second."),
            ("trailing space ", "So it goes", "so it goes"),
            ("in the", "Afternoon sun", " Afternoon sun"),
            ("we met", "Anna there", " Anna there"),
            ("and", "I agree", " I agree"),
            ("a list", "A, b and c", " a, b and c"),
        ] {
            assert_eq!(join(previous, text), joined, "{previous:?} + {text:?}");
        }
    }

    struct Fixed(Option<bool>);
    impl InputActivity for Fixed {
        fn inputs_between(&self, _: Instant, _: Instant) -> Option<usize> {
            self.0.map(usize::from)
        }
    }

    fn place(window: &str) -> Place {
        Place {
            app_id: "foot".into(),
            pid: Some(42),
            window: window.into(),
        }
    }

    /// fn-85 R3: the same window with no input in between joins; input in
    /// between, another window, no input signal or no take start since
    /// the delivery leave the text as it is.
    #[test]
    fn only_a_quiet_take_into_the_same_window_joins() {
        for (activity, window, started, joined) in [
            (Some(false), "0x1", true, true),
            (Some(true), "0x1", true, false),
            (Some(false), "0x2", true, false),
            (None, "0x1", true, false),
            (Some(false), "0x1", false, false),
        ] {
            let c = Continuation::new(Arc::new(Fixed(activity)));
            c.delivered(Some(place("0x1")), "I wrote the report", Instant::now());
            if started {
                c.take_started(Instant::now());
            }
            let text = c.text_for(Some(&place(window)), "And then I left.");
            let expected = if joined {
                " and then I left."
            } else {
                "And then I left."
            };
            assert_eq!(text, expected, "{activity:?} {window} {started}");
        }
        let c = Continuation::new(Arc::new(Fixed(Some(false))));
        c.delivered(None, "copied, not typed", Instant::now());
        c.take_started(Instant::now());
        assert_eq!(c.text_for(Some(&place("0x1")), "And so"), "And so");
    }

    /// Input bursts at the given times, as the idle watch records them.
    struct Bursts(Vec<Instant>);
    impl InputActivity for Bursts {
        fn inputs_between(&self, from: Instant, until: Instant) -> Option<usize> {
            Some(self.0.iter().filter(|&&r| r > from && r < until).count())
        }
    }

    /// fn-85 R3: typing that paused before the start chord disables the
    /// join even inside the hotkey's second; the chord alone, as one burst
    /// of input, still joins.
    #[test]
    fn typing_just_before_the_chord_disables_the_join() {
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        let press = ms(5_000);
        for (bursts, joined) in [
            (vec![ms(4_950)], true),
            (vec![ms(4_400), ms(4_950)], false),
            (vec![ms(3_000), ms(4_950)], false),
            (vec![], true),
        ] {
            let c = Continuation::new(Arc::new(Bursts(bursts.clone())));
            c.delivered(Some(place("0x1")), "I wrote the report", ms(1_000));
            c.take_started(press);
            let text = c.text_for(Some(&place("0x1")), "And then I left.");
            let expected = if joined {
                " and then I left."
            } else {
                "And then I left."
            };
            assert_eq!(text, expected, "{bursts:?}");
        }
    }
}
