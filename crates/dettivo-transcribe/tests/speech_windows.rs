//! Speech windows preserve source time and independently detect language.
use dettivo_engine_proto::{Backend, RecognizeResult, Segment, Word};
use dettivo_speech::{Capabilities, EngineError, PreloadSource, RecognizeRequest, SttEngine};
use dettivo_transcribe::{Request, SAMPLE_RATE, Settings, run};
use std::sync::{Mutex, atomic::AtomicBool};
use std::time::Duration;

#[derive(Default)]
struct SpeechProbe {
    calls: Mutex<Vec<(String, usize, usize)>>,
}
impl SttEngine for SpeechProbe {
    fn provider(&self) -> &str {
        "whisper"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_timestamps: true,
            supports_streaming: false,
            supported_languages: vec![],
            max_duration_seconds: 3600,
            supports_custom_vocabulary: true,
            supports_model_deletion: false,
        }
    }
    fn preload(&self, _: PreloadSource) -> Result<(), EngineError> {
        Ok(())
    }
    fn cancel(&self) {}
    fn backend(&self) -> Option<Backend> {
        Some(Backend::Cpu)
    }
    fn recognize(&self, r: RecognizeRequest, _: Duration) -> Result<RecognizeResult, EngineError> {
        let first = r.pcm.iter().position(|v| *v != 0).unwrap();
        let last = r.pcm.iter().rposition(|v| *v != 0).unwrap() + 1;
        let language = if r.language != "auto" {
            r.language.clone()
        } else if r.pcm[first] > 0 {
            "de".into()
        } else {
            "en".into()
        };
        self.calls
            .lock()
            .unwrap()
            .push((r.language, r.pcm.len(), first));
        let start_ms = first as u64 * 1000 / SAMPLE_RATE;
        let end_ms = last as u64 * 1000 / SAMPLE_RATE;
        Ok(RecognizeResult {
            text: language.clone(),
            language: language.clone(),
            duration_ms: r.pcm.len() as u64 * 1000 / SAMPLE_RATE,
            backend: Backend::Cpu,
            segments: vec![Segment {
                start_ms,
                end_ms,
                text: language.clone(),
                words: vec![Word {
                    start_ms,
                    end_ms,
                    text: language,
                    confidence: 1.0,
                }],
            }],
        })
    }
}

#[test]
fn silent_openings_do_not_reach_detection_and_a_later_language_switch_is_preserved() {
    let mut pcm = vec![0; 210 * SAMPLE_RATE as usize];
    pcm[120 * SAMPLE_RATE as usize..125 * SAMPLE_RATE as usize].fill(3300);
    pcm[180 * SAMPLE_RATE as usize..185 * SAMPLE_RATE as usize].fill(-3300);
    let engine = SpeechProbe::default();
    let mut progress = Vec::new();
    let t = run(
        &mut pcm,
        &engine,
        &Request {
            language: "auto".into(),
            prompt: None,
            from_system: false,
        },
        &Settings::default(),
        &AtomicBool::new(false),
        &mut |p| progress.push(p),
    )
    .unwrap();
    assert_eq!(t.text, "de en");
    assert_eq!(
        t.duration_ms, 210000,
        "skipped silence must not shorten the meeting"
    );
    assert_eq!(t.segments.len(), 2);
    for (segment, start) in t.segments.iter().zip([120000_u64, 180000]) {
        assert!(segment.start_ms.abs_diff(start) <= 1);
        assert!(segment.end_ms.abs_diff(start + 5000) <= 1);
        assert!(segment.words[0].start_ms.abs_diff(start) <= 1);
    }
    let calls = engine.calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        2,
        "wholly silent windows never reach the engine"
    );
    assert!(
        calls
            .iter()
            .all(|(language, samples, leading)| language == "auto"
                && *samples <= 30 * SAMPLE_RATE as usize
                && *leading <= SAMPLE_RATE as usize)
    );
    assert_eq!(
        progress.last().unwrap().chunks_done,
        progress.last().unwrap().chunks_total
    );
}

#[test]
fn trimming_preserves_quiet_speech_onsets_and_tails_near_a_louder_word() {
    let mut pcm = vec![0; 25 * SAMPLE_RATE as usize];
    pcm[10 * SAMPLE_RATE as usize..16 * SAMPLE_RATE as usize].fill(40);
    pcm[11 * SAMPLE_RATE as usize..15 * SAMPLE_RATE as usize].fill(3300);
    let engine = SpeechProbe::default();
    let t = run(
        &mut pcm,
        &engine,
        &Request {
            language: "de".into(),
            prompt: None,
            from_system: true,
        },
        &Settings::default(),
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(t.segments[0].start_ms, 10000);
    assert_eq!(t.segments[0].end_ms, 16000);
    let calls = engine.calls.lock().unwrap();
    assert_eq!(calls[0].0, "de", "an explicit language is retained");
    assert!(calls[0].1 < 10 * SAMPLE_RATE as usize);
}
