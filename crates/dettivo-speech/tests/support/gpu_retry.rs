//! An engine moved to the CPU by a full GPU goes back to the GPU once a
//! hold has passed, at the start of a request (fn-77 R3, R4). The fake
//! engine reads its GPU markers on every request, so swapping
//! `gpu-oom-*-on-request` for `gpu-has-room` frees the GPU.

use super::*;
use dettivo_engine_proto::{Backend, LoadParams};
use dettivo_speech::supervisor::{PARAKEET_BINARY, WHISPER_BINARY};

fn supervisor(dir: &Path, hold: Duration) -> std::sync::Arc<Supervisor> {
    Supervisor::new(Settings {
        force_cpu: false,
        gpu_retry_hold: hold,
        ..settings(dir, Duration::from_secs(60))
    })
}

fn loads(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("loads.log")).unwrap()
}

fn free_the_gpu(dir: &Path, marker: &str) {
    std::fs::remove_file(dir.join(marker)).unwrap();
    std::fs::write(dir.join("gpu-has-room"), "").unwrap();
}

fn fill_the_gpu(dir: &Path, marker: &str) {
    std::fs::remove_file(dir.join("gpu-has-room")).unwrap();
    std::fs::write(dir.join(marker), "").unwrap();
}

fn backend(supervisor: &Supervisor, binary: &str) -> Option<Backend> {
    supervisor.status(&[binary])[0].backend
}

/// A crash on a full GPU holds the engine on the CPU; the first request
/// after the hold respawns it and it loads on the GPU again.
#[test]
fn a_crashed_engine_returns_to_the_gpu_after_the_hold() {
    let dir = fake_engine_dir(WHISPER_BINARY);
    std::fs::write(dir.path().join("gpu-oom-on-request"), "").unwrap();
    std::fs::write(dir.path().join("record-loads"), "").unwrap();
    let supervisor = supervisor(dir.path(), Duration::from_secs(2));
    let before = Instant::now();
    let Err(EngineError::Crashed(tail)) = recognize(&supervisor, WHISPER_BINARY, "en") else {
        panic!("the GPU request crashes");
    };
    let crashed = Instant::now();
    // The crash warning names the Vulkan error (R5).
    let logged = dettivo_speech::process::diagnostic_lines(&tail);
    assert!(
        logged.contains("  what():  vk::Device::allocateMemory: ErrorOutOfDeviceMemory"),
        "{logged}"
    );
    assert!(recognize(&supervisor, WHISPER_BINARY, "en").is_ok());
    assert!(recognize(&supervisor, WHISPER_BINARY, "en").is_ok());
    assert!(
        before.elapsed() < Duration::from_secs(2),
        "checked inside the hold"
    );
    assert_eq!(loads(dir.path()), "auto\ncpu\n");
    assert_eq!(backend(&supervisor, WHISPER_BINARY), Some(Backend::Cpu));
    free_the_gpu(dir.path(), "gpu-oom-on-request");
    std::thread::sleep(Duration::from_millis(2050).saturating_sub(crashed.elapsed()));
    assert!(recognize(&supervisor, WHISPER_BINARY, "en").is_ok());
    assert_eq!(loads(dir.path()), "auto\ncpu\nauto\n");
    assert_eq!(backend(&supervisor, WHISPER_BINARY), Some(Backend::Vulkan));
    supervisor.shutdown();
}

