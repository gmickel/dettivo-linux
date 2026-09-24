//! R2 and R5 against real processes: the supervisor spawns an engine on
//! first use, keeps it warm across requests, unloads and terminates it
//! after the idle timeout, restarts it with backoff after a crash, marks it
//! degraded after three crashes in a row, and names the binary and the
//! directories it searched when none is found. The scripted engine is
//! `dettivo-engine-proto`'s `fake-engine` example; the last tests drive the
//! real Whisper and Parakeet engines over the protocol when their test
//! models are present.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use dettivo_engine_proto::BackendPreference;
use dettivo_speech::supervisor::{LlmLoad, ParakeetEngine, Settings, Supervisor, WhisperEngine};
use dettivo_speech::{EngineError, PreloadSource, RecognizeRequest, SttEngine};
use serde_json::json;

fn target_dir() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    dir.pop();
    dir.join("target/debug")
}

/// Builds the scripted engine and copies it once per test process, before
/// any test spawns it: a copy written while another test thread forks
/// leaves the write descriptor open in the child until its exec, and the
/// engine's own exec then fails with `ETXTBSY`. Every test gets a
/// directory holding a link to that one copy under the name the
/// supervisor is asked for.
fn fake_engine_dir(name: &str) -> tempfile::TempDir {
    static COPY: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let shared = COPY.get_or_init(|| {
        let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args([
                "build",
                "-q",
                "-p",
                "dettivo-engine-proto",
                "--example",
                "fake-engine",
            ])
            .status()
            .expect("cargo build");
        assert!(status.success());
        let example = target_dir().join("examples/fake-engine");
        // Under target/ so `cargo clean` removes it; one copy per process.
        let dir = target_dir().join(format!("qa-fake-engine-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let copy = dir.join("fake-engine");
        std::fs::copy(&example, &copy).unwrap();
        copy
    });
    // A hard link keeps the binary's own directory (the engine looks for
    // its marker files there) without opening the copy for writing. The
    // directory lives next to the copy: a link across filesystems (`/tmp`
    // is usually tmpfs) fails, and the copy fallback reopens the binary
    // for writing while sibling tests fork, which is the `ETXTBSY` above.
    let dir = tempfile::tempdir_in(shared.parent().unwrap()).unwrap();
    let link = dir.path().join(name);
    if std::fs::hard_link(shared, &link).is_err() {
        std::fs::copy(shared, &link).unwrap();
    }
    dir
}

fn settings(dir: &Path, idle: Duration) -> Settings {
    Settings {
        directory: Some(dir.to_path_buf()),
        stt_idle: idle,
        llm_idle: idle / 4,
        force_cpu: true,
        load_timeout: Duration::from_secs(10),
    }
}

fn recognize(supervisor: &Supervisor, binary: &str, language: &str) -> Result<String, EngineError> {
    supervisor.with_engine(binary, "/models/fake.bin", None, |process, _| {
        let v = process.call(
            "recognize",
            json!({"language": language, "timestamps": true}),
            &[&[0u8; 3200]],
            Duration::from_secs(10),
            |_| {},
        )?;
        Ok(v["text"].as_str().unwrap_or("").to_string())
    })
}

#[test]
fn spawns_once_keeps_warm_and_unloads_after_idle() {
    let dir = fake_engine_dir("dettivo-engine-fake");
    let supervisor = Supervisor::new(settings(dir.path(), Duration::from_millis(400)));
    let first = recognize(&supervisor, "dettivo-engine-fake", "en").unwrap();
    let second = recognize(&supervisor, "dettivo-engine-fake", "de").unwrap();
    let pid = |s: &str| s.split("pid=").nth(1).unwrap().to_string();
    assert_eq!(
        pid(&first),
        pid(&second),
        "the warm process served both requests"
    );
    let status = supervisor.status(&["dettivo-engine-fake"]);
    assert!(status[0].running);
    assert_eq!(status[0].model.as_deref(), Some("/models/fake.bin"));
    assert_eq!(status[0].backend, Some(dettivo_engine_proto::Backend::Cpu));

    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(supervisor.reap_idle(), 1, "idle engine unloaded");
    let status = supervisor.status(&["dettivo-engine-fake"]);
    assert!(!status[0].running);
    assert_eq!(status[0].model, None);

    let third = recognize(&supervisor, "dettivo-engine-fake", "fr").unwrap();
    assert_ne!(pid(&third), pid(&first), "a fresh process after the unload");
    supervisor.shutdown();
}

/// The language model engine has its own idle timeout: with `llm_idle`
/// a quarter of `stt_idle`, the LLM slot is reaped while the speech slot
/// stays warm.
#[test]
fn the_llm_engine_is_reaped_on_its_own_idle_timeout() {
    let dir = fake_engine_dir("dettivo-engine-fake");
    let llm_link = dir.path().join("dettivo-engine-llm");
    std::fs::hard_link(dir.path().join("dettivo-engine-fake"), &llm_link)
        .or_else(|_| std::fs::copy(dir.path().join("dettivo-engine-fake"), &llm_link).map(|_| ()))
        .unwrap();
    let supervisor = Supervisor::new(settings(dir.path(), Duration::from_millis(1200)));
    recognize(&supervisor, "dettivo-engine-fake", "en").unwrap();
    recognize(&supervisor, "dettivo-engine-llm", "en").unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(supervisor.reap_idle(), 1, "the LLM slot idles out first");
    let rows = supervisor.status(&["dettivo-engine-fake", "dettivo-engine-llm"]);
    assert!(rows[0].running, "speech engine still warm");
    assert!(!rows[1].running, "LLM engine unloaded");
    supervisor.shutdown();
}

#[test]
fn crashes_restart_with_backoff_and_degrade_after_three() {
    let dir = fake_engine_dir("dettivo-engine-fake");
    let supervisor = Supervisor::new(settings(dir.path(), Duration::from_secs(60)));
    let first = recognize(&supervisor, "dettivo-engine-fake", "en").unwrap();
    let pid = |s: &str| s.split("pid=").nth(1).unwrap().to_string();

    let err = recognize(&supervisor, "dettivo-engine-fake", "crash").unwrap_err();
    assert!(matches!(err, EngineError::Crashed(_)), "{err:?}");
    let status = supervisor.status(&["dettivo-engine-fake"]);
    assert_eq!(status[0].crashes, 1);
    assert!(!status[0].degraded);

    let started = Instant::now();
    let restarted = recognize(&supervisor, "dettivo-engine-fake", "en").unwrap();
    assert!(
        started.elapsed() >= Duration::from_millis(900),
        "backoff before the restart"
    );
    assert_ne!(pid(&restarted), pid(&first));
    assert_eq!(
        supervisor.status(&["dettivo-engine-fake"])[0].crashes,
        0,
        "a good load clears the count"
    );

    // Degradation after three load crashes in a row has its own test below;
    // a request crash after a good load counts one again.
    let err = recognize(&supervisor, "dettivo-engine-fake", "crash").unwrap_err();
    assert!(matches!(err, EngineError::Crashed(_)), "{err:?}");
    assert_eq!(supervisor.status(&["dettivo-engine-fake"])[0].crashes, 1);
    supervisor.shutdown();
}

#[test]
fn three_load_crashes_in_a_row_mark_the_engine_degraded() {
    let dir = fake_engine_dir("dettivo-engine-fake");
    std::fs::write(dir.path().join("crash-on-load"), b"").unwrap();
    let supervisor = Supervisor::new(Settings {
        load_timeout: Duration::from_secs(5),
        ..settings(dir.path(), Duration::from_secs(60))
    });
    for i in 1..=3 {
        let err = recognize(&supervisor, "dettivo-engine-fake", "en").unwrap_err();
        assert!(matches!(err, EngineError::Crashed(_)), "crash {i}: {err:?}");
        let status = supervisor.status(&["dettivo-engine-fake"]);
        assert_eq!(status[0].crashes, i);
    }
    let status = supervisor.status(&["dettivo-engine-fake"]);
    assert!(status[0].degraded);
    let err = recognize(&supervisor, "dettivo-engine-fake", "en").unwrap_err();
    assert_eq!(err, EngineError::Degraded);
    supervisor.shutdown();
}

/// engines/F6: the slot caches the whole `load`, so a backend-only change
/// (same model, same adapter) reaches the engine as a new load.
#[test]
fn a_backend_only_change_reloads_the_engine() {
    let dir = fake_engine_dir("dettivo-engine-fake");
    let supervisor = Supervisor::new(settings(dir.path(), Duration::from_secs(60)));
    let loads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = loads.clone();
    supervisor.set_hook(std::sync::Arc::new(move |t| {
        if t.state == "loaded" {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }));
    for backend in [
        BackendPreference::Auto,
        BackendPreference::Auto,
        BackendPreference::Cpu,
    ] {
        supervisor
            .with_engine_on(
                "dettivo-engine-fake",
                "/models/fake.bin",
                None,
                backend,
                LlmLoad::default(),
                |_, _| Ok(()),
            )
            .unwrap();
    }
    assert_eq!(
        loads.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "the same load is reused once; the changed backend loads again"
    );
    supervisor.shutdown();
}

/// engines/F3: a request that times out leaves the engine in a state
/// nobody can vouch for; the process is terminated and the next request
/// spawns and loads afresh instead of reading the late answer.
#[test]
fn a_timed_out_request_terminates_the_engine_and_the_next_one_starts_fresh() {
    let dir = fake_engine_dir("dettivo-engine-fake");
    let supervisor = Supervisor::new(settings(dir.path(), Duration::from_secs(60)));
    let first = recognize(&supervisor, "dettivo-engine-fake", "en").unwrap();
    let stalled = supervisor.with_engine(
        "dettivo-engine-fake",
        "/models/fake.bin",
        None,
        |process, _| {
            process
                .call(
                    "recognize",
                    json!({"language": "stall", "timestamps": true}),
                    &[&[0u8; 3200]],
                    Duration::from_millis(300),
                    |_| {},
                )
                .map(|_| ())
        },
    );
    assert!(
        matches!(stalled, Err(EngineError::Transport(_))),
        "{stalled:?}"
    );
    let status = supervisor.status(&["dettivo-engine-fake"]).remove(0);
    assert!(!status.running, "the stalled engine is gone");
    assert_eq!(status.crashes, 0, "a timeout is not a crash");
    let second = recognize(&supervisor, "dettivo-engine-fake", "en").unwrap();
    assert_ne!(first, second, "a fresh process answers (the pid differs)");
    assert!(second.starts_with("fake en"), "{second}");
    supervisor.shutdown();
}

#[test]
fn a_missing_binary_names_itself_and_the_directories_searched() {
    let dir = tempfile::tempdir().unwrap();
    let supervisor = Supervisor::new(settings(dir.path(), Duration::from_secs(60)));
    let err = recognize(&supervisor, "dettivo-engine-absent", "en").unwrap_err();
    match err {
        EngineError::NotFound { binary, searched } => {
            assert_eq!(binary, "dettivo-engine-absent");
            assert!(searched.contains(&dir.path().to_path_buf()));
        }
        other => panic!("{other:?}"),
    }
    let text = err_text(&supervisor);
    assert!(text.contains("dettivo-engine-absent") && text.contains(dir.path().to_str().unwrap()));
}

fn err_text(supervisor: &Supervisor) -> String {
    recognize(supervisor, "dettivo-engine-absent", "en")
        .unwrap_err()
        .to_string()
}

fn test_model() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let model = data.join("dettivo/models/whisper/tiny.en/ggml-tiny.en.bin");
    model.is_file().then_some(model)
}

fn read_wav(path: &Path) -> Vec<i16> {
    let mut reader = hound::WavReader::open(path).unwrap();
    reader.samples::<i16>().map(Result::unwrap).collect()
}

/// R3 over the protocol: the real engine, driven by the supervisor, returns
/// the fixture's transcript.
#[test]
fn the_whisper_engine_recognizes_the_fixture_over_the_protocol() {
    let Some(model) = test_model() else {
        eprintln!("skip: test model missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let fixture = model
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures/jfk.wav");
    let binary = target_dir().join("dettivo-engine-whisper");
    if !binary.is_file() {
        let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-q", "-p", "dettivo-engine-whisper"])
            .status()
            .expect("cargo build");
        assert!(status.success());
    }
    let supervisor = Supervisor::new(Settings {
        directory: Some(target_dir()),
        stt_idle: Duration::from_secs(60),
        llm_idle: Duration::from_secs(60),
        force_cpu: true,
        load_timeout: Duration::from_secs(120),
    });
    let engine = WhisperEngine::new(
        supervisor.clone(),
        model.to_string_lossy().into_owned(),
        None,
    );
    engine.preload(PreloadSource::Startup).unwrap();
    assert_eq!(engine.backend(), Some(dettivo_engine_proto::Backend::Cpu));
    let result = engine
        .recognize(
            RecognizeRequest {
                pcm: read_wav(&fixture),
                language: "en".into(),
                prompt: None,
                timestamps: true,
            },
            Duration::from_secs(120),
        )
        .unwrap();
    let words: Vec<String> = result
        .text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect();
    assert!(
        words.contains(&"americans".to_string()) && words.contains(&"country".to_string()),
        "{}",
        result.text
    );
    assert!(!result.segments.is_empty());
    assert!(result.duration_ms > 10_000);
    supervisor.shutdown();
}

/// R2 for Parakeet over the protocol: the real engine, driven by the
/// supervisor with the `[engines.parakeet] backend = cpu` preference,
/// returns the fixture's transcript with word timestamps and confidence
/// on every segment, and the prompt never reaches it.
#[test]
fn the_parakeet_engine_recognizes_the_fixture_over_the_protocol_with_words() {
    let Some(whisper_model) = test_model() else {
        eprintln!("skip: test models missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let models = whisper_model
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let model = models.join("parakeet/parakeet-v2/tdt-0.6b-v2-q8_0.gguf");
    if !model.is_file() {
        eprintln!("skip: parakeet-v2 missing (run scripts/models/fetch-test-model.sh)");
        return;
    }
    let binary = target_dir().join("dettivo-engine-parakeet");
    if !binary.is_file() {
        let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-q", "-p", "dettivo-engine-parakeet"])
            .status()
            .expect("cargo build");
        assert!(status.success());
    }
    let supervisor = Supervisor::new(Settings {
        directory: Some(target_dir()),
        stt_idle: Duration::from_secs(60),
        llm_idle: Duration::from_secs(60),
        force_cpu: false,
        load_timeout: Duration::from_secs(120),
    });
    let engine = ParakeetEngine::new(supervisor.clone(), model.to_string_lossy().into_owned())
        .with_backend(dettivo_engine_proto::BackendPreference::Cpu);
    assert_eq!(engine.provider(), "parakeet");
    assert!(!engine.capabilities().supports_custom_vocabulary);
    engine.preload(PreloadSource::Startup).unwrap();
    assert_eq!(engine.backend(), Some(dettivo_engine_proto::Backend::Cpu));
    let status = supervisor.status(&["dettivo-engine-parakeet"]);
    assert_eq!(
        status[0].reason.as_deref(),
        Some("backend_preference = cpu")
    );
    let result = engine
        .recognize(
            RecognizeRequest {
                pcm: read_wav(&models.join("fixtures/jfk.wav")),
                language: "en".into(),
                prompt: Some("MARKER_PROMPT_TEXT".into()),
                timestamps: true,
            },
            Duration::from_secs(120),
        )
        .unwrap();
    let text = result.text.to_lowercase();
    assert!(
        text.contains("fellow americans") && text.contains("country"),
        "{}",
        result.text
    );
    assert_eq!(result.language, "en");
    assert!(!result.segments.is_empty());
    let words: Vec<&dettivo_engine_proto::Word> =
        result.segments.iter().flat_map(|s| &s.words).collect();
    assert!(words.len() >= 20, "{}", words.len());
    assert!(
        words
            .iter()
            .all(|w| w.confidence > 0.0 && w.confidence <= 1.0)
    );
    assert!(words.windows(2).all(|p| p[0].end_ms <= p[1].start_ms));
    let err = engine
        .recognize(
            RecognizeRequest {
                pcm: vec![0; 16_000],
                language: "de".into(),
                prompt: None,
                timestamps: true,
            },
            Duration::from_secs(30),
        )
        .unwrap_err();
    match err {
        EngineError::Engine { code, message } => {
            assert_eq!(code, "bad_request");
            assert!(message.contains("languages: en"), "{message}");
        }
        other => panic!("{other:?}"),
    }
    supervisor.shutdown();
}

#[path = "support/cpu_fallback.rs"]
mod cpu_fallback;
