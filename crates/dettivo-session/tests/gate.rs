//! fn-79 through the session: the voice stage gates a take the level
//! passed (R2), a missing or failing voice pass leaves the level stage to
//! decide (R3), a Parakeet take reaches the engine with its trailing
//! silence and padding never empties a take (R4), and a prompt echo never
//! pastes the vocabulary (R6).

mod support;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use dettivo_proto::runtime::InsertionOutcome;
use dettivo_session::SessionError;
use dettivo_session::machine::Session;
use dettivo_speech::gate::VoiceDetector;
use dettivo_speech::{Capabilities, EngineError, PreloadSource, RecognizeRequest, SttEngine};
use support::{FakeEngine, Feed, policy, rig, silence_reason};

/// A voice pass with a fixed answer.
struct Voice(Result<u64, EngineError>);

impl VoiceDetector for Voice {
    fn speech_ms(&self, _: &[i16]) -> Result<u64, EngineError> {
        self.0.clone()
    }
}

/// One take of 400 ms of tone through `session`; the stop's result.
fn take(
    session: &Session,
    engine: Arc<dyn SttEngine>,
    feed: &Arc<Feed>,
    vocabulary: &[&str],
) -> Result<dettivo_session::Transcript, SessionError> {
    let dir = tempfile::tempdir().unwrap();
    let mut p = policy(dir.path(), false);
    p.vocabulary = vocabulary.iter().map(|w| (*w).into()).collect();
    session.start(p, engine, feed.clone(), None).unwrap();
    feed.speak(400);
    std::thread::sleep(Duration::from_millis(50));
    session.stop()
}

#[test]
fn the_voice_stage_gates_a_take_the_level_passed() {
    let (_dir, rec, session, engine, feed) = rig("never used");
    session.set_vad(Some(Arc::new(Voice(Ok(0)))));
    let ended = take(&session, engine.clone(), &feed, &[]);
    assert_eq!(ended.unwrap_err(), SessionError::NoSpeech);
    assert_eq!(engine.calls.load(Ordering::Relaxed), 0);
    assert_eq!(silence_reason(&rec).as_deref(), Some("silence: voice"));

    for vad in [Ok(240), Err(EngineError::Crashed("gone".into()))] {
        let (_dir, _rec, session, engine, feed) = rig("ok");
        session.set_vad(Some(Arc::new(Voice(vad))));
        assert_eq!(
            take(&session, engine.clone(), &feed, &[]).unwrap().text,
            "Ok"
        );
        assert_eq!(engine.calls.load(Ordering::Relaxed), 1);
    }
}

/// A voice pass that hears speech after a delay.
struct SlowVoice(Duration);

impl VoiceDetector for SlowVoice {
    fn speech_ms(&self, _: &[i16]) -> Result<u64, EngineError> {
        std::thread::sleep(self.0);
        Ok(240)
    }
}

#[test]
fn a_cancel_during_the_voice_pass_never_reaches_the_engine() {
    let (dir, _rec, session, engine, feed) = rig("never used");
    session.set_vad(Some(Arc::new(SlowVoice(Duration::from_millis(300)))));
    session
        .start(
            policy(dir.path(), false),
            engine.clone(),
            feed.clone(),
            None,
        )
        .unwrap();
    feed.speak(400);
    let session = Arc::new(session);
    let stopper = {
        let s = session.clone();
        std::thread::spawn(move || s.stop())
    };
    std::thread::sleep(Duration::from_millis(150));
    session.cancel().unwrap();
    assert_eq!(
        stopper.join().unwrap().unwrap_err(),
        SessionError::Cancelled
    );
    assert_eq!(engine.calls.load(Ordering::Relaxed), 0);
}

/// The scripted engine under the Parakeet provider name.
struct Parakeet(Arc<FakeEngine>);

impl SttEngine for Parakeet {
    fn provider(&self) -> &str {
        "parakeet"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_custom_vocabulary: false,
            ..self.0.capabilities()
        }
    }
    fn preload(&self, source: PreloadSource) -> Result<(), EngineError> {
        self.0.preload(source)
    }
    fn recognize(
        &self,
        request: RecognizeRequest,
        timeout: Duration,
    ) -> Result<dettivo_engine_proto::RecognizeResult, EngineError> {
        self.0.recognize(request, timeout)
    }
    fn cancel(&self) {
        self.0.cancel();
    }
    fn backend(&self) -> Option<dettivo_engine_proto::Backend> {
        self.0.backend()
    }
}

