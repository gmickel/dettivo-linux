//! R2 and R3 through the job: a scripted engine records the chunks it is
//! given, the silent chunk never reaches it, fillers are dropped, the
//! merged result matches the three-chunk golden, a crash is retried once
//! and a second one fails the job with the chunk named while the earlier
//! chunks are kept, a cancel between chunks leaves the finished chunks,
//! and an engine without timestamps is refused naming its capability.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dettivo_engine_proto::{Backend, RecognizeResult, Segment};
use dettivo_speech::{Capabilities, EngineError, PreloadSource, RecognizeRequest, SttEngine};
use dettivo_transcribe::merger::{self, ChunkResult};
use dettivo_transcribe::{JobError, Progress, Request, SAMPLE_RATE, Settings, Stage, run};

/// A scripted engine: one answer per call, in order.
struct Scripted {
    answers: Mutex<Vec<Result<RecognizeResult, EngineError>>>,
    calls: Mutex<Vec<usize>>,
    timestamps: bool,
    gate: Option<std::sync::Arc<std::sync::Barrier>>,
}

impl Scripted {
    fn new(answers: Vec<Result<RecognizeResult, EngineError>>) -> Self {
        Self {
            answers: Mutex::new(answers),
            calls: Mutex::new(Vec::new()),
            timestamps: true,
            gate: None,
        }
    }
}

fn result(segments: Vec<Segment>) -> RecognizeResult {
    RecognizeResult {
        text: segments
            .iter()
            .map(|s| s.text.clone())
            .collect::<Vec<_>>()
            .join(" "),
        language: "en".into(),
        segments,
        duration_ms: 0,
        backend: Backend::Cpu,
    }
}

fn seg(start: u64, end: u64, text: &str) -> Segment {
    Segment {
        start_ms: start,
        end_ms: end,
        text: text.into(),
        words: Vec::new(),
    }
}

impl SttEngine for Scripted {
    fn provider(&self) -> &str {
        "scripted"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_timestamps: self.timestamps,
            supports_streaming: false,
            supported_languages: Vec::new(),
            max_duration_seconds: 3600,
            supports_custom_vocabulary: true,
            supports_model_deletion: false,
        }
    }
    fn preload(&self, _: PreloadSource) -> Result<(), EngineError> {
        Ok(())
    }
    fn recognize(
        &self,
        request: RecognizeRequest,
        _: Duration,
    ) -> Result<RecognizeResult, EngineError> {
        if let Some(gate) = &self.gate {
            gate.wait();
            gate.wait();
        }
        self.calls.lock().unwrap().push(request.pcm.len());
        let mut answers = self.answers.lock().unwrap();
        if answers.is_empty() {
            return Ok(result(Vec::new()));
        }
        answers.remove(0)
    }
    fn cancel(&self) {}
    fn backend(&self) -> Option<Backend> {
        Some(Backend::Cpu)
    }
}

/// `seconds` of a loud square wave.
fn loud(seconds: u64) -> Vec<i16> {
    (0..seconds * SAMPLE_RATE)
        .map(|i| if i % 2 == 0 { 8000 } else { -8000 })
        .collect()
}

fn settings() -> Settings {
    Settings {
        chunk_seconds: 10,
        overlap_seconds: 2,
        safety_margin_seconds: 1,
        ..Settings::default()
    }
}

fn request() -> Request {
    Request {
        language: "en".into(),
        prompt: None,
        from_system: false,
    }
}

fn collect(progress: &mut Vec<Progress>) -> impl FnMut(Progress) + '_ {
    move |p| progress.push(p)
}

