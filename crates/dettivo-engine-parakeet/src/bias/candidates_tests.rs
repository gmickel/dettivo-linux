//! Tests of the candidate finder.

use super::*;

fn term(text: &str, aliases: &[&str]) -> Term {
    let aliases: Vec<String> = aliases.iter().map(|a| a.to_string()).collect();
    Term::new(text, &aliases).unwrap()
}

fn spans(heard: &str, terms: &[Term]) -> Vec<(usize, usize, usize)> {
    let words: Vec<&str> = heard.split_whitespace().collect();
    find(&words, terms, &vec![true; words.len()], MAX_CANDIDATES)
        .into_iter()
        .map(|c| (c.first, c.last, c.term))
        .collect()
}

#[test]
fn mishearings_of_a_term_are_candidates() {
    let terms = [
        term("Dettivo", &["detivo"]),
        term("Hyprland", &[]),
        term("SapienXT", &[]),
        term("Omarchy", &[]),
    ];
    // An edit away from an alias, letters in common, two words that
    // spell the term, one consonant sound off.
    assert!(spans("with dativo on", &terms).contains(&(1, 1, 0)));
    assert!(spans("The HyperLAN config", &terms).contains(&(1, 1, 1)));
    assert!(spans("to Zapian XT before", &terms).contains(&(1, 2, 2)));
    assert!(spans("in Omar He changed", &terms).contains(&(1, 2, 3)));
    assert!(spans("Itivo puts the text", &terms).contains(&(0, 0, 0)));
}

/// R4: an alias widens matching.
#[test]
fn an_alias_finds_a_span_the_spelling_alone_misses() {
    assert_eq!(
        spans("she shaped the clay", &[term("Clef", &["clay"])]),
        vec![(3, 3, 0)]
    );
    assert!(
        Term::new("Clef", &[]).is_none(),
        "a short term needs an alias"
    );
    assert_eq!(
        spans("I installed oh marky", &[term("Omarchy", &["oh marky"])]),
        vec![(2, 3, 0)]
    );
    assert_eq!(
        spans("I installed oh marky", &[term("Omarchy", &[])]),
        vec![]
    );
}

/// R5: case-only differences, inflections, paths, URLs and addresses
/// are never rewritten, and short terms without an alias are skipped.
#[test]
fn what_must_stay_as_heard_is_never_a_candidate() {
    let terms = [term("Dettivo", &["detivo"])];
    for heard in [
        "open dettivo now",
        "open DETTIVO now",
        "two Dettivos here",
        "the Dettivo's window",
        "the detivos window",
        "see src/detivo please",
        "see https://dativo.com please",
        "see dativo.com please",
        "mail me@dativo.org please",
    ] {
        assert_eq!(spans(heard, &terms), vec![], "{heard}");
    }
    assert!(Term::new("Jev", &[]).is_none());
    assert!(Term::new("Jev", &["jeff".into()]).is_some());
}

/// R5: a plural or possessive fused onto a mishearing of the term
/// ("Hyperlans" for "Hyprland's") stays as heard, while a term that
/// itself ends in `s` still matches a mishearing ending in `s`.
#[test]
fn an_inflected_mishearing_is_never_a_candidate() {
    let terms = [
        term("Hyprland", &["hyper land", "hyperland"]),
        term("SapienXT", &[]),
    ];
    for heard in [
        "Hyperlans config is short.",
        "Hyperlan's config is short.",
        "Both hyperlans crashed",
        "two Zapian XTs here",
        "the Zapian XT's dashboard",
    ] {
        assert_eq!(spans(heard, &terms), vec![], "{heard}");
    }
    assert_eq!(
        spans("I run Cubernetes at home", &[term("Kubernetes", &[])]),
        vec![(2, 2, 0)]
    );
    // A mishearing followed by a word ending in `s` is still one.
    assert!(spans("Hyperland draws the windows", &terms).contains(&(0, 0, 0)));
}

/// R5: words that already are a multi-word term are never rewritten
/// in part, so no word of the term is written twice.
#[test]
fn a_term_already_heard_is_never_rewritten_in_part() {
    for (heard, written) in [
        ("moved because of the holiday", "because of the"),
        ("O Markey ships with", "o Markey"),
        ("Rain is expected over", "Rain Is"),
        ("two Sapien XTs here", "Sapien XT"),
    ] {
        assert_eq!(spans(heard, &[term(written, &[])]), vec![], "{heard}");
    }
}

#[test]
fn a_span_never_swallows_a_neighbouring_word() {
    let found = spans("invoice to Sapien XT before", &[term("SapienXT", &[])]);
    assert!(found.contains(&(2, 3, 0)), "{found:?}");
    assert!(!found.contains(&(1, 3, 0)), "{found:?}");
}