#[test]
fn a_parakeet_take_reaches_the_engine_with_trailing_silence() {
    let (_dir, _rec, session, engine, feed) = rig("send it.");
    let dir = tempfile::tempdir().unwrap();
    session
        .start(
            policy(dir.path(), false),
            Arc::new(Parakeet(engine.clone())),
            feed.clone(),
            None,
        )
        .unwrap();
    feed.speak(2_000);
    std::thread::sleep(Duration::from_millis(80));
    let t = session.stop().unwrap();
    assert_eq!(t.duration_ms, 2_000);
    assert_eq!(
        engine.last_samples.load(Ordering::Relaxed),
        32_000 + 6_400,
        "400 ms of silence after the last word"
    );
}

/// An engine that, like Parakeet v3 on some takes, returns nothing for the
/// padded take and the words for the take as captured.
struct EmptyWhenPadded(Arc<FakeEngine>, usize);

impl SttEngine for EmptyWhenPadded {
    fn provider(&self) -> &str {
        "parakeet"
    }
    fn capabilities(&self) -> Capabilities {
        self.0.capabilities()
    }
    fn preload(&self, source: PreloadSource) -> Result<(), EngineError> {
        self.0.preload(source)
    }
    fn recognize(
        &self,
        request: RecognizeRequest,
        timeout: Duration,
    ) -> Result<dettivo_engine_proto::RecognizeResult, EngineError> {
        let captured = request.pcm.len() == self.1;
        let mut result = self.0.recognize(request, timeout)?;
        if !captured {
            result.text.clear();
        }
        Ok(result)
    }
    fn cancel(&self) {}
    fn backend(&self) -> Option<dettivo_engine_proto::Backend> {
        None
    }
}

#[test]
fn padding_never_empties_a_take() {
    let (_dir, _rec, session, engine, feed) = rig("what you can do for your country.");
    let dir = tempfile::tempdir().unwrap();
    session
        .start(
            policy(dir.path(), false),
            Arc::new(EmptyWhenPadded(engine.clone(), 32_000)),
            feed.clone(),
            None,
        )
        .unwrap();
    feed.speak(2_000);
    let t = session.stop().unwrap();
    assert_eq!(t.text, "What you can do for your country.");
    assert_eq!(engine.calls.load(Ordering::Relaxed), 2);
    assert_eq!(engine.last_samples.load(Ordering::Relaxed), 32_000);
}

#[test]
fn a_prompt_echo_never_pastes_the_vocabulary() {
    let (_dir, rec, session, engine, feed) = rig("Dettivo, Omarchy.");
    let t = take(&session, engine, &feed, &["Dettivo", "Omarchy"]).unwrap();
    assert_eq!(t.raw_text, "");
    assert!(t.insertion.is_none());
    assert!(rec.inserted.lock().unwrap().is_empty());

    let (_dir, _rec, session, engine, feed) = rig("Dettivo.");
    let t = take(&session, engine, &feed, &["Dettivo"]).unwrap();
    assert_eq!(t.text, "Dettivo.");
    assert_eq!(
        t.insertion.unwrap().outcome,
        InsertionOutcome::Inserted,
        "the one listed term pastes"
    );
}

/// The scripted engine, keeping the prompt it was primed with.
struct Prompted(Arc<FakeEngine>, std::sync::Mutex<Option<String>>);

impl SttEngine for Prompted {
    fn provider(&self) -> &str {
        self.0.provider()
    }
    fn capabilities(&self) -> Capabilities {
        self.0.capabilities()
    }
    fn preload(&self, source: PreloadSource) -> Result<(), EngineError> {
        self.0.preload(source)
    }
    fn recognize(
        &self,
        request: RecognizeRequest,
        timeout: Duration,
    ) -> Result<dettivo_engine_proto::RecognizeResult, EngineError> {
        *self.1.lock().unwrap() = request.prompt.clone();
        self.0.recognize(request, timeout)
    }
    fn cancel(&self) {
        self.0.cancel();
    }
    fn backend(&self) -> Option<dettivo_engine_proto::Backend> {
        self.0.backend()
    }
}

/// fn-83 R4: the engine is primed with the terms, never their aliases.
#[test]
fn the_engine_prompt_carries_terms_and_never_aliases() {
    let (dir, _rec, session, engine, feed) = rig("Ship the Clef build.");
    let prompted = Arc::new(Prompted(engine, std::sync::Mutex::new(None)));
    let mut p = policy(dir.path(), false);
    p.vocabulary = vec![
        dettivo_core::config::vocabulary_schema::VocabularyEntry::Entry {
            term: "Clef".into(),
            sounds_like: vec!["clay".into(), "cleff".into()],
        },
        "Dettivo".into(),
    ];
    session
        .start(p, prompted.clone(), feed.clone(), None)
        .unwrap();
    feed.speak(400);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(session.stop().unwrap().text, "Ship the Clef build.");
    assert_eq!(
        prompted.1.lock().unwrap().as_deref(),
        Some("Clef, Dettivo.")
    );
}
