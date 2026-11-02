//! Word boundaries, marks, repeated words and bytes the check must get
//! right (PR #49 review).

use super::tests::{plain, take};
use super::*;

/// A term inside Chinese or Japanese text, which has no spaces between
/// words, is a span; its replacement adds no spaces either.
#[test]
fn a_term_inside_text_without_spaces_is_found() {
    let sure = take(&[&[(" 我在纽约工作。", 0.0)]], &["纽约"]);
    assert_eq!(sure.candidates(), 1);
    assert!(!sure.needs_plain_decode());
    let unsure = take(
        &[&[(" 我在", 0.0), ("纽约", -1.5), ("工作。", 0.0)]],
        &["纽约"],
    );
    assert!(unsure.needs_plain_decode());
    assert_eq!(
        settle(&unsure, &plain(&[" 我在牛约工作。"])),
        Some(plain(&["我在牛约工作。"]))
    );
    let japanese = take(&[&[(" 東京", -1.0), ("に行きます。", 0.0)]], &["東京"]);
    assert_eq!(japanese.candidates(), 1);
    assert_eq!(settle(&japanese, &plain(&[" 東京に行きます。"])), None);
}

/// A sentence that ends inside a closing quote or bracket still ends, so
/// the name after it is a sentence of its own and may stay by sound.
#[test]
fn a_sentence_closed_by_a_quote_lets_the_next_name_stand_alone() {
    let found = take(
        &[&[
            (" He said \"hello.\"", 0.0),
            (" Clef.", -0.416),
            (" Continue.", 0.0),
        ]],
        &["Clef"],
    );
    assert!(found.needs_plain_decode());
    assert_eq!(
        settle(&found, &plain(&[" He said \"hello.\" Pleph. Continue."])),
        None
    );
}

/// A multi-word term never spans a full stop or a comma between its
/// words, unless the term is spelt with that mark.
#[test]
fn a_multi_word_term_never_crosses_punctuation() {
    for text in [" It was new. York is big.", " It was new, York said."] {
        let found = take(&[&[(text, -1.0)]], &["New York"]);
        assert_eq!(found.candidates(), 0, "{text}");
        assert!(!found.needs_plain_decode(), "{text}");
    }
    let marked = take(&[&[(" Open Yahoo! Mail now.", 0.0)]], &["Yahoo! Mail"]);
    assert_eq!(marked.candidates(), 1);
}

/// `C++` and `C#` keep their marks: a plain `C` is no span of them, and
/// a prompt-free `C` does not confirm an unsure `C++`.
#[test]
fn marks_a_term_is_spelt_with_tell_it_apart() {
    for term in ["C++", "C#"] {
        let found = take(&[&[(" I wrote C code.", -1.0)]], &[term]);
        assert_eq!(found.candidates(), 0, "{term}");
    }
    let unsure = take(
        &[&[(" I wrote", 0.0), (" C++", -1.0), (" code.", 0.0)]],
        &["C++"],
    );
    assert_eq!(unsure.candidates(), 1);
    assert_eq!(
        settle(&unsure, &plain(&[" I wrote C code."])),
        Some(plain(&["I wrote C code."]))
    );
    let sure = take(&[&[(" I wrote C++, then C#.", 0.0)]], &["C++", "C#"]);
    assert_eq!(sure.candidates(), 2);
}

/// A suspect is not confirmed by another occurrence of the same word that
/// the prompt-free decode places elsewhere.
#[test]
fn a_repeated_word_does_not_confirm_a_suspect() {
    let found = take(&[&[(" Clef.", -1.0), (" Clef works.", 0.0)]], &["Clef"]);
    assert!(found.needs_plain_decode());
    assert_eq!(
        settle(&found, &plain(&[" Clef works."])),
        Some(plain(&["Clef works."]))
    );
}

/// A multi-word term stands only when the prompt-free decode heard its
/// words together, not with a word between them.
#[test]
fn a_multi_word_term_must_be_heard_whole() {
    let found = take(
        &[&[
            (" Visit", 0.0),
            (" New York University", -1.0),
            (" tomorrow.", 0.0),
        ]],
        &["New York University"],
    );
    assert!(found.needs_plain_decode());
    assert_eq!(
        settle(
            &found,
            &plain(&[" Visit New York State University tomorrow."])
        ),
        Some(plain(&["Visit New York State University tomorrow."]))
    );
    assert_eq!(
        settle(&found, &plain(&[" Visit New York University tomorrow."])),
        None
    );
}

/// A lone name with a version number does not stay when the prompt-free
/// decode heard another number.
#[test]
fn a_lone_name_with_other_digits_gives_way() {
    let found = take(&[&[(" GPT-4.", -0.5)]], &["GPT-4"]);
    assert!(found.needs_plain_decode());
    assert_eq!(
        settle(&found, &plain(&[" GPT-3."])),
        Some(plain(&["GPT-3."]))
    );
    assert_eq!(settle(&found, &plain(&[" GPT 4."])), None);
}

/// Only the settled gap changes: spacing elsewhere, in the same segment
/// and in the others, comes back byte for byte.
#[test]
fn bytes_outside_the_settled_gap_are_kept() {
    let found = take(
        &[
            &[(" Keep  two  spaces.", 0.0)],
            &[(" Run `foo  bar`", 0.0), (" Clef", -1.0), (" now.", 0.0)],
        ],
        &["Clef"],
    );
    assert!(found.needs_plain_decode());
    assert_eq!(
        settle(
            &found,
            &plain(&[" Keep two spaces. Run `foo bar` Clyde now."])
        ),
        Some(plain(&["Keep  two  spaces.", "Run `foo  bar` Clyde now."]))
    );
    assert_eq!(
        settle(&found, &plain(&[" Keep two spaces. Run `foo bar` now."])),
        Some(plain(&["Keep  two  spaces.", "Run `foo  bar` now."]))
    );
}

/// An echo of two terms after a listed name that ends its own sentence
/// is still an echo: the run starts after that sentence's end.
#[test]
fn an_echo_after_a_sentence_ending_name_is_an_echo() {
    let found = take(
        &[&[(" Book Hyprland.", 0.0), (" Omarchy, Clef.", 0.0)]],
        &["Omarchy", "Hyprland", "Clef"],
    );
    assert!(found.needs_plain_decode());
    assert_eq!(
        settle(&found, &plain(&[" Book Hyprland."])),
        Some(plain(&["Book Hyprland."]))
    );
    let names = take(&[&[(" Omarchy. Hyprland.", 0.0)]], &["Omarchy", "Hyprland"]);
    assert!(names.needs_plain_decode());
}
