use super::*;

fn word(text: &str, start_ms: u64, end_ms: u64) -> Word {
    Word {
        start_ms,
        end_ms,
        text: text.into(),
        confidence: 1.0,
    }
}

/// One word every second, 600 ms long: `w0` at 0 s, `w1` at 1 s, ...
fn speech(from_s: u64, until_s: u64) -> Vec<Word> {
    (from_s..until_s)
        .map(|s| word(&format!("w{s}"), s * 1000, s * 1000 + 600))
        .collect()
}

fn names(words: &[Word]) -> Vec<String> {
    words.iter().map(|w| w.text.clone()).collect()
}

#[test]
fn no_round_decodes_the_whole_take() {
    assert_eq!(
        Rounds::default().release(&WINDOWS, 30_000, None),
        Release::Whole
    );
}

#[test]
fn a_short_take_commits_nothing_and_decodes_from_the_start() {
    let mut r = Rounds::default();
    r.absorb(&WINDOWS, 30_000, speech(0, 30));
    assert_eq!(r.committed(), 0);
    assert_eq!(r.window(&WINDOWS, 31_500), (0, 31_500));
}

#[test]
fn a_long_take_commits_the_words_behind_the_horizon() {
    let mut r = Rounds::default();
    r.absorb(&WINDOWS, 70_000, speech(0, 70));
    // Words ending by 50 s are committed: w0..w49.
    assert_eq!(r.committed(), 50);
    // The next round starts the context before the commit point.
    let (from, until) = r.window(&WINDOWS, 71_500);
    assert_eq!((from, until), (29_800, 71_500));
    // A round's words before the commit point were context only.
    r.absorb(&WINDOWS, 71_500, speech(29, 71));
    let Release::Tail { from_ms, cut_ms } = r.release(&WINDOWS, 72_000, None) else {
        panic!("expected a tail decode");
    };
    assert!((62_000..=69_500).contains(&cut_ms), "{cut_ms}");
    let tail = speech(from_ms / 1000, 72);
    let all = r.stitch(cut_ms, tail);
    assert_eq!(names(&all), names(&speech(0, 72)));
}

#[test]
fn silence_after_the_last_round_reuses_its_words() {
    let mut r = Rounds::default();
    r.absorb(&WINDOWS, 30_000, speech(0, 29));
    assert_eq!(
        r.release(&WINDOWS, 31_000, Some(29_700)),
        Release::Reuse(speech(0, 29))
    );
    // Voice after the round's end needs the tail.
    assert!(matches!(
        r.release(&WINDOWS, 31_000, Some(30_500)),
        Release::Tail { .. }
    ));
}

#[test]
fn the_cut_falls_in_a_pause_near_the_tail_start() {
    let mut r = Rounds::default();
    // A long pause between 25.6 s and 27 s, near the tail's start at 26.5 s.
    let mut words = speech(0, 26);
    words.extend(speech(27, 30));
    r.absorb(&WINDOWS, 30_000, words);
    let Release::Tail { from_ms, cut_ms } = r.release(&WINDOWS, 30_500, None) else {
        panic!("expected a tail decode");
    };
    assert_eq!(cut_ms, 26_300);
    assert_eq!(from_ms, 24_300);
}

#[test]
fn a_round_that_fell_behind_decodes_the_whole_take() {
    let mut r = Rounds::default();
    r.absorb(&WINDOWS, 10_000, speech(0, 10));
    assert_eq!(r.release(&WINDOWS, 40_000, None), Release::Whole);
}

#[test]
fn a_word_on_the_cut_is_kept_once() {
    let mut r = Rounds::default();
    r.absorb(&WINDOWS, 20_000, speech(0, 20));
    let Release::Tail { cut_ms, .. } = r.release(&WINDOWS, 20_500, None) else {
        panic!("expected a tail decode");
    };
    // The release decode times the word on the cut slightly differently.
    let tail: Vec<Word> = speech(10, 21)
        .into_iter()
        .map(|mut w| {
            w.start_ms += 40;
            w.end_ms += 40;
            w
        })
        .collect();
    let all = r.stitch(cut_ms, tail);
    assert_eq!(names(&all), names(&speech(0, 21)));
}

#[test]
fn a_round_past_the_final_take_decodes_the_whole_take() {
    // A release that queued behind a slow start cut the take before audio
    // the last round already heard.
    let mut r = Rounds::default();
    r.absorb(&WINDOWS, 90_000, speech(0, 90));
    assert_eq!(r.release(&WINDOWS, 60_000, None), Release::Whole);
}

#[test]
fn stitched_words_keep_the_segments_the_guards_judge() {
    // Speech, a pause over 700 ms, then a filler the guards drop alone.
    let mut words = vec![
        word("Book", 0, 300),
        word("the", 350, 500),
        word("room.", 550, 900),
    ];
    words.extend([
        word("Thanks", 2_000, 2_300),
        word("for", 2_350, 2_500),
        word("watching!", 2_550, 3_000),
    ]);
    let segments = segments(&words);
    assert_eq!(
        segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(),
        ["Book the room.", "Thanks for watching!"]
    );
    let result = dettivo_engine_proto::RecognizeResult {
        text: text(&words),
        language: "en".into(),
        segments,
        duration_ms: 3_000,
        backend: dettivo_engine_proto::Backend::Vulkan,
    };
    assert_eq!(crate::guards::clean(&result, &[]), "Book the room.");
}
