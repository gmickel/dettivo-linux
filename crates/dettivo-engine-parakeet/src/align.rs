//! The pure parts of recognition: which languages a model accepts, how
//! long audio is cut into overlapping chunks and stitched back by word
//! timestamps, and how words become segments.

use dettivo_engine_proto::{Segment, Word};

/// The languages Parakeet TDT 0.6B v3 was trained on (25 European
/// languages), as ISO 639-1 codes.
pub const LANGUAGES_V3: &[&str] = &[
    "bg", "hr", "cs", "da", "nl", "en", "et", "fi", "fr", "de", "el", "hu", "it", "lv", "lt", "mt",
    "pl", "pt", "ro", "sk", "sl", "es", "sv", "ru", "uk",
];

/// The languages a model accepts, from its `general.name`; `None` when the
/// model is not one this engine knows the training set of (any language
/// is then passed through).
pub fn languages_for(model_name: &str) -> Option<&'static [&'static str]> {
    let name = model_name.to_lowercase();
    if name.contains("tdt-0.6b-v2") || name.contains("tdt_ctc") && name.contains("110m") {
        Some(&["en"])
    } else if name.contains("tdt-0.6b-v3") || name.contains("parakeet-ultra") {
        // Parakeet Ultra is Moondream's post-train of v3: same vocabulary,
        // same 25 languages.
        Some(LANGUAGES_V3)
    } else {
        None
    }
}

/// `auto` or one of `languages` passes; anything else is the message the
/// protocol's `bad_request` carries, naming the model's languages.
pub fn check_language(language: &str, languages: Option<&[&str]>) -> Result<(), String> {
    let Some(list) = languages else {
        return Ok(());
    };
    if language == "auto" || list.contains(&language) {
        return Ok(());
    }
    Err(format!(
        "language {language:?} is not one of this model's languages: {}",
        list.join(", ")
    ))
}

/// Audio at or under this length goes through in one piece.
pub const SINGLE_PASS_SAMPLES: usize = 16_000 * 90;
/// Longer audio is cut into windows of this length...
pub const CHUNK_SAMPLES: usize = 16_000 * 60;
/// ...overlapping by this much, so a word on a cut is whole in one of them.
pub const OVERLAP_SAMPLES: usize = 16_000 * 2;

/// The `[start, end)` sample ranges recognition runs over.
pub fn chunks(len: usize) -> Vec<(usize, usize)> {
    if len <= SINGLE_PASS_SAMPLES {
        return vec![(0, len)];
    }
    let mut out = Vec::new();
    let mut start = 0;
    loop {
        let end = (start + CHUNK_SAMPLES).min(len);
        out.push((start, end));
        if end == len {
            return out;
        }
        start = end - OVERLAP_SAMPLES;
    }
}

/// Words from consecutive chunks (each already offset to absolute time),
/// merged at the midpoint of every overlap: the earlier chunk keeps the
/// words that start before it, the later chunk the rest.
pub fn stitch(chunks: &[(usize, usize, Vec<Word>)]) -> Vec<Word> {
    let mut out: Vec<Word> = Vec::new();
    for (i, (start, _end, words)) in chunks.iter().enumerate() {
        let lower = if i == 0 {
            0
        } else {
            let prev_end = chunks[i - 1].1;
            ms(*start + (prev_end - *start) / 2)
        };
        let upper = chunks
            .get(i + 1)
            .map(|(next_start, _, _)| ms(*next_start + (chunks[i].1 - *next_start) / 2))
            .unwrap_or(u64::MAX);
        out.extend(
            words
                .iter()
                .filter(|w| w.start_ms >= lower && w.start_ms < upper)
                .cloned(),
        );
    }
    out
}

fn ms(samples: usize) -> u64 {
    (samples as u64 * 1000) / 16_000
}

/// A pause longer than this starts a new segment...
pub const SEGMENT_GAP_MS: u64 = 700;
/// ...and so does a segment reaching this length.
pub const SEGMENT_MAX_MS: u64 = 30_000;

