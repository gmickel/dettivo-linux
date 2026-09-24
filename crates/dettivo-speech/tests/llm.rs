//! R1 and R4 for the `LlmEngine` over real processes: a `generate`
//! through the supervisor returns the engine's answer, a generation that
//! outlives its timeout is cancelled and reported as such, a crash
//! mid-generation is recorded as a crash and the next request restarts
//! the engine, and `status` reads the running process without loading.
//! The scripted engine is `dettivo-engine-proto`'s `fake-engine` example
//! linked in under the LLM binary's name; the idle-unload and VRAM run
//! against the real engine lives in `crates/dettivod/tests/llm_local.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use dettivo_engine_proto::BackendPreference;
use dettivo_speech::EngineError;
use dettivo_speech::llm::{GenerateRequest, LlmEngine};
use dettivo_speech::supervisor::{LLM_BINARY, Settings, Supervisor};

fn target_dir() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    dir.pop();
    dir.join("target/debug")
}

/// The scripted engine under the LLM binary's name (see
/// `tests/supervisor.rs` for why it is copied once and linked per test).
fn fake_llm_dir() -> tempfile::TempDir {
    static COPY: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
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
        let dir = target_dir().join(format!("qa-fake-llm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let copy = dir.join("fake-engine");
        std::fs::copy(&example, &copy).unwrap();
        copy
    });
    let dir = tempfile::tempdir_in(shared.parent().unwrap()).unwrap();
    let link = dir.path().join(LLM_BINARY);
    if std::fs::hard_link(shared, &link).is_err() {
        std::fs::copy(shared, &link).unwrap();
    }
    dir
}

fn engine(dir: &Path) -> (std::sync::Arc<Supervisor>, LlmEngine) {
    let supervisor = Supervisor::new(Settings {
        directory: Some(dir.to_path_buf()),
        stt_idle: Duration::from_secs(60),
        llm_idle: Duration::from_secs(60),
        force_cpu: true,
        load_timeout: Duration::from_secs(10),
    });
    let engine = LlmEngine::new(
        supervisor.clone(),
        "/models/fake.gguf".into(),
        BackendPreference::Cpu,
        Some(2048),
    );
    (supervisor, engine)
}

fn request(user: &str) -> GenerateRequest {
    GenerateRequest {
        system: Some("rewrite".into()),
        user: user.into(),
        max_tokens: 32,
        temperature: 0.0,
        stop: Vec::new(),
        raw: false,
    }
}

#[test]
fn busy_engine_status_tier_and_reaper_do_not_wait_for_inference() {
    let dir = fake_llm_dir();
    let (supervisor, engine) = engine(dir.path());
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let inference = {
        let supervisor = supervisor.clone();
        std::thread::spawn(move || {
            supervisor.with_engine(LLM_BINARY, "/models/fake.gguf", None, |_, _| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(())
            })
        })
    };
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let (observed_tx, observed_rx) = std::sync::mpsc::channel();
    let probe = {
        let supervisor = supervisor.clone();
        std::thread::spawn(move || {
            let status = engine.status();
            let tier = supervisor.tier(&[LLM_BINARY]);
            let reaped = supervisor.reap_idle();
            observed_tx.send((status, tier, reaped)).unwrap();
        })
    };
    let observed = observed_rx.recv_timeout(Duration::from_millis(500));
    release_tx.send(()).unwrap();
    inference.join().unwrap().unwrap();
    probe.join().unwrap();
    supervisor.shutdown();
    let (status, _, reaped) = observed.expect(
        "status, capabilities tier and idle checks must return while inference holds the slot",
    );
    assert!(status.busy && status.loaded && status.engine.running);
    assert_eq!(status.engine.model.as_deref(), Some("/models/fake.gguf"));
    assert_eq!(reaped, 0);
}

