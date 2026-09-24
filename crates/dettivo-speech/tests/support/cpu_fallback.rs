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