/// A short term whose only spelling differs from it in spaces or marks
/// keeps that spelling, so `a w s` still becomes `AWS`.
#[test]
fn a_short_term_spelt_out_letter_by_letter_still_matches() {
    assert!(Term::new("AWS", &["A W S".into()]).is_some());
    let found = spans("deploy to a w s today", &[term("AWS", &["A W S"])]);
    assert_eq!(found.first(), Some(&(2, 4, 0)), "{found:?}");
}

/// R5: one spelling ending in `s` no longer turns the inflection guard
/// off for the whole term, and a written possessive of an `s`-ending
/// term stays as heard.
#[test]
fn an_s_ending_spelling_keeps_inflections_guarded() {
    let hyprland = [term("Hyprland", &["hyper land", "hyperland", "hyperlands"])];
    for heard in ["Hyperlans config is short.", "Hyperlan's config is short."] {
        assert_eq!(spans(heard, &hyprland), vec![], "{heard}");
    }
    // A listed spelling ending in `s` is still a candidate as it
    // stands, and its near miss without the `s` stays guarded.
    let listed = [term("Hyprland", &["hyperlands"])];
    assert_eq!(spans("Hyperlands config", &listed), vec![(0, 0, 0)]);
    assert_eq!(spans("Hyperlans config", &listed), vec![]);
    let kubernetes = [term("Kubernetes", &[])];
    for heard in ["Cubernetes's nodes", "the Cubernetes' nodes"] {
        assert_eq!(spans(heard, &kubernetes), vec![], "{heard}");
    }
}

/// Code stays as heard: an identifier joined by `::` and every word of
/// a backticked span.
#[test]
fn identifiers_and_backticked_code_are_never_candidates() {
    let terms = [term("Hyprland", &["hyperland"])];
    for heard in [
        "call x::Hyperland now",
        "run `cargo Hyperland build` now",
        "run `Hyperland` now",
    ] {
        assert_eq!(spans(heard, &terms), vec![], "{heard}");
    }
    assert_eq!(spans("run `cargo` on Hyperland", &terms), vec![(3, 3, 0)]);
}

/// A run never crosses a sentence or clause mark between its words.
#[test]
fn a_span_never_crosses_punctuation() {
    let terms = [term("New York", &[])];
    for heard in ["moved to New. York later", "the new, York office"] {
        let found = spans(heard, &terms);
        assert!(!found.iter().any(|&(f, l, _)| f < l), "{heard}: {found:?}");
    }
    assert_eq!(spans("moved to New Yolk later", &terms), vec![(2, 3, 0)]);
}

/// R5: no shorter run inside an inflected mishearing is rewritten, so
/// `Hyperland Pro's` never becomes `Hyprland Pro Pro's`; a word that
/// only happens to end in `s` is not an inflection.
#[test]
fn no_part_of_an_inflected_mishearing_is_a_candidate() {
    let terms = [term("Hyprland Pro", &["hyperland"])];
    assert_eq!(spans("open Hyperland Pro's settings", &terms), vec![]);
    assert_eq!(spans("open Hyperland Pros settings", &terms), vec![]);
    assert!(spans("Hyperland draws the windows", &terms).contains(&(0, 0, 0)));
}

/// Only eligible words make a candidate (a chunk's own time), and the
/// count stops at the budget the take has left.
#[test]
fn ineligible_words_and_a_spent_budget_give_no_candidate() {
    let terms = [term("SapienXT", &[])];
    let words = ["to", "Zapian", "XT", "and", "Zapian", "XT"];
    let found = |eligible: &[bool], limit| -> Vec<(usize, usize)> {
        find(&words, &terms, eligible, limit)
            .into_iter()
            .map(|c| (c.first, c.last))
            .collect()
    };
    let all = [true; 6];
    assert_eq!(found(&all, MAX_CANDIDATES).len(), 2);
    assert_eq!(found(&all, 1).len(), 1);
    assert_eq!(found(&all, 0), vec![]);
    // The first run straddles the cut after `Zapian`: only the second
    // run, whole on this side, is a candidate.
    let after_cut = [false, false, true, true, true, true];
    assert_eq!(found(&after_cut, MAX_CANDIDATES), vec![(4, 5)]);
}

#[test]
fn unrelated_words_are_not_candidates_and_the_count_is_capped() {
    let terms = [
        term("Hyprland", &[]),
        term("SapienXT", &[]),
        term("Detective", &[]),
    ];
    assert_eq!(spans("We walked across the highland", &terms[..2]), vec![]);
    let many = "detectiv ".repeat(20);
    assert_eq!(spans(&many, &terms).len(), MAX_CANDIDATES);
}