/// A GPU that still has no room answers the retry with the same error:
/// the request that hit it is re-run on the CPU as before, the hold
/// doubles, and a request served on the GPU starts the hold over.
#[test]
fn a_failed_retry_doubles_the_hold_and_a_working_gpu_resets_it() {
    const OOM: &str = "gpu-oom-error-on-request";
    let hold = Duration::from_millis(400);
    let dir = fake_engine_dir(PARAKEET_BINARY);
    std::fs::write(dir.path().join(OOM), "").unwrap();
    std::fs::write(dir.path().join("record-loads"), "").unwrap();
    let supervisor = supervisor(dir.path(), hold);
    let oom = |s: &Supervisor| {
        let err = recognize(s, PARAKEET_BINARY, "en").unwrap_err();
        assert!(err.to_string().contains("OutOfDeviceMemory"), "{err}");
    };
    let on_cpu = |s: &Supervisor| {
        assert!(recognize(s, PARAKEET_BINARY, "en").is_ok());
        assert_eq!(backend(s, PARAKEET_BINARY), Some(Backend::Cpu));
    };
    oom(&supervisor);
    on_cpu(&supervisor);
    std::thread::sleep(hold + Duration::from_millis(50));
    oom(&supervisor);
    let second = Instant::now();
    on_cpu(&supervisor);
    assert_eq!(loads(dir.path()), "auto\ncpu\nauto\ncpu\n");
    // The hold is now 800 ms: at 450 ms the engine stays on the CPU.
    std::thread::sleep((hold + Duration::from_millis(50)).saturating_sub(second.elapsed()));
    on_cpu(&supervisor);
    assert!(
        second.elapsed() < hold * 2,
        "checked inside the doubled hold"
    );
    assert_eq!(loads(dir.path()), "auto\ncpu\nauto\ncpu\n");
    free_the_gpu(dir.path(), OOM);
    std::thread::sleep((hold * 2 + Duration::from_millis(50)).saturating_sub(second.elapsed()));
    assert!(recognize(&supervisor, PARAKEET_BINARY, "en").is_ok());
    assert_eq!(backend(&supervisor, PARAKEET_BINARY), Some(Backend::Vulkan));
    assert_eq!(loads(dir.path()), "auto\ncpu\nauto\ncpu\nauto\n");
    // The GPU worked, so the next failure holds for the first hold again.
    fill_the_gpu(dir.path(), OOM);
    oom(&supervisor);
    let third = Instant::now();
    on_cpu(&supervisor);
    free_the_gpu(dir.path(), OOM);
    std::thread::sleep((hold + Duration::from_millis(50)).saturating_sub(third.elapsed()));
    assert!(recognize(&supervisor, PARAKEET_BINARY, "en").is_ok());
    assert_eq!(backend(&supervisor, PARAKEET_BINARY), Some(Backend::Vulkan));
    assert!(third.elapsed() < hold * 2, "retried after the first hold");
    supervisor.shutdown();
}

/// A retry whose GPU load itself aborts answers the same request on the
/// CPU, and doubles the hold.
#[test]
fn a_retry_that_crashes_on_load_answers_the_request_on_the_cpu() {
    let dir = fake_engine_dir(WHISPER_BINARY);
    std::fs::write(dir.path().join("crash-on-gpu-load"), "").unwrap();
    std::fs::write(dir.path().join("record-loads"), "").unwrap();
    let hold = Duration::from_millis(300);
    let supervisor = supervisor(dir.path(), hold);
    let load = |s: &Supervisor| {
        s.with_engine_load(
            WHISPER_BINARY,
            LoadParams {
                model: "/models/fake.bin".into(),
                backend_preference: BackendPreference::Auto,
                vad_model: None,
                context_length: None,
                lora: None,
                threads: None,
            },
            |_, loaded| Ok(loaded.backend),
        )
        .unwrap()
    };
    assert_eq!(load(&supervisor), Backend::Cpu);
    std::thread::sleep(hold + Duration::from_millis(50));
    assert_eq!(load(&supervisor), Backend::Cpu);
    let retried = Instant::now();
    assert_eq!(loads(dir.path()), "auto\ncpu\nauto\ncpu\n");
    std::thread::sleep((hold + Duration::from_millis(50)).saturating_sub(retried.elapsed()));
    assert_eq!(load(&supervisor), Backend::Cpu);
    assert!(retried.elapsed() < hold * 2);
    assert_eq!(
        loads(dir.path()),
        "auto\ncpu\nauto\ncpu\n",
        "the doubled hold held"
    );
    supervisor.shutdown();
}