#[test]
fn a_generate_goes_through_the_supervisor_and_status_reads_the_process() {
    let dir = fake_llm_dir();
    let (supervisor, engine) = engine(dir.path());
    let before = engine.status();
    assert!(!before.engine.running && !before.loaded && !before.busy);
    assert!(before.engine.path.is_some(), "the binary is found");

    let out = engine
        .generate(&request("hello team"), Duration::from_secs(10))
        .unwrap();
    assert_eq!(out.text, "HELLO TEAM");
    assert_eq!(out.tokens, 2);
    assert_eq!(out.backend, dettivo_engine_proto::Backend::Cpu);
    assert_eq!(engine.model(), "/models/fake.gguf");
    assert_eq!(engine.backend(), Some(dettivo_engine_proto::Backend::Cpu));

    let after = engine.status();
    assert!(after.engine.running, "{after:?}");
    assert!(after.loaded && !after.busy, "{after:?}");
    assert_eq!(after.engine.model.as_deref(), Some("/models/fake.gguf"));
    supervisor.shutdown();
    let gone = engine.status();
    assert!(!gone.engine.running && !gone.loaded);
}

#[test]
fn a_generation_past_its_timeout_is_cancelled_and_the_engine_serves_the_next_request() {
    let dir = fake_llm_dir();
    let (supervisor, engine) = engine(dir.path());
    let started = Instant::now();
    let err = engine
        .generate(&request("__SLOW__"), Duration::from_millis(300))
        .unwrap_err();
    match err {
        EngineError::Transport(m) => assert!(m.contains("cancelled"), "{m}"),
        other => panic!("{other:?}"),
    }
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "bounded by the timeout and the cancel"
    );
    // The engine finishes its slow answer, then serves the next request.
    let out = engine
        .generate(&request("next"), Duration::from_secs(10))
        .unwrap();
    assert_eq!(out.text, "NEXT");
    supervisor.shutdown();
}

/// An engine that cannot finish on its own (thirty seconds of silence,
/// past every deadline here) and cannot answer the cancel either is
/// terminated, and the next request restarts it; the whole sequence is
/// bounded by the timeout and the cancel grace, never by the engine.
#[test]
fn an_engine_that_ignores_the_cancel_is_terminated_and_the_next_request_restarts_it() {
    let dir = fake_llm_dir();
    let (supervisor, engine) = engine(dir.path());
    let started = Instant::now();
    let err = engine
        .generate(&request("__HANG__"), Duration::from_millis(300))
        .unwrap_err();
    match err {
        EngineError::Transport(m) => assert!(m.contains("cancelled"), "{m}"),
        other => panic!("{other:?}"),
    }
    let out = engine
        .generate(&request("next"), Duration::from_secs(10))
        .unwrap();
    assert_eq!(out.text, "NEXT");
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "bounded by the timeout, the cancel grace and a restart, not by the engine: {:?}",
        started.elapsed()
    );
    supervisor.shutdown();
}

#[test]
fn a_crash_mid_generation_is_recorded_and_the_next_request_restarts_the_engine() {
    let dir = fake_llm_dir();
    let (supervisor, engine) = engine(dir.path());
    let err = engine
        .generate(&request("__CRASH__"), Duration::from_secs(10))
        .unwrap_err();
    assert!(matches!(err, EngineError::Crashed(_)), "{err:?}");
    let rows = supervisor.status(&[LLM_BINARY]);
    assert_eq!(rows[0].crashes, 1);
    assert!(!rows[0].running);
    let out = engine
        .generate(&request("again"), Duration::from_secs(10))
        .unwrap();
    assert_eq!(out.text, "AGAIN");
    assert_eq!(
        supervisor.status(&[LLM_BINARY])[0].crashes,
        0,
        "a load clears the count"
    );
    supervisor.shutdown();
}

