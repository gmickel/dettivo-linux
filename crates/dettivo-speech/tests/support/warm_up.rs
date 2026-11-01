use super::*;
use std::sync::{Arc, Mutex};

use dettivo_speech::supervisor::{EngineTransition, WHISPER_BINARY};

/// A supervisor over the scripted engine under the Whisper name, with
/// every transition state it fires.
fn recorded(dir: &Path) -> (Arc<Supervisor>, Arc<Mutex<Vec<&'static str>>>) {
    let supervisor = Supervisor::new(settings(dir, Duration::from_secs(60)));
    let states = Arc::new(Mutex::new(Vec::new()));
    let sink = states.clone();
    supervisor.set_hook(Arc::new(move |t: EngineTransition| {
        sink.lock().unwrap().push(t.state);
    }));
    (supervisor, states)
}

fn whisper(supervisor: &Arc<Supervisor>, model: &str) -> WhisperEngine {
    WhisperEngine::new(supervisor.clone(), model.into(), None)
}

fn take(engine: &WhisperEngine) {
    let request = RecognizeRequest {
        pcm: vec![0; 1600],
        language: "en".into(),
        prompt: None,
        timestamps: false,
    };
    engine.recognize(request, Duration::from_secs(10)).unwrap();
}

fn warm(supervisor: &Supervisor) -> bool {
    supervisor.status(&[WHISPER_BINARY])[0].warm
}

/// R1: a preload loads the model and warms it before any take; a take
/// afterwards finds it warm and fires no second warm-up.
#[test]
fn a_preload_warms_the_engine_before_the_first_take() {
    let dir = fake_engine_dir(WHISPER_BINARY);
    let (supervisor, states) = recorded(dir.path());
    let engine = whisper(&supervisor, "/models/fake.bin");
    engine.preload(PreloadSource::Startup).unwrap();
    assert_eq!(*states.lock().unwrap(), ["spawned", "loaded", "warm"]);
    assert!(warm(&supervisor));
    take(&engine);
    engine.preload(PreloadSource::SessionStart).unwrap();
    assert_eq!(*states.lock().unwrap(), ["spawned", "loaded", "warm"]);
    supervisor.shutdown();
}

/// R1: an engine that recognised within the last minute skips the
/// warm-up after a new load, and reads loaded rather than warm.
#[test]
fn a_recent_take_skips_the_warm_up_after_a_reload() {
    let dir = fake_engine_dir(WHISPER_BINARY);
    let (supervisor, states) = recorded(dir.path());
    take(&whisper(&supervisor, "/models/a.bin"));
    states.lock().unwrap().clear();
    whisper(&supervisor, "/models/b.bin")
        .preload(PreloadSource::ModelSelection)
        .unwrap();
    assert_eq!(*states.lock().unwrap(), ["loaded"]);
    assert!(!warm(&supervisor));
    supervisor.shutdown();
}

/// R1: a real request that asked before the warm-up could start runs
/// instead of it; the warm-up never queues ahead of a take.
#[test]
fn a_waiting_take_cancels_the_warm_up() {
    let dir = fake_engine_dir(WHISPER_BINARY);
    let (supervisor, states) = recorded(dir.path());
    let engine = whisper(&supervisor, "/models/fake.bin");
    let waiting = supervisor.begin_request(WHISPER_BINARY);
    engine.preload(PreloadSource::SessionStart).unwrap();
    assert_eq!(*states.lock().unwrap(), ["spawned", "loaded"]);
    drop(waiting);
    take(&engine);
    assert_eq!(*states.lock().unwrap(), ["spawned", "loaded", "warm"]);
    assert!(warm(&supervisor), "the take itself warmed the engine");
    supervisor.shutdown();
}
