//! The speech engines as `SttEngine`s over the supervisor: one struct per
//! engine binary with its capabilities, the per-provider meeting
//! capability the S-13 alignment spike decided (ADR 0018), and the Silero
//! voice activity pass the dictation gate asks (`SileroVad`).

use std::sync::Arc;
use std::time::Duration;

use dettivo_engine_proto::{
    Backend, BackendPreference, DetectSpeechParams, DetectSpeechResult, FeedSpeechParams,
    FeedSpeechResult, LoadParams, RecognizeParams, RecognizeResult,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::gate::{SpeechStream, VAD_PARAMS, VoiceDetector};
use crate::supervisor::Supervisor;
use crate::{Capabilities, EngineError, PreloadSource, RecognizeRequest, SttEngine};

/// The whisper engine binary name.
pub const WHISPER_BINARY: &str = "dettivo-engine-whisper";
/// The parakeet engine binary name.
pub const PARAKEET_BINARY: &str = dettivo_engine_proto::warm_up::PARAKEET_BINARY;
/// The language model engine binary name.
pub const LLM_BINARY: &str = "dettivo-engine-llm";
/// The diarization engine binary name (ADR 0035).
pub const DIARIZE_BINARY: &str = "dettivo-engine-diarize";
/// The supervisor slot of the voice activity pass: a second Whisper engine
/// process holding only the Silero model, so it never evicts a recognizer.
pub const VAD_SLOT: &str = "dettivo-engine-whisper/vad";

/// Whether a provider's timestamps are precise enough for the meeting
/// merger: the outcome of the alignment spike
/// (`docs/reports/parakeet-alignment-report.json`, ADR 0018). Whisper
/// carries segment timestamps only and stays meeting-capable as the
/// other ports run it; Parakeet's word timestamps missed the rule (pooled
/// start/end p95 of 308 ms for v2 and 347 ms for v3 against the 200 ms
/// tolerance), so Parakeet is dictation-only until a later spike moves it.
pub fn meeting_capable(provider: &str) -> bool {
    provider == "whisper"
}

/// The capability contract per provider.
pub fn capabilities_of(provider: &str) -> Capabilities {
    match provider {
        "parakeet" => Capabilities {
            supports_timestamps: true,
            supports_streaming: false,
            supported_languages: Vec::new(),
            max_duration_seconds: 3600,
            supports_custom_vocabulary: false,
            supports_model_deletion: true,
        },
        _ => Capabilities {
            supports_timestamps: true,
            supports_streaming: false,
            supported_languages: Vec::new(),
            max_duration_seconds: 3600,
            supports_custom_vocabulary: true,
            supports_model_deletion: true,
        },
    }
}

/// The longest a warm-up `recognize` may take.
const WARM_UP_TIMEOUT: Duration = Duration::from_secs(60);

/// What every engine over the supervisor shares.
struct Shared {
    supervisor: Arc<Supervisor>,
    binary: &'static str,
    model: String,
    vad_model: Option<String>,
    backend: BackendPreference,
}

impl Shared {
    /// The complete `load` every request on this engine carries.
    fn load_params(&self) -> LoadParams {
        LoadParams {
            model: self.model.clone(),
            backend_preference: self.backend,
            vad_model: self.vad_model.clone(),
            context_length: None,
            lora: None,
            threads: None,
        }
    }

    /// Loads the model, then warms it with one silent `recognize` whose
    /// result is discarded (ADR 0084); only the timing is logged.
    fn preload(&self, source: PreloadSource) -> Result<(), EngineError> {
        tracing::info!(source = ?source, engine = self.binary, "preload");
        self.supervisor
            .with_engine_load(self.binary, self.load_params(), |_, _| Ok(()))?;
        let params = RecognizeParams {
            language: "en".into(),
            prompt: None,
            timestamps: false,
            vocabulary: None,
        };
        let samples = dettivo_engine_proto::warm_up::samples(self.binary);
        let pcm = dettivo_engine_proto::pcm_to_bytes(&vec![0; samples]);
        let warmed = self.supervisor.warm_up(self.binary, |process| {
            let value = process.call(
                "recognize",
                serde_json::to_value(&params).unwrap_or(Value::Null),
                &[&pcm],
                WARM_UP_TIMEOUT,
                |_| {},
            )?;
            // The text is discarded; the backend is kept, since the engine
            // may have moved to the CPU for want of GPU memory (ADR 0080).
            Ok(serde_json::from_value::<Backend>(value["backend"].clone()).ok())
        })?;
        if let Some(took) = warmed {
            tracing::info!(
                engine = self.binary,
                warm_up_ms = took.as_millis() as u64,
                "engine warmed up"
            );
        }
        Ok(())
    }

    fn recognize(
        &self,
        request: RecognizeRequest,
        timeout: Duration,
    ) -> Result<RecognizeResult, EngineError> {
        let params = RecognizeParams {
            language: request.language.clone(),
            prompt: request.prompt.clone(),
            timestamps: request.timestamps,
            vocabulary: request.vocabulary.clone(),
        };
        let pcm = dettivo_engine_proto::pcm_to_bytes(&request.pcm);
        let _request = self.supervisor.begin_request(self.binary);
        // The supervisor records the take's warmth and backend (an engine
        // moves itself to the CPU when the GPU has no room, ADR 0080).
        self.supervisor
            .recognize_on(self.binary, self.load_params(), |process| {
                let value = process.call(
                    "recognize",
                    serde_json::to_value(&params).unwrap_or(Value::Null),
                    &[&pcm],
                    timeout,
                    |event| tracing::debug!(event = %event.name, "engine event"),
                )?;
                serde_json::from_value(value)
                    .map_err(|e| EngineError::Transport(format!("recognize response: {e}")))
            })
    }

    fn backend(&self) -> Option<Backend> {
        self.supervisor
            .status(&[self.binary])
            .first()
            .and_then(|s| s.backend)
    }
}

/// The Whisper engine as an `SttEngine`.
pub struct WhisperEngine(Shared);

impl WhisperEngine {
    /// A recognizer over the supervisor for `model`.
    pub fn new(supervisor: Arc<Supervisor>, model: String, vad_model: Option<String>) -> Self {
        Self(Shared {
            supervisor,
            binary: WHISPER_BINARY,
            model,
            vad_model,
            backend: BackendPreference::Auto,
        })
    }

    /// The backend preference every load carries (`[engines.whisper] backend`).
    pub fn with_backend(mut self, backend: BackendPreference) -> Self {
        self.0.backend = backend;
        self
    }
}

impl SttEngine for WhisperEngine {
    fn provider(&self) -> &str {
        "whisper"
    }

    fn capabilities(&self) -> Capabilities {
        capabilities_of("whisper")
    }

    fn preload(&self, source: PreloadSource) -> Result<(), EngineError> {
        self.0.preload(source)
    }

    fn recognize(
        &self,
        request: RecognizeRequest,
        timeout: Duration,
    ) -> Result<RecognizeResult, EngineError> {
        self.0.recognize(request, timeout)
    }

    fn cancel(&self) {
        // Recognition runs to completion in the engine; the next request
        // starts fresh. A running request is bounded by its timeout.
    }

    fn backend(&self) -> Option<Backend> {
        self.0.backend()
    }
}

/// The Parakeet engine as an `SttEngine` (ADR 0018): word timestamps and
/// confidence on every segment, no vocabulary prompt.
pub struct ParakeetEngine(Shared);

impl ParakeetEngine {
    /// A recognizer over the supervisor for `model`.
    pub fn new(supervisor: Arc<Supervisor>, model: String) -> Self {
        Self(Shared {
            supervisor,
            binary: PARAKEET_BINARY,
            model,
            vad_model: None,
            backend: BackendPreference::Auto,
        })
    }

    /// The backend preference every load carries (`[engines.parakeet] backend`).
    pub fn with_backend(mut self, backend: BackendPreference) -> Self {
        self.0.backend = backend;
        self
    }
}

impl SttEngine for ParakeetEngine {
    fn provider(&self) -> &str {
        "parakeet"
    }

    fn capabilities(&self) -> Capabilities {
        capabilities_of("parakeet")
    }

    fn preload(&self, source: PreloadSource) -> Result<(), EngineError> {
        self.0.preload(source)
    }

    fn recognize(
        &self,
        request: RecognizeRequest,
        timeout: Duration,
    ) -> Result<RecognizeResult, EngineError> {
        // Parakeet takes the vocabulary as terms, never as a prompt; the
        // prompt is dropped here so the engine never sees it.
        self.0.recognize(
            RecognizeRequest {
                prompt: None,
                ..request
            },
            timeout,
        )
    }

    fn cancel(&self) {}

    fn backend(&self) -> Option<Backend> {
        self.0.backend()
    }
}

/// The Silero voice activity pass over the supervisor, on the CPU.
#[derive(Clone)]
pub struct SileroVad {
    supervisor: Arc<Supervisor>,
    model: String,
}

impl SileroVad {
    /// A pass over `supervisor` with the Silero model at `model`.
    pub fn new(supervisor: Arc<Supervisor>, model: String) -> Self {
        Self { supervisor, model }
    }

    /// Sends `name` with `params` and `pcm` to the voice activity process.
    fn call<R: DeserializeOwned>(
        &self,
        name: &str,
        params: impl Serialize,
        pcm: &[i16],
    ) -> Result<R, EngineError> {
        let load = LoadParams {
            model: String::new(),
            backend_preference: BackendPreference::Cpu,
            vad_model: Some(self.model.clone()),
            context_length: None,
            lora: None,
            threads: None,
        };
        let pcm = dettivo_engine_proto::pcm_to_bytes(pcm);
        self.supervisor
            .with_engine_slot(VAD_SLOT, WHISPER_BINARY, load, |process, _| {
                let value = process.call(
                    name,
                    serde_json::to_value(params).unwrap_or(Value::Null),
                    &[&pcm],
                    Duration::from_secs(30),
                    |_| {},
                )?;
                serde_json::from_value::<R>(value)
                    .map_err(|e| EngineError::Transport(format!("{name} response: {e}")))
            })
    }
}

impl VoiceDetector for SileroVad {
    fn speech_ms(&self, pcm: &[i16]) -> Result<u64, EngineError> {
        self.call::<DetectSpeechResult>("detect_speech", VAD_PARAMS, pcm)
            .map(|r| r.speech_ms)
    }

    fn stream(&self) -> Option<Box<dyn SpeechStream>> {
        Some(Box::new(SileroStream {
            vad: self.clone(),
            fed: 0,
        }))
    }
}

/// One take through the voice activity process as it is captured
/// (`feed_speech`, then `detect_speech` with `continues`).
struct SileroStream {
    vad: SileroVad,
    /// The samples the process holds for this take.
    fed: u64,
}

impl SileroStream {
    /// Sends `pcm` with `threshold` and checks the process holds the take.
    fn send(
        &mut self,
        pcm: &[i16],
        threshold: Option<f64>,
    ) -> Result<FeedSpeechResult, EngineError> {
        let start = self.fed == 0;
        let fed: FeedSpeechResult =
            self.vad
                .call("feed_speech", FeedSpeechParams { start, threshold }, pcm)?;
        if fed.samples != self.fed + pcm.len() as u64 {
            return Err(EngineError::Transport(format!(
                "feed_speech holds {} samples, not {}",
                fed.samples,
                self.fed + pcm.len() as u64
            )));
        }
        self.fed = fed.samples;
        Ok(fed)
    }
}

impl SpeechStream for SileroStream {
    fn feed(&mut self, pcm: &[i16]) -> Result<(), EngineError> {
        self.send(pcm, None).map(|_| ())
    }

    fn feed_voiced(&mut self, pcm: &[i16]) -> Result<Option<u64>, EngineError> {
        self.send(pcm, Some(VAD_PARAMS.threshold))
            .map(|fed| fed.voiced_until)
    }

    fn finish(&mut self, rest: &[i16]) -> Result<u64, EngineError> {
        if self.fed == 0 {
            return self.vad.speech_ms(rest);
        }
        let params = DetectSpeechParams {
            continues: Some(self.fed),
            ..VAD_PARAMS
        };
        self.vad
            .call::<DetectSpeechResult>("detect_speech", params, rest)
            .map(|r| r.speech_ms)
    }
}

/// The engine for `provider` over `supervisor`, or `None` for a provider
/// with no engine binary.
pub fn engine_for(
    provider: &str,
    supervisor: Arc<Supervisor>,
    model: String,
    backend: BackendPreference,
) -> Option<Arc<dyn SttEngine>> {
    match provider {
        "whisper" => Some(Arc::new(
            WhisperEngine::new(supervisor, model, None).with_backend(backend),
        )),
        "parakeet" => Some(Arc::new(
            ParakeetEngine::new(supervisor, model).with_backend(backend),
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_and_meeting_capability_per_provider() {
        assert!(capabilities_of("whisper").supports_custom_vocabulary);
        assert!(!capabilities_of("parakeet").supports_custom_vocabulary);
        assert!(capabilities_of("parakeet").supports_timestamps);
        assert!(meeting_capable("whisper"));
        assert!(!meeting_capable("parakeet"), "ADR 0018: dictation-only");
        assert!(!meeting_capable("nope"));
        let supervisor = Supervisor::new(Default::default());
        assert!(
            engine_for(
                "nope",
                supervisor.clone(),
                "m".into(),
                BackendPreference::Auto
            )
            .is_none()
        );
        let engine =
            engine_for("parakeet", supervisor, "m".into(), BackendPreference::Cpu).unwrap();
        assert_eq!(engine.provider(), "parakeet");
        assert_eq!(engine.backend(), None);
    }
}