#[test]
fn chunks_reach_the_engine_in_order_with_progress_and_a_silent_chunk_is_skipped() {
    // 25 s: chunk 0 = 0..~10 s, chunk 1 = ~8..~18 s, chunk 2 = ~16..25 s;
    // the middle chunk is silence and never reaches the engine.
    let mut pcm = loud(25);
    for s in &mut pcm[7 * SAMPLE_RATE as usize..18 * SAMPLE_RATE as usize] {
        *s = 0;
    }
    let engine = Scripted::new(vec![
        Ok(result(vec![seg(500, 2_000, "first chunk")])),
        Ok(result(vec![
            seg(3_000, 4_000, "Thanks for watching!"),
            seg(5_000, 6_000, "third chunk"),
        ])),
    ]);
    let mut progress = Vec::new();
    let transcript = run(
        &mut pcm,
        &engine,
        &request(),
        &settings(),
        &AtomicBool::new(false),
        &mut collect(&mut progress),
    )
    .unwrap();
    assert_eq!(transcript.text, "first chunk third chunk");
    assert_eq!(transcript.notice, None);
    assert_eq!(transcript.duration_ms, 25_000);
    assert_eq!(transcript.segments.len(), 2);
    assert_eq!(
        engine.calls.lock().unwrap().len(),
        2,
        "the silent chunk was skipped"
    );
    let stages: Vec<(Stage, u32, u32)> = progress
        .iter()
        .map(|p| (p.stage, p.chunks_done, p.chunks_total))
        .collect();
    assert_eq!(
        stages,
        vec![
            (Stage::Transcribing, 0, 3),
            (Stage::Transcribing, 1, 3),
            (Stage::Transcribing, 2, 3),
            (Stage::Transcribing, 3, 3),
            (Stage::Merging, 3, 3),
        ]
    );
    assert!((progress[2].fraction() - 2.0 / 3.0).abs() < 1e-9);

    let mut silence = vec![0i16; 15 * SAMPLE_RATE as usize];
    let quiet = Scripted::new(vec![Ok(result(vec![seg(0, 1000, "invented")]))]);
    let t = run(
        &mut silence,
        &quiet,
        &request(),
        &settings(),
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(t.text, "");
    assert_eq!(t.notice.as_deref(), Some("silent"));
    assert!(
        quiet.calls.lock().unwrap().is_empty(),
        "silence never reaches the engine"
    );
}

/// A brief utterance in a long silent chunk reaches the engine: the
/// floor is judged per frame, never over the whole chunk's average.
#[test]
fn a_brief_utterance_in_a_long_silence_reaches_the_engine() {
    let mut pcm = vec![0i16; 300 * SAMPLE_RATE as usize];
    for s in &mut pcm[100 * SAMPLE_RATE as usize..101 * SAMPLE_RATE as usize] {
        *s = 3_277;
    }
    let engine = Scripted::new(vec![Ok(result(vec![seg(400, 1400, "yes")]))]);
    let t = run(
        &mut pcm,
        &engine,
        &request(),
        &Settings {
            chunk_seconds: 300,
            ..Settings::default()
        },
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(t.text, "yes");
    assert_eq!(t.segments[0].start_ms, 100_000);
    assert_eq!(t.notice, None);
    assert_eq!(
        engine.calls.lock().unwrap().len(),
        1,
        "the utterance reached the engine"
    );
}

#[test]
fn a_crash_is_retried_once_and_a_second_one_fails_the_chunk_by_index_keeping_earlier_text() {
    let mut pcm = loud(25);
    let engine = Scripted::new(vec![
        Ok(result(vec![seg(500, 2_000, "kept")])),
        Err(EngineError::Crashed("secret".into())),
        Ok(result(vec![seg(2_500, 4_000, "after retry")])),
        Err(EngineError::Crashed("secret".into())),
        Err(EngineError::Crashed("secret".into())),
    ]);
    let err = run(
        &mut pcm,
        &engine,
        &request(),
        &settings(),
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap_err();
    match err {
        JobError::Chunk {
            index,
            total,
            message,
            partial,
        } => {
            assert_eq!((index, total), (2, 3));
            assert_eq!(message, "engine crashed");
            assert_eq!(partial.text, "kept after retry");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(engine.calls.lock().unwrap().len(), 5);
}

#[test]
fn cancel_during_the_last_chunk_keeps_its_text_without_completing() {
    let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let mut engine = Scripted::new(vec![Ok(result(vec![seg(0, 900, "last words retained")]))]);
    engine.gate = Some(gate.clone());
    let flag = cancel.clone();
    let worker = std::thread::spawn(move || {
        run(
            &mut loud(1),
            &engine,
            &request(),
            &settings(),
            &flag,
            &mut |_| {},
        )
    });
    gate.wait();
    cancel.store(true, Ordering::SeqCst);
    gate.wait();
    match worker.join().unwrap().unwrap_err() {
        JobError::Cancelled { partial } => {
            assert_eq!(partial.text, "last words retained");
            assert_eq!(partial.duration_ms, 1000);
            assert_eq!(partial.segments.len(), 1);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn cancel_between_chunks_keeps_the_finished_chunks_and_no_timestamps_names_the_capability() {
    let mut pcm = loud(25);
    let engine = Scripted::new(vec![Ok(result(vec![seg(500, 2_000, "one")]))]);
    let cancel = AtomicBool::new(false);
    let mut seen = 0;
    let err = run(
        &mut pcm,
        &engine,
        &request(),
        &settings(),
        &cancel,
        &mut |p| {
            if p.chunks_done == 1 {
                cancel.store(true, Ordering::SeqCst);
            }
            seen += 1;
        },
    )
    .unwrap_err();
    match err {
        JobError::Cancelled { partial } => assert_eq!(partial.text, "one"),
        other => panic!("{other:?}"),
    }
    assert_eq!(engine.calls.lock().unwrap().len(), 1);

    let mut without = Scripted::new(Vec::new());
    without.timestamps = false;
    let err = run(
        &mut pcm,
        &without,
        &request(),
        &settings(),
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(
        err,
        JobError::NoTimestamps {
            provider: "scripted".into()
        }
    );
    assert!(err.to_string().contains("supports_timestamps"));
    // An engine that claims timestamps but returns text without segments.
    let bare = Scripted::new(vec![Ok(RecognizeResult {
        text: "words but no times".into(),
        language: "en".into(),
        segments: Vec::new(),
        duration_ms: 0,
        backend: Backend::Cpu,
    })]);
    let err = run(
        &mut pcm,
        &bare,
        &request(),
        &settings(),
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap_err();
    assert!(matches!(err, JobError::NoTimestamps { .. }));
}

/// The three-chunk overlap fixture merged against its golden (R2):
/// duplicate words across the overlap dropped, boundaries snapped to the
/// merged words, segment order preserved, word timestamps kept.
#[test]
fn the_three_chunk_overlap_fixture_matches_its_golden() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/overlap");
    let input: Vec<ChunkResult> = {
        let raw: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("three-chunks.json")).unwrap())
                .unwrap();
        raw["chunks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| ChunkResult {
                start_ms: c["start_ms"].as_u64().unwrap(),
                end_ms: c["end_ms"].as_u64().unwrap(),
                segments: serde_json::from_value(c["segments"].clone()).unwrap(),
            })
            .collect()
    };
    let merged = merger::merge(&input);
    let got = serde_json::to_string_pretty(&serde_json::json!({
        "text": merger::text_of(&merged),
        "segments": merged,
    }))
    .unwrap()
        + "\n";
    let golden_path = dir.join("three-chunks.golden.json");
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::write(&golden_path, &got).unwrap();
    }
    let golden = std::fs::read_to_string(&golden_path).unwrap();
    assert_eq!(got, golden, "the merged fixture drifted from its golden");
    let text = merger::text_of(&merged);
    assert_eq!(text.matches("review").count(), 1, "{text}");
    assert_eq!(text.matches("now.").count(), 1, "{text}");
    let starts: Vec<u64> = merged.iter().map(|s| s.start_ms).collect();
    let mut sorted = starts.clone();
    sorted.sort_unstable();
    assert_eq!(starts, sorted, "segment order preserved");
}
