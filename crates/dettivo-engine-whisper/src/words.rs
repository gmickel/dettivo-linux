//! Timed words from whisper.cpp's tokens (ADR 0074). A word is a run of
//! text-token bytes between whitespace, so the words of a segment are its
//! text split on whitespace; sub-word tokens merge into the word they
//! continue. Times come from the tokens' DTW timestamps when the model
//! has an alignment-head preset, and from whisper.cpp's plain token
//! timestamps otherwise.

use std::io::Read;
use std::path::Path;

use dettivo_engine_proto::Word;
use whisper_rs::DtwModelPreset;

/// One text token of a segment, times in centiseconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// The token's bytes (a leading space opens a new word).
    pub bytes: Vec<u8>,
    /// Plain start.
    pub t0: i64,
    /// Plain end.
    pub t1: i64,
    /// DTW timestamp.
    pub dtw: i64,
    /// Probability.
    pub p: f32,
}

/// Where word times come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timing {
    /// whisper.cpp's token timestamps (`token_timestamps`).
    Plain,
    /// DTW over the alignment heads.
    Dtw,
}

/// The alignment-head preset for the ggml model at `path`, read from its
/// header: the layer counts, the mel bins and the vocabulary tell every
/// catalogue model apart. `None` for anything else (large-v1 and v2
/// share one shape and are not told apart).
pub fn dtw_preset(path: &Path) -> Option<DtwModelPreset> {
    let mut head = [0u8; 44];
    std::fs::File::open(path).ok()?.read_exact(&mut head).ok()?;
    let field = |i: usize| {
        i32::from_le_bytes([
            head[4 * i],
            head[4 * i + 1],
            head[4 * i + 2],
            head[4 * i + 3],
        ])
    };
    if field(0) as u32 != 0x6767_6d6c {
        return None;
    }
    let (vocab, audio_layers, text_layers, mels) = (field(1), field(5), field(9), field(10));
    let english = vocab == 51_864;
    Some(match (audio_layers, english) {
        (4, true) => DtwModelPreset::TinyEn,
        (4, false) => DtwModelPreset::Tiny,
        (6, true) => DtwModelPreset::BaseEn,
        (6, false) => DtwModelPreset::Base,
        (12, true) => DtwModelPreset::SmallEn,
        (12, false) => DtwModelPreset::Small,
        (24, true) => DtwModelPreset::MediumEn,
        (24, false) => DtwModelPreset::Medium,
        (32, false) if text_layers == 4 => DtwModelPreset::LargeV3Turbo,
        (32, false) if mels == 128 => DtwModelPreset::LargeV3,
        _ => return None,
    })
}

/// One segment as whisper.cpp returned it: its span and its text tokens.
#[derive(Debug, Clone, PartialEq)]
pub struct Spoken {
    /// Start and end in centiseconds.
    pub span: (i64, i64),
    /// The text tokens.
    pub tokens: Vec<Token>,
}

/// One DTW step: the alignment runs on 20 ms audio frames.
const DTW_FRAME_CS: i64 = 2;

/// The timed words of every segment and each segment's span, snapped to
/// its words when it has any. A word is a run of non-whitespace token
/// bytes; its confidence is the mean probability of its tokens.
///
/// DTW: a word starts at its first token's DTW time and ends one frame
/// after its last token's, never past the next word's start. The words
/// are not held to whisper's segment timestamps, which drift by seconds
/// in long audio (ADR 0074). Plain: a word runs from its first token's
/// start to its last token's end, inside its segment.
pub fn timed(segments: &[Spoken], timing: Timing, audio_cs: i64) -> Vec<((i64, i64), Vec<Word>)> {
    let runs: Vec<Vec<Run>> = segments.iter().map(|s| runs(&s.tokens)).collect();
    let starts: Vec<i64> = segments
        .iter()
        .zip(&runs)
        .flat_map(|(s, r)| r.iter().map(|w| s.tokens[w.first].dtw))
        .collect();
    let mut index = 0;
    let mut floor = 0;
    let mut out = Vec::with_capacity(segments.len());
    for (segment, runs) in segments.iter().zip(&runs) {
        let (lo, hi) = (segment.span.0, segment.span.1.max(segment.span.0));
        let mut words = Vec::with_capacity(runs.len());
        for run in runs {
            index += 1;
            let tokens = &segment.tokens[run.first..=run.last];
            let (first, last) = (&tokens[0], &tokens[tokens.len() - 1]);
            let (start, end) = match timing {
                Timing::Plain => {
                    let start = first.t0.clamp(lo.max(floor), hi);
                    (start, last.t1.clamp(start, hi))
                }
                Timing::Dtw => {
                    let start = first.dtw.clamp(floor, audio_cs);
                    let following = starts.get(index).copied().unwrap_or(audio_cs);
                    let end = (last.dtw + DTW_FRAME_CS).min(following).min(audio_cs);
                    (start, end.max(start))
                }
            };
            floor = start;
            let p = tokens.iter().map(|t| f64::from(t.p)).sum::<f64>() / tokens.len() as f64;
            words.push(Word {
                start_ms: start.max(0) as u64 * 10,
                end_ms: end.max(0) as u64 * 10,
                text: String::from_utf8_lossy(&run.bytes).into_owned(),
                confidence: p.clamp(1e-6, 1.0),
            });
        }
        let span = match (words.first(), words.last()) {
            (Some(a), Some(b)) => ((a.start_ms / 10) as i64, (b.end_ms / 10) as i64),
            _ => (lo, hi),
        };
        out.push((span, words));
    }
    out
}

