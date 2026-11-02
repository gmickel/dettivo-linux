use super::*;

/// A segment spelt by `(token text, log probability)` pairs, as
/// whisper.cpp returns one: the text and its tokens.
fn segment(pieces: &[(&str, f32)]) -> (String, Vec<Token>) {
    let tokens: Vec<Token> = pieces
        .iter()
        .map(|&(text, plog)| Token {
            bytes: text.as_bytes().to_vec(),
            t0: 0,
            t1: 0,
            dtw: 0,
            p: plog.exp(),
            plog,
        })
        .collect();
    (pieces.iter().map(|p| p.0).collect(), tokens)
}

/// A whole take, each piece certain unless it carries its own probability.
fn take(segments: &[&[(&str, f32)]], terms: &[&str]) -> Finding {
    let (texts, tokens): (Vec<String>, Vec<Vec<Token>>) =
        segments.iter().map(|s| segment(s)).unzip();
    let terms: Vec<String> = terms.iter().map(|t| t.to_string()).collect();
    find(&texts, &tokens, &terms)
}

fn plain(texts: &[&str]) -> Vec<String> {
    texts.iter().map(|t| t.to_string()).collect()
}

const TERMS: &[&str] = &["Omarchy", "Hyprland", "Clef", "SapienXT"];

/// R3: a take that spells no listed term has no candidate and asks for no
/// second decode.
#[test]
fn a_take_without_a_listed_term_needs_nothing() {
    let found = take(
        &[&[(" The detective closed", 0.0), (" the case.", 0.0)]],
        TERMS,
    );
    assert_eq!(found.candidates(), 0);
    assert!(!found.needs_plain_decode());
}

/// A confident term inside a sentence stands without a second decode;
/// split, plural and possessive spellings are term spans too.
#[test]
fn a_confident_term_in_a_sentence_stands() {
    let found = take(
        &[&[
            (" I bound the key in", 0.0),
            (" Hy", -0.4),
            ("pr", -0.2),
            ("land.", -0.1),
        ]],
        TERMS,
    );
    assert_eq!(found.candidates(), 1);
    assert!(!found.needs_plain_decode());
    let spellings = take(
        &[&[(" Sapien XT and Clefs and Hyprland's config.", 0.0)]],
        TERMS,
    );
    assert_eq!(spellings.candidates(), 3);
    assert!(!spellings.needs_plain_decode());
}

/// R1: a term whose tokens are unsure gives way to what the prompt-free
/// decode heard there, and only the gap between shared words changes.
#[test]
fn an_unsure_term_gives_way_to_the_prompt_free_words() {
    let found = take(
        &[&[(" Clef", -2.5), (" and I went to the cinema.", -0.1)]],
        TERMS,
    );
    assert!(found.needs_plain_decode());
    let settled = settle(&found, &plain(&[" Play and I went to the cinema."]));
    assert_eq!(settled, Some(plain(&["Play and I went to the cinema."])));
    let merged = take(
        &[&[(" Omarchy", -1.6), (" said would arrive at noon.", -0.1)]],
        TERMS,
    );
    assert_eq!(
        settle(&merged, &plain(&[" Omar, he said, would arrive at noon."])),
        Some(plain(&["Omar, he said would arrive at noon."]))
    );
}

/// R1: a segment, or the sentences closing one, made only of listed terms
/// is an echo however sure the decoder was; it becomes what the
/// prompt-free decode heard there, or nothing.
#[test]
fn an_echo_of_listed_terms_is_replaced_or_dropped() {
    let whole = take(&[&[(" Omarchy,", 0.0), (" Hyprland.", 0.0)]], TERMS);
    assert!(whole.needs_plain_decode());
    let heard = " Omar keeps the spare keys in his desk.";
    assert_eq!(
        settle(&whole, &plain(&[heard])),
        Some(plain(&[heard.trim()]))
    );
    let trailing = take(
        &[&[(" Book the room.", -0.1), (" Omarchy, Hyprland.", 0.0)]],
        TERMS,
    );
    assert!(trailing.needs_plain_decode());
    assert_eq!(
        settle(&trailing, &plain(&[" Book the room."])),
        Some(plain(&["Book the room."]))
    );
    let own_segment = take(
        &[&[(" Book the room.", -0.1)], &[(" Hyprland.", 0.0)]],
        TERMS,
    );
    assert_eq!(
        settle(&own_segment, &plain(&[" Book the room."])),
        Some(plain(&["Book the room.", ""]))
    );
}

/// A suspect the prompt-free decode also spells stands, even split into
/// other words.
#[test]
fn a_suspect_the_prompt_free_decode_also_spells_stands() {
    let found = take(
        &[&[
            (" Send it to", -0.1),
            (" SapienXT", -3.0),
            (" today.", -0.1),
        ]],
        TERMS,
    );
    assert!(found.needs_plain_decode());
    assert_eq!(
        settle(&found, &plain(&[" Send it to Sapien XT today."])),
        None
    );
    let echo = take(&[&[(" Clef.", 0.0)]], TERMS);
    assert_eq!(settle(&echo, &plain(&[" Clef."])), None);
}

/// Tokens that do not spell the segment's text leave no confidence, so
/// the term is checked rather than trusted.
#[test]
fn tokens_that_do_not_spell_the_text_leave_the_term_unsure() {
    let (text, mut tokens) = segment(&[(" Ask the Clef team.", 0.0)]);
    tokens[0].bytes.pop();
    let terms = vec!["Clef".to_string()];
    assert!(find(&[text], &[tokens], &terms).needs_plain_decode());
}
