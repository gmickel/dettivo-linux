use super::*;
use dettivo_speech::diarize::DiarizeEngine;
use dettivo_speech::supervisor::DIARIZE_BINARY;

/// A long meeting's spans in one `embed` reply outgrew the 1 MiB frame
/// header, and the engine could not send it. The client asks in batches,
/// each with only the audio its spans cover, and gets every vector back in
/// order.
#[test]
fn embed_answers_a_long_meetings_spans_in_batches() {
    let dir = fake_engine_dir(DIARIZE_BINARY);
    let supervisor = Supervisor::new(settings(dir.path(), Duration::from_secs(60)));
    let engine = DiarizeEngine::new(
        supervisor.clone(),
        DIARIZE_BINARY,
        "/models/fake".into(),
        None,
    );
    let pcm = vec![0i16; 16 * 3_000_000];
    let spans: Vec<(u64, u64)> = (0..2000u64)
        .map(|i| (i * 1_400 + 100, i * 1_400 + 1_300))
        .collect();
    let vectors = engine.embed(&pcm, &spans, Duration::from_secs(30)).unwrap();
    assert_eq!(vectors.len(), spans.len());
    for (vector, &(start, end)) in vectors.iter().zip(&spans) {
        let v = vector.as_ref().unwrap();
        assert_eq!(
            v[1] - v[0],
            (end - start) as f32,
            "each span keeps its length"
        );
        assert!(
            v[1] <= v[2],
            "{start}..{end} lies inside the audio sent with it: {:?}",
            &v[..3]
        );
        assert!(
            v[2] < 200_000.0,
            "a batch sends only the audio its spans cover, not the whole track"
        );
    }
    supervisor.shutdown();
}