/// R4 on this machine: with a real `dettivo-engine-llm` in
/// `DETTIVO_LLM_ENGINE_DIR` (a Vulkan build on a development machine) and
/// `llm/qwen3-1.7b` on disk, the model loads, a rewrite runs, the engine
/// idles past a one-second `llm_idle`, the reaper unloads and ends the
/// process, the status reports it gone, and the device memory a fresh
/// engine reports afterwards is back under what the loaded engine held.
/// The numbers land in `target/llm-idle-unload-report.json`.
#[test]
fn the_real_engine_is_unloaded_after_the_idle_timeout_and_the_device_memory_drops() {
    let Some(dir) = std::env::var_os("DETTIVO_LLM_ENGINE_DIR").map(PathBuf::from) else {
        eprintln!("skip: DETTIVO_LLM_ENGINE_DIR not set (a directory holding dettivo-engine-llm)");
        return;
    };
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap();
    let model = data.join("dettivo/models/llm/qwen3-1.7b/Qwen3-1.7B-Q4_K_M.gguf");
    if !model.is_file() {
        eprintln!("skip: llm/qwen3-1.7b missing (run scripts/models/fetch-llm-test-model.sh)");
        return;
    }
    let binary = dir.join(LLM_BINARY);
    let read_memory = |what: &str| -> Option<u64> {
        let mut process = dettivo_speech::process::EngineProcess::spawn(&binary, false).unwrap();
        let v = process
            .call(
                "status",
                serde_json::json!({}),
                &[],
                Duration::from_secs(10),
                |_| {},
            )
            .unwrap();
        process.terminate();
        let memory = v["memory_bytes"].as_u64();
        eprintln!("{what}: memory_bytes={memory:?}");
        memory
    };
    let before = read_memory("before the load");
    let supervisor = Supervisor::new(Settings {
        directory: Some(dir.clone()),
        stt_idle: Duration::from_secs(60),
        llm_idle: Duration::from_secs(1),
        force_cpu: false,
        load_timeout: Duration::from_secs(120),
    });
    let engine = LlmEngine::new(
        supervisor.clone(),
        model.to_string_lossy().into_owned(),
        BackendPreference::Auto,
        Some(2048),
    );
    let started = Instant::now();
    let out = engine
        .generate(
            &GenerateRequest {
                system: Some(
                    "You rewrite dictated text. Answer with the rewritten text only.".into(),
                ),
                user: "um so i think we should ship this thing tomorrow".into(),
                max_tokens: 48,
                temperature: 0.0,
                stop: Vec::new(),
                raw: false,
            },
            Duration::from_secs(120),
        )
        .unwrap();
    let generate_ms = started.elapsed().as_millis();
    let loaded = engine.status();
    assert!(loaded.engine.running && loaded.loaded, "{loaded:?}");
    let backend = loaded.engine.backend;
    let reason = loaded.engine.reason.clone();
    let during = loaded.memory_bytes;
    eprintln!(
        "loaded: backend={backend:?} reason={reason:?} memory_bytes={during:?} generate_ms={generate_ms}"
    );
    std::thread::sleep(Duration::from_millis(1300));
    assert_eq!(supervisor.reap_idle(), 1, "the idle engine is unloaded");
    let gone = engine.status();
    assert!(!gone.engine.running && !gone.loaded, "{gone:?}");
    assert_eq!(gone.engine.model, None);
    let after = read_memory("after the unload");
    if let (Some(during), Some(after)) = (during, after) {
        assert!(after < during, "device memory dropped: {during} -> {after}");
    }
    let report = serde_json::json!({
        "engine": binary.display().to_string(),
        "model": "llm/qwen3-1.7b",
        "backend": backend.map(|b| format!("{b:?}").to_lowercase()),
        "reason": reason,
        "text": out.text,
        "generate_ms": generate_ms,
        "memory_bytes_before_load": before,
        "memory_bytes_loaded": during,
        "memory_bytes_after_unload": after,
        "llm_idle_seconds": 1,
        "process_gone": !gone.engine.running,
    });
    let mut out_path = target_dir();
    out_path.pop();
    std::fs::write(
        out_path.join("llm-idle-unload-report.json"),
        serde_json::to_string_pretty(&report).unwrap() + "\n",
    )
    .unwrap();
    eprintln!("{report}");
    supervisor.shutdown();
}