/// Groups words into segments at pauses and at the length cap.
pub fn segments(words: &[Word]) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    for word in words {
        let split = match out.last() {
            None => true,
            Some(last) => {
                word.start_ms.saturating_sub(last.end_ms) > SEGMENT_GAP_MS
                    || word.end_ms.saturating_sub(last.start_ms) > SEGMENT_MAX_MS
            }
        };
        if split {
            out.push(Segment {
                start_ms: word.start_ms,
                end_ms: word.end_ms,
                text: word.text.clone(),
                words: vec![word.clone()],
            });
        } else if let Some(last) = out.last_mut() {
            last.end_ms = word.end_ms.max(last.end_ms);
            last.text.push(' ');
            last.text.push_str(&word.text);
            last.words.push(word.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(start_ms: u64, text: &str) -> Word {
        Word {
            start_ms,
            end_ms: start_ms + 200,
            text: text.into(),
            confidence: 0.9,
        }
    }

    #[test]
    fn languages_follow_the_model_name() {
        assert_eq!(
            languages_for("nvidia/parakeet-tdt-0.6b-v2"),
            Some(&["en"][..])
        );
        assert_eq!(
            languages_for("nvidia/parakeet-tdt-0.6b-v3"),
            Some(LANGUAGES_V3)
        );
        assert_eq!(
            languages_for("moondream/parakeet-ultra"),
            Some(LANGUAGES_V3)
        );
        assert_eq!(languages_for("nvidia/parakeet-ctc-1.1b"), None);
        assert!(check_language("auto", Some(&["en"])).is_ok());
        assert!(check_language("de", Some(LANGUAGES_V3)).is_ok());
        assert!(check_language("ja", None).is_ok());
        let err = check_language("de", Some(&["en"])).unwrap_err();
        assert!(
            err.contains("\"de\"") && err.ends_with("languages: en"),
            "{err}"
        );
    }

    #[test]
    fn short_audio_is_one_chunk_and_long_audio_overlaps() {
        assert_eq!(chunks(16_000 * 30), vec![(0, 16_000 * 30)]);
        let c = chunks(16_000 * 150);
        assert_eq!(
            c,
            vec![
                (0, 16_000 * 60),
                (16_000 * 58, 16_000 * 118),
                (16_000 * 116, 16_000 * 150),
            ]
        );
    }

    #[test]
    fn stitching_keeps_each_word_once_from_the_chunk_it_is_whole_in() {
        // Chunks of 0..60 s and 58..118 s overlap on 58..60 s; the midpoint
        // is 59 s. A word at 58.5 s belongs to the first chunk, one at 59.5 s
        // to the second, and the second chunk's copy of the first is dropped.
        let first = vec![word(1_000, "a"), word(58_500, "b"), word(59_500, "c")];
        let second = vec![word(58_500, "b"), word(59_500, "c"), word(70_000, "d")];
        let out = stitch(&[(0, 16_000 * 60, first), (16_000 * 58, 16_000 * 118, second)]);
        let texts: Vec<&str> = out.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["a", "b", "c", "d"]);
    }

    #[test]
    fn segments_split_at_pauses_and_at_the_length_cap() {
        let words = vec![
            word(0, "one"),
            word(300, "two"),
            word(2_000, "three"),
            word(2_300, "four"),
        ];
        let s = segments(&words);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].text, "one two");
        assert_eq!((s[0].start_ms, s[0].end_ms), (0, 500));
        assert_eq!(s[1].words.len(), 2);
        let long: Vec<Word> = (0..120).map(|i| word(i * 400, "w")).collect();
        let s = segments(&long);
        assert!(s.len() >= 2, "{}", s.len());
        assert!(s.iter().all(|x| x.end_ms - x.start_ms <= SEGMENT_MAX_MS));
        assert_eq!(segments(&[]).len(), 0);
    }
}
