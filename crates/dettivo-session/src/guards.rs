//! What a dictation take's recognized text loses before the text pipeline
//! sees it: a segment that only echoes the vocabulary prompt, a decoder
//! loop (the text before it stays), and the known silence hallucinations
//! and blank markers the meeting pipeline drops
//! (`dettivo_transcribe::filters`). The engine's own text stands when no
//! segment changed.

use dettivo_engine_proto::RecognizeResult;
use dettivo_transcribe::filters::{canonical, should_drop};

/// The most words a looping n-gram has.
const LOOP_MAX_NGRAM: usize = 8;
/// A loop repeats its n-gram at least this many times in a row...
const LOOP_MIN_REPEATS: usize = 4;
/// ...over at least this many words, so "no, no, no" stays speech.
const LOOP_MIN_WORDS: usize = 8;

/// The text of `result` with the guards applied; `prompt_terms` are the
/// vocabulary terms the engine was primed with (none when it took no
/// prompt).
pub(crate) fn clean(result: &RecognizeResult, prompt_terms: &[String]) -> String {
    let pieces: Vec<(&str, u64)> = if result.segments.is_empty() {
        vec![(result.text.as_str(), result.duration_ms)]
    } else {
        result
            .segments
            .iter()
            .map(|s| (s.text.as_str(), s.end_ms.saturating_sub(s.start_ms)))
            .collect()
    };
    let mut changed = false;
    let mut kept = Vec::with_capacity(pieces.len());
    for (text, duration_ms) in pieces {
        let cut = before_loop(text);
        let drop = should_drop(cut, duration_ms, false) || is_prompt_echo(cut, prompt_terms);
        changed |= drop || cut.len() != text.trim_end().len();
        if !drop {
            kept.push(cut);
        }
    }
    let text = if changed {
        kept.join(" ")
    } else {
        result.text.clone()
    };
    if is_prompt_echo(&text, prompt_terms) {
        return String::new();
    }
    text
}

/// True when `text` is the whole prompt read back: the prompt has two or
/// more terms, so dictating the one listed name still pastes it.
fn is_prompt_echo(text: &str, prompt_terms: &[String]) -> bool {
    prompt_terms.len() >= 2 && normalise(text) == normalise(&prompt_terms.join(" "))
}

/// Lower case, punctuation stripped, spelled digits as digits, whitespace
/// collapsed.
fn normalise(text: &str) -> String {
    const DIGITS: [&str; 10] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    ];
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| match DIGITS.iter().position(|d| *d == w) {
            Some(d) => d.to_string(),
            None => w.to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `text` up to the first decoder loop, trimmed; all of it when none.
fn before_loop(text: &str) -> &str {
    let words: Vec<(usize, String)> = text
        .split_whitespace()
        .map(|w| (w.as_ptr() as usize - text.as_ptr() as usize, canonical(w)))
        .collect();
    for start in 0..words.len() {
        for n in 1..=LOOP_MAX_NGRAM {
            let gram = |k: usize| words.get(start + k * n..start + (k + 1) * n);
            let Some(first) = gram(0) else {
                break;
            };
            let mut repeats = 1;
            while gram(repeats).is_some_and(|g| same(g, first)) {
                repeats += 1;
            }
            if repeats >= LOOP_MIN_REPEATS && repeats * n >= LOOP_MIN_WORDS {
                return text[..words[start].0].trim_end();
            }
        }
    }
    text.trim_end()
}

fn same(a: &[(usize, String)], b: &[(usize, String)]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.1 == y.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dettivo_engine_proto::{Backend, Segment};

    /// An engine answer shaped like the one recorded: `(start_ms, end_ms,
    /// text)` per segment, the text joined the way the engine joins it.
    fn answer(segments: &[(u64, u64, &str)]) -> RecognizeResult {
        RecognizeResult {
            text: segments.iter().map(|s| s.2).collect::<Vec<_>>().join(" "),
            language: "en".into(),
            segments: segments
                .iter()
                .map(|&(start_ms, end_ms, text)| Segment {
                    start_ms,
                    end_ms,
                    text: text.into(),
                    words: Vec::new(),
                })
                .collect(),
            duration_ms: segments.last().map_or(0, |s| s.1),
            backend: Backend::Cpu,
        }
    }

    fn terms(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    /// R6: silence primed with a two-term vocabulary never pastes the
    /// vocabulary; saying the one listed term pastes it.
    #[test]
    fn a_prompt_echo_is_dropped_and_one_listed_term_is_kept() {
        let two = terms(&["Dettivo", "Omarchy"]);
        let echo = answer(&[(0, 1_500, "Dettivo, Omarchy.")]);
        assert_eq!(clean(&echo, &two), "");
        let spoken_digits = answer(&[(0, 1_500, "Clef one, Astra.")]);
        assert_eq!(clean(&spoken_digits, &terms(&["Clef 1", "Astra"])), "");
        let split = answer(&[(0, 700, "Dettivo,"), (700, 1_500, "Omarchy.")]);
        assert_eq!(clean(&split, &two), "", "an echo over two segments");
        let one = terms(&["Dettivo"]);
        assert_eq!(clean(&answer(&[(0, 900, "Dettivo.")]), &one), "Dettivo.");
        let more = answer(&[(0, 2_000, "Dettivo and Omarchy work.")]);
        assert_eq!(clean(&more, &two), "Dettivo and Omarchy work.");
        assert_eq!(clean(&echo, &[]), "Dettivo, Omarchy.", "no prompt, no echo");
    }

    /// R7: a decoder loop yields the text before it.
    #[test]
    fn a_decoder_loop_keeps_the_text_before_it() {
        let looped = answer(&[
            (0, 2_000, "Send the report to Anna."),
            (
                2_000,
                30_000,
                "And then we will. And then we will. And then we will. And then we will. And then we will.",
            ),
        ]);
        assert_eq!(clean(&looped, &[]), "Send the report to Anna.");
        let inline = answer(&[(
            0,
            9_000,
            "Ship it on Friday the the the the the the the the the the",
        )]);
        assert_eq!(clean(&inline, &[]), "Ship it on Friday");
        // Repetition people say is not a loop.
        for kept in [
            "No, no, no, not that one.",
            "Very very very good.",
            "Ha ha ha ha.",
        ] {
            assert_eq!(clean(&answer(&[(0, 2_000, kept)]), &[]), kept);
        }
    }

    /// R7: the known silence hallucinations and blank markers go; the
    /// engine's own text stands when nothing changed.
    #[test]
    fn known_hallucinations_and_blank_markers_are_dropped() {
        let filler = answer(&[
            (0, 2_000, "Book the room for Tuesday."),
            (2_000, 4_000, "Thanks for watching!"),
            (4_000, 5_000, "[BLANK_AUDIO]"),
        ]);
        assert_eq!(clean(&filler, &[]), "Book the room for Tuesday.");
        assert_eq!(clean(&answer(&[(0, 1_000, " [BLANK_AUDIO]")]), &[]), "");
        let mut own = answer(&[(0, 900, "Hello,"), (900, 1_800, "world.")]);
        own.text = "Hello, world.".into();
        assert_eq!(clean(&own, &[]), "Hello, world.");
        let untimed = RecognizeResult {
            segments: Vec::new(),
            ..answer(&[(0, 1_000, "Thanks for watching.")])
        };
        assert_eq!(clean(&untimed, &[]), "");
    }
}
