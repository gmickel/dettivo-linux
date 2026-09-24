//! The raw layer's cases: spoken punctuation, replacements, whitespace
//! and, since fn-39, every protected token class through the whole pass.

use super::apply;

fn raw(text: &str) -> String {
    apply(text, &[], true, true)
}

#[test]
fn spoken_punctuation_becomes_marks_and_glues() {
    assert_eq!(raw("hello comma world period"), "Hello, world.");
    assert_eq!(raw("is it done question mark yes"), "Is it done? Yes");
    assert_eq!(raw("one new line two"), "One\ntwo");
    assert_eq!(raw("first new paragraph second"), "First\n\nsecond");
    assert_eq!(raw("open quote hi close quote"), "\u{201c}hi\u{201d}");
    assert_eq!(raw("wait exclamation mark now"), "Wait! Now");
}

#[test]
fn non_ascii_text_never_breaks_matching() {
    let r = vec![("straße".to_string(), "Strasse".to_string())];
    assert_eq!(
        apply("Die STRASSE und die Straße", &r, false, true),
        "Die STRASSE und die Strasse"
    );
    assert_eq!(
        apply(
            "İstanbul café 日本語 ﬁne",
            &[("café".into(), "cafe".into())],
            true,
            true
        ),
        "İstanbul cafe 日本語 ﬁne"
    );
}

#[test]
fn replacements_are_whole_words_and_case_insensitive() {
    let r = vec![
        ("teh".to_string(), "the".to_string()),
        ("dettivo".to_string(), "Dettivo".to_string()),
    ];
    assert_eq!(
        apply("Teh dettivo daemon and tether", &r, false, true),
        "The Dettivo daemon and tether"
    );
}

#[test]
fn whitespace_is_tidied_and_punctuation_can_stay_literal() {
    assert_eq!(
        apply("  too   many   spaces ", &[], false, true),
        "Too many spaces"
    );
    assert_eq!(
        apply("say the word comma please", &[], false, true),
        "Say the word comma please"
    );
    assert_eq!(raw(""), "");
}

#[test]
fn the_golden_set_holds() {
    let cases = [
        (
            "meet me at nine comma then we go period",
            "Meet me at nine, then we go.",
        ),
        (
            "the path is slash home slash gordon",
            "The path is slash home slash gordon",
        ),
        ("dash is a word here", "- is a word here"),
        ("open paren inside close paren after", "(inside) after"),
    ];
    for (input, want) in cases {
        assert_eq!(raw(input), want, "{input}");
    }
}

/// R1: one token per class, dictated mid-sentence with a spoken sentence
/// end after it, comes back byte-identical and the sentence rules still
/// fire around it.
#[test]
fn every_protected_token_class_survives_the_raw_layer() {
    let tokens = [
        "index.ts",
        "src/app/index.ts",
        "/home/gordon/work",
        "https://example.com/docs?q=1",
        "www.example.com",
        "gordon@mickel.tech",
        "v1.2.3",
        "1.2",
        "foo.bar()",
        "std::io::Error",
        "`cargo build --release`",
        "straße.txt",
    ];
    for token in tokens {
        let input = format!("open {token} period then run it");
        let got = raw(&input);
        assert_eq!(got, format!("Open {token}. Then run it"), "{token}");
        assert!(got.contains(token), "{token} changed bytes: {got}");
    }
}

#[test]
fn a_leading_token_keeps_its_case_and_a_sentence_can_end_with_one() {
    assert_eq!(raw("index.ts is open"), "index.ts is open");
    assert_eq!(
        raw("open index.ts. then run it."),
        "Open index.ts. Then run it."
    );
    assert_eq!(raw("see example.com."), "See example.com.");
    assert_eq!(
        raw("open index.ts then run it"),
        "Open index.ts then run it"
    );
}

#[test]
fn protection_off_restores_the_old_sentence_rule() {
    assert_eq!(
        apply("open index.ts now", &[], true, false),
        "Open index. Ts now"
    );
}

/// R2: a rule whose source appears inside a URL and outside it rewrites
/// only the plain occurrence; a rule that names a token whole still wins.
#[test]
fn replacements_stay_out_of_protected_spans() {
    let r = vec![("example".to_string(), "sample".to_string())];
    assert_eq!(
        apply(
            "an example at https://example.com/example and gordon@example.org",
            &r,
            false,
            true
        ),
        "An sample at https://example.com/example and gordon@example.org"
    );
    let r = vec![("ts".to_string(), "TypeScript".to_string())];
    assert_eq!(
        apply("index.ts is ts", &r, false, true),
        "index.ts is TypeScript"
    );
    let r = vec![("e.g.".to_string(), "for example".to_string())];
    assert_eq!(
        apply("e.g. this one", &r, true, true),
        "For example this one"
    );
    let r = vec![("index.ts".to_string(), "index.tsx".to_string())];
    assert_eq!(
        apply("open index.ts now", &r, true, true),
        "Open index.tsx now"
    );
}
