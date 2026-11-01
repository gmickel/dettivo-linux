use super::*;
use std::sync::{Arc, Mutex};

use dettivo_engine_proto::Backend;
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

/// R1: a reload of the same model path on another backend is a new
/// instance that never inferred: the earlier take does not make it warm.
#[test]
fn a_take_never_warms_a_reload_of_its_model_on_another_backend() {
    let dir = fake_engine_dir(WHISPER_BINARY);
    let (supervisor, states) = recorded(dir.path());
    take(&whisper(&supervisor, "/models/a.bin"));
    assert!(warm(&supervisor));
    states.lock().unwrap().clear();
    whisper(&supervisor, "/models/a.bin")
        .with_backend(BackendPreference::Cpu)
        .preload(PreloadSource::ModelSelection)
        .unwrap();
    assert_eq!(*states.lock().unwrap(), ["loaded"]);
    assert!(!warm(&supervisor));
    supervisor.shutdown();
}

/// Every transition with the backend it names.
type Seen = Arc<Mutex<Vec<(&'static str, Option<Backend>)>>>;

/// A supervisor that may load on the GPU (`gpu-has-room`), where the
/// scripted engine's every inference then reports the CPU.
fn on_gpu(dir: &Path) -> (Arc<Supervisor>, Seen) {
    std::fs::write(dir.join("gpu-has-room"), "").unwrap();
    let supervisor = Supervisor::new(Settings {
        force_cpu: false,
        ..settings(dir, Duration::from_secs(60))
    });
    let seen: Seen = Arc::default();
    let sink = seen.clone();
    supervisor.set_hook(Arc::new(move |t: EngineTransition| {
        sink.lock().unwrap().push((t.state, t.backend));
    }));
    (supervisor, seen)
}

/// ADR 0080 and 0084: the backend an inference ran on is recorded in the
/// same step that marks the engine warm, so `warm` is never published on
/// the backend the engine left, by the warm-up or by a first take.
#[test]
fn warm_is_published_on_the_backend_the_inference_ran_on() {
    let expected = [
        ("spawned", None),
        ("loaded", Some(Backend::Vulkan)),
        ("loaded", Some(Backend::Cpu)),
        ("warm", Some(Backend::Cpu)),
    ];
    let dir = fake_engine_dir(WHISPER_BINARY);
    let (supervisor, seen) = on_gpu(dir.path());
    whisper(&supervisor, "/models/fake.bin")
        .preload(PreloadSource::Startup)
        .unwrap();
    assert_eq!(*seen.lock().unwrap(), expected, "the warm-up");
    supervisor.shutdown();

    let dir = fake_engine_dir(WHISPER_BINARY);
    let (supervisor, seen) = on_gpu(dir.path());
    take(&whisper(&supervisor, "/models/fake.bin"));
    assert_eq!(*seen.lock().unwrap(), expected, "the first take");
    let status = &supervisor.status(&[WHISPER_BINARY])[0];
    assert_eq!((status.backend, status.warm), (Some(Backend::Cpu), true));
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

/// R1 on the CPU backend (ADR 0084): there are no GPU kernels to compile,
/// so the first take costs what a warm one does; a CPU load reads warm at
/// once and runs no silent inference a take would have to wait behind.
#[test]
fn a_cpu_load_is_warm_without_a_warm_up() {
    let dir = fake_engine_dir(WHISPER_BINARY);
    std::fs::write(dir.path().join("record-recognizes"), "").unwrap();
    let (supervisor, states) = recorded(dir.path());
    whisper(&supervisor, "/models/fake.bin")
        .preload(PreloadSource::Startup)
        .unwrap();
    assert_eq!(*states.lock().unwrap(), ["spawned", "loaded", "warm"]);
    assert!(warm(&supervisor));
    let recognized = std::fs::read_to_string(dir.path().join("recognizes.log")).unwrap_or_default();
    assert_eq!(recognized, "", "no warm-up inference on the CPU");
    supervisor.shutdown();
}
