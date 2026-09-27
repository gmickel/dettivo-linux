//! Span embeddings on the real model: two spans of one speaker in the
//! two-speaker fixture sit closer than spans of different speakers, every
//! vector is unit length, and a span too short or past the end gets none.
//! The ranges follow the fixture's diarization (speaker A 689..3946 and
//! 7287..11371 ms, speaker B 4267..7051 and 16315..18425 ms).

use std::path::Path;

use super::*;
use crate::engine::downloaded_model;
use crate::provider::Selection;

fn span(start_ms: u64, end_ms: u64) -> Span {
    Span { start_ms, end_ms }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[test]
fn spans_of_one_speaker_embed_closer_than_spans_of_two() {
    let Some(model) = downloaded_model() else {
        eprintln!("skip: the calibrated diarization model is not downloaded");
        return;
    };
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dettivo-qa/fixtures/diarization/two-speakers.wav");
    let pcm = dettivo_engine_proto::host::read_wav(&fixture).unwrap();
    let diarizer = Diarizer::open(&model, 1, Selection::cpu("test")).unwrap();
    let params = EmbedParams {
        spans: vec![
            span(1000, 3500),
            span(7500, 10_500),
            span(4500, 6800),
            span(16_500, 18_300),
            span(1000, 1100),
            span(60_000, 61_000),
        ],
    };
    let result = diarizer.embed_spans(&pcm, &params).unwrap();
    let e = &result.embeddings;
    assert_eq!(e.len(), 6);
    assert_eq!(e[4], None, "100 ms is too short");
    assert_eq!(e[5], None, "past the end of the samples");
    let v: Vec<&Vec<f32>> = e[..4].iter().map(|v| v.as_ref().unwrap()).collect();
    for x in &v {
        assert_eq!(x.len(), v[0].len());
        assert!((cosine(x, x) - 1.0).abs() < 1e-4, "unit length");
    }
    let same_a = cosine(v[0], v[1]);
    let same_b = cosine(v[2], v[3]);
    let across = [(0, 2), (0, 3), (1, 2), (1, 3)].map(|(i, j)| cosine(v[i], v[j]));
    let worst = across.iter().copied().fold(f32::MIN, f32::max);
    assert!(
        same_a > worst && same_b > worst,
        "{same_a} {same_b} {across:?}"
    );
    let again = diarizer.embed_spans(&pcm, &params).unwrap();
    assert_eq!(again, result, "the kept extractor answers the same");
}

#[test]
fn spans_clamp_to_the_samples_and_vectors_normalise() {
    let len = 16_000;
    assert_eq!(range(&span(0, 500), len), Some(0..8000));
    assert_eq!(range(&span(600, 5000), len), None, "clamped to 400 ms");
    assert_eq!(range(&span(500, 1000), len), Some(8000..16_000));
    assert_eq!(range(&span(900, 100), len), None);
    assert_eq!(normalised(vec![3.0, 4.0]), Some(vec![0.6, 0.8]));
    assert_eq!(normalised(vec![0.0, 0.0]), None);
}
