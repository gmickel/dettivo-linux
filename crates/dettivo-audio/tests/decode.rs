//! R1: every import content type decodes to 16 kHz mono in blocks with
//! the expected length and level, and a container that is not what the
//! upload declares is refused naming both.

use std::path::{Path, PathBuf};

use dettivo_audio::decode::{self, DecodeError, Decoder};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/decode")
        .join(name)
}

fn rms(samples: &[i16]) -> f64 {
    (samples
        .iter()
        .map(|s| (f64::from(*s) / 32_768.0).powi(2))
        .sum::<f64>()
        / samples.len().max(1) as f64)
        .sqrt()
}

/// One row per content type: the fixture, its type, the container
/// Symphonia names, and the length tolerance (a lossy encoder pads).
const ROWS: &[(&str, &str, &str, f64)] = &[
    ("tone.wav", "audio/wav", "wave", 0.002),
    ("tone.aiff", "audio/aiff", "aiff", 0.002),
    ("tone.caf", "audio/caf", "caf", 0.002),
    ("tone.caf", "audio/x-caf", "caf", 0.002),
    ("tone.flac", "audio/flac", "flac", 0.002),
    ("tone.mp3", "audio/mpeg", "mp3", 0.06),
    ("tone.m4a", "audio/m4a", "isomp4", 0.06),
    ("tone.m4a", "audio/mp4", "isomp4", 0.06),
    ("tone.aac", "audio/aac", "aac", 0.12),
    ("tone.ogg", "audio/ogg", "ogg", 0.06),
];

#[test]
fn every_content_type_decodes_to_sixteen_k_mono_in_blocks() {
    for (name, content_type, container, tolerance) in ROWS {
        let mut decoder = Decoder::open(&fixture(name), content_type)
            .unwrap_or_else(|e| panic!("{name} as {content_type}: {e}"));
        let info = decoder.info().clone();
        assert_eq!(info.container, *container, "{name}");
        assert_eq!(info.content_type, *content_type);
        let mut blocks = 0usize;
        let mut samples = Vec::new();
        while let Some(block) = decoder.next_block().unwrap() {
            blocks += 1;
            samples.extend(block);
        }
        assert!(blocks > 1, "{name}: streamed in {blocks} block");
        let expected = 16_000.0;
        let ratio = samples.len() as f64 / expected;
        assert!(
            (ratio - 1.0).abs() <= *tolerance,
            "{name}: {} samples (tolerance {tolerance})",
            samples.len()
        );
        // The tone is a 440 Hz sine at 0.5: RMS 0.354. The middle of the
        // clip is measured so an encoder's fade-in never counts.
        let mid = &samples[4000..12_000];
        let level = rms(mid);
        assert!((level - 0.354).abs() < 0.03, "{name}: rms {level}");
        if let Some(d) = info.duration_ms {
            assert!((900..=1100).contains(&d), "{name}: probe says {d} ms");
        }
    }
}

#[test]
fn a_mismatched_container_is_refused_naming_the_declared_and_detected_types() {
    let err = Decoder::open(&fixture("tone.mp3"), "audio/flac")
        .err()
        .expect("refused");
    assert_eq!(
        err,
        DecodeError::Mismatch {
            declared: "audio/flac".into(),
            detected: "mp3".into()
        }
    );
    assert_eq!(
        err.to_string(),
        "the upload declares audio/flac but its container is mp3"
    );
    let unknown = Decoder::open(&fixture("tone.wav"), "text/csv")
        .err()
        .expect("refused");
    assert_eq!(unknown, DecodeError::UnsupportedType("text/csv".into()));
    let dir = tempfile::tempdir().unwrap();
    let garbage = dir.path().join("x.wav");
    std::fs::write(&garbage, b"not audio at all, just bytes").unwrap();
    assert!(matches!(
        Decoder::open(&garbage, "audio/wav").err().expect("refused"),
        DecodeError::NoContainer(_)
    ));
}

#[test]
fn decode_to_wav_writes_sixteen_k_mono_and_honours_the_length_limit() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.wav");
    let (info, written) =
        decode::decode_to_wav(&fixture("tone.ogg"), "audio/ogg", &out, Some(5)).unwrap();
    assert_eq!(info.channels, 2);
    assert_eq!(info.sample_rate, 44_100);
    let reader = hound::WavReader::open(&out).unwrap();
    assert_eq!(reader.spec().sample_rate, 16_000);
    assert_eq!(reader.spec().channels, 1);
    assert_eq!(u64::from(reader.len()), written);
    let limit = dir.path().join("limit.wav");
    let err =
        decode::decode_to_wav(&fixture("tone.wav"), "audio/wav", &limit, Some(0)).unwrap_err();
    assert_eq!(err, DecodeError::TooLong { max_seconds: 0 });
    assert!(!limit.exists(), "a refused decode leaves no file");
    assert_eq!(decode::content_types().count(), 11);
}