/// A word's bytes and its first and last token.
struct Run {
    bytes: Vec<u8>,
    first: usize,
    last: usize,
}

/// Each run of non-whitespace bytes over `tokens`.
fn runs(tokens: &[Token]) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    let mut open = false;
    for (i, t) in tokens.iter().enumerate() {
        for &b in &t.bytes {
            if b.is_ascii_whitespace() {
                open = false;
                continue;
            }
            match out.last_mut() {
                Some(run) if open => {
                    run.bytes.push(b);
                    run.last = i;
                }
                _ => {
                    out.push(Run {
                        bytes: vec![b],
                        first: i,
                        last: i,
                    });
                    open = true;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A token from `(text, t0, t1, dtw, p)`.
    fn tok(spec: (&str, i64, i64, i64, f32)) -> Token {
        Token {
            bytes: spec.0.as_bytes().to_vec(),
            t0: spec.1,
            t1: spec.2,
            dtw: spec.3,
            p: spec.4,
        }
    }

    fn spelled(words: &[Word]) -> Vec<(&str, u64, u64)> {
        words
            .iter()
            .map(|w| (w.text.as_str(), w.start_ms, w.end_ms))
            .collect()
    }

    fn spoken(span: (i64, i64), tokens: &[(&str, i64, i64, i64, f32)]) -> Spoken {
        Spoken {
            span,
            tokens: tokens.iter().copied().map(tok).collect(),
        }
    }

    fn spans(out: &[((i64, i64), Vec<Word>)]) -> Vec<(i64, i64)> {
        out.iter().map(|(span, _)| *span).collect()
    }

    #[test]
    fn sub_word_tokens_merge_and_each_timing_reads_its_own_times() {
        let segment = spoken(
            (0, 120),
            &[
                (" Amer", 10, 20, 12, 0.5),
                ("icans", 20, 40, 30, 1.0),
                (",", 40, 41, 41, 1.0),
                (" ask", 90, 100, 95, 0.8),
            ],
        );
        let plain = timed(std::slice::from_ref(&segment), Timing::Plain, 200);
        assert_eq!(
            spelled(&plain[0].1),
            [("Americans,", 100, 410), ("ask", 900, 1000)]
        );
        assert!((plain[0].1[0].confidence - 2.5 / 3.0).abs() < 1e-6);
        let dtw = timed(&[segment], Timing::Dtw, 200);
        assert_eq!(
            spelled(&dtw[0].1),
            [("Americans,", 120, 430), ("ask", 950, 970)]
        );
        assert_eq!(spans(&dtw), [(12, 97)], "the span snaps to its words");
    }

    /// Plain words stay inside their segment; DTW words keep their own
    /// times across segments, in order, never past the next start.
    #[test]
    fn plain_words_stay_in_their_segment_and_dtw_words_stay_in_order() {
        let segments = [
            spoken((10, 50), &[(" a", -5, 30, 60, 0.0)]),
            spoken((50, 60), &[(" b", 20, 999, 55, 1.0)]),
            spoken((60, 70), &[(" ", 60, 61, 60, 1.0)]),
        ];
        let plain = timed(&segments, Timing::Plain, 300);
        assert_eq!(spelled(&plain[0].1), [("a", 100, 300)]);
        assert_eq!(spelled(&plain[1].1), [("b", 500, 600)]);
        assert!(plain[0].1[0].confidence > 0.0, "confidence stays in (0, 1]");
        let dtw = timed(&segments, Timing::Dtw, 300);
        assert_eq!(spelled(&dtw[0].1), [("a", 600, 600)]);
        assert_eq!(spelled(&dtw[1].1), [("b", 600, 600)]);
        assert!(dtw[2].1.is_empty());
        assert_eq!(
            spans(&dtw)[2],
            (60, 70),
            "a segment without words keeps its span"
        );
    }

    fn header(fields: [i32; 11]) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        let bytes: Vec<u8> = std::iter::once(0x6767_6d6c_i32)
            .chain(fields)
            .flat_map(i32::to_le_bytes)
            .collect();
        std::io::Write::write_all(&mut f, &bytes).unwrap();
        f
    }

    #[test]
    fn the_header_names_the_preset_of_every_catalogue_shape() {
        let shape = |vocab, audio, text, mels| [vocab, 1500, 0, 0, audio, 448, 0, 0, text, mels, 1];
        let cases = [
            (shape(51_864, 4, 4, 80), Some("TinyEn")),
            (shape(51_865, 6, 6, 80), Some("Base")),
            (shape(51_864, 12, 12, 80), Some("SmallEn")),
            (shape(51_865, 24, 24, 80), Some("Medium")),
            (shape(51_866, 32, 32, 128), Some("LargeV3")),
            (shape(51_866, 32, 4, 128), Some("LargeV3Turbo")),
            (shape(51_865, 32, 32, 80), None),
        ];
        for (fields, want) in cases {
            let file = header(fields);
            let got = dtw_preset(file.path()).map(|p| format!("{p:?}"));
            assert_eq!(got.as_deref(), want, "{fields:?}");
        }
        let bad = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            bad.path(),
            b"GGUF and not ggml at all, long enough for a header",
        )
        .unwrap();
        assert!(dtw_preset(bad.path()).is_none(), "another format");
        std::fs::write(bad.path(), b"lmgg").unwrap();
        assert!(dtw_preset(bad.path()).is_none(), "a short file");
    }
}
