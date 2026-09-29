use super::*;
use dettivo_engine_proto::{Backend, LoadParams};
use dettivo_speech::supervisor::WHISPER_BINARY;

fn load(supervisor: &Supervisor, preference: BackendPreference) -> Result<Backend, EngineError> {
    supervisor.with_engine_load(
        WHISPER_BINARY,
        LoadParams {
            model: "/models/fake.bin".into(),
            backend_preference: preference,
            vad_model: None,
            context_length: None,
            lora: None,
            threads: None,
        },
        |_, loaded| Ok(loaded.backend),
    )
}

#[test]
fn auto_load_survives_a_crash_or_corrupt_frame_and_keeps_cpu_warm() {
    for marker in ["crash-on-gpu-load", "malformed-on-gpu-load"] {
        let dir = fake_engine_dir(WHISPER_BINARY);
        std::fs::write(dir.path().join(marker), "").unwrap();
        std::fs::write(dir.path().join("record-loads"), "").unwrap();
        let supervisor = Supervisor::new(Settings {
            force_cpu: false,
            ..settings(dir.path(), Duration::from_secs(60))
        });
        assert_eq!(
            load(&supervisor, BackendPreference::Auto).unwrap(),
            Backend::Cpu
        );
        assert_eq!(
            load(&supervisor, BackendPreference::Auto).unwrap(),
            Backend::Cpu
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("loads.log")).unwrap(),
            "auto\ncpu\n"
        );
        let status = supervisor.status(&[WHISPER_BINARY]);
        assert!(status[0].reason.as_ref().unwrap().contains("CPU fallback"));
        // Explicit GPU selection must still fail, even after an automatic fallback.
        assert!(load(&supervisor, BackendPreference::Vulkan).is_err());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("loads.log")).unwrap(),
            "auto\ncpu\nvulkan\n"
        );
        supervisor.shutdown();
    }
}

#[test]
fn a_failed_cpu_retry_is_bounded_and_cannot_loop_back_to_gpu() {
    let dir = fake_engine_dir(WHISPER_BINARY);
    std::fs::write(dir.path().join("crash-on-load"), "").unwrap();
    std::fs::write(dir.path().join("record-loads"), "").unwrap();
    let supervisor = Supervisor::new(Settings {
        force_cpu: false,
        ..settings(dir.path(), Duration::from_secs(60))
    });
    assert!(load(&supervisor, BackendPreference::Auto).is_err());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("loads.log")).unwrap(),
        "auto\ncpu\n"
    );
    assert!(load(&supervisor, BackendPreference::Auto).is_err());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("loads.log")).unwrap(),
        "auto\ncpu\ncpu\n"
    );
    supervisor.shutdown();
}

/// A GPU engine that aborts mid-request, as ggml does when a game holds
/// the GPU's memory, answers the retry and every later request on the CPU,
/// for any engine; an explicit GPU choice is never moved.
#[test]
fn a_crash_on_the_gpu_moves_the_engine_to_the_cpu() {
    for binary in [WHISPER_BINARY, dettivo_speech::supervisor::PARAKEET_BINARY] {
        let dir = fake_engine_dir(binary);
        std::fs::write(dir.path().join("gpu-oom-on-request"), "").unwrap();
        std::fs::write(dir.path().join("record-loads"), "").unwrap();
        let supervisor = Supervisor::new(Settings {
            force_cpu: false,
            ..settings(dir.path(), Duration::from_secs(60))
        });
        assert!(matches!(
            recognize(&supervisor, binary, "en"),
            Err(EngineError::Crashed(_))
        ));
        assert!(
            recognize(&supervisor, binary, "en")
                .unwrap()
                .starts_with("fake en")
        );
        assert!(
            recognize(&supervisor, binary, "en")
                .unwrap()
                .starts_with("fake en")
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("loads.log")).unwrap(),
            "auto\ncpu\n"
        );
        let status = supervisor.status(&[binary]);
        assert!(status[0].reason.as_ref().unwrap().contains("CPU fallback"));
        supervisor.shutdown();
    }
    let dir = fake_engine_dir(WHISPER_BINARY);
    std::fs::write(dir.path().join("gpu-oom-on-request"), "").unwrap();
    std::fs::write(dir.path().join("record-loads"), "").unwrap();
    let supervisor = Supervisor::new(Settings {
        force_cpu: false,
        ..settings(dir.path(), Duration::from_secs(60))
    });
    let on_vulkan = |s: &Supervisor| {
        s.with_engine_on(
            WHISPER_BINARY,
            "/models/fake.bin",
            None,
            BackendPreference::Vulkan,
            dettivo_speech::supervisor::LlmLoad::default(),
            |process, _| {
                process.call(
                    "recognize",
                    json!({"language": "en"}),
                    &[&[0u8; 3200]],
                    Duration::from_secs(10),
                    |_| {},
                )
            },
        )
    };
    assert!(on_vulkan(&supervisor).is_err());
    assert!(on_vulkan(&supervisor).is_err());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("loads.log")).unwrap(),
        "vulkan\nvulkan\n"
    );
    supervisor.shutdown();
}

/// The language model engine moves itself to the CPU mid-life (ADR 0080);
/// the supervisor's record follows, so status stops naming the GPU.
#[test]
fn a_noted_backend_change_reaches_the_status() {
    let dir = fake_engine_dir(dettivo_speech::supervisor::LLM_BINARY);
    std::fs::write(dir.path().join("gpu-oom-on-request"), "").unwrap();
    let supervisor = Supervisor::new(Settings {
        force_cpu: false,
        ..settings(dir.path(), Duration::from_secs(60))
    });
    let binary = dettivo_speech::supervisor::LLM_BINARY;
    let loaded = supervisor
        .with_engine(binary, "/models/fake.gguf", None, |_, loaded| {
            Ok(loaded.backend)
        })
        .unwrap();
    assert_eq!(loaded, Backend::Vulkan);
    supervisor.note_backend(binary, Backend::Cpu, "the GPU had no room");
    supervisor.note_backend(binary, Backend::Cpu, "the GPU had no room");
    let status = &supervisor.status(&[binary])[0];
    assert_eq!(status.backend, Some(Backend::Cpu));
    let reason = status.reason.as_deref().unwrap();
    assert_eq!(reason.matches("the GPU had no room").count(), 1, "{reason}");
    supervisor.shutdown();
}

/// An engine that answers GPU out of memory instead of crashing (parakeet
/// while a game holds the GPU) is not kept warm on the GPU: the next
/// request runs on the CPU.
#[test]
fn a_gpu_out_of_memory_error_moves_the_engine_to_the_cpu() {
    let binary = dettivo_speech::supervisor::PARAKEET_BINARY;
    let dir = fake_engine_dir(binary);
    std::fs::write(dir.path().join("gpu-oom-error-on-request"), "").unwrap();
    std::fs::write(dir.path().join("record-loads"), "").unwrap();
    let supervisor = Supervisor::new(Settings {
        force_cpu: false,
        ..settings(dir.path(), Duration::from_secs(60))
    });
    let err = recognize(&supervisor, binary, "en").unwrap_err();
    assert!(err.to_string().contains("OutOfDeviceMemory"), "{err}");
    assert!(
        recognize(&supervisor, binary, "en")
            .unwrap()
            .starts_with("fake en")
    );
    assert!(
        recognize(&supervisor, binary, "en")
            .unwrap()
            .starts_with("fake en")
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("loads.log")).unwrap(),
        "auto\ncpu\n"
    );
    supervisor.shutdown();
}
