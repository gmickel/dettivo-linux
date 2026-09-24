//! Tests for the QA environment parser (`qa.rs`), kept beside it so the
//! parser stays under the file-length limit.

use super::*;

fn vars(pairs: &[(&str, &str)]) -> Vec<(String, OsString)> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), OsString::from(v)))
        .collect()
}

#[test]
fn off_by_default_and_every_switch_turns_it_on() {
    assert!(
        !QaEnv::from_vars(&vars(&[]), BuildKind::Debug)
            .unwrap()
            .enabled
    );
    for switch in ["DETTIVO_QA_MODE", "DETTIVO_QA", "DETTIVO_MOCK_MODE"] {
        let qa = QaEnv::from_vars(&vars(&[(switch, "1")]), BuildKind::Debug).unwrap();
        assert!(qa.enabled, "{switch}");
    }
}

#[test]
fn release_builds_refuse_qa_mode_unless_allowed() {
    let err = QaEnv::from_vars(&vars(&[("DETTIVO_QA_MODE", "1")]), BuildKind::Release).unwrap_err();
    assert_eq!(err.variable, "DETTIVO_QA_MODE");
    let qa = QaEnv::from_vars(
        &vars(&[("DETTIVO_QA_MODE", "1"), ("DETTIVO_QA_ALLOW_RELEASE", "1")]),
        BuildKind::Release,
    )
    .unwrap();
    assert!(qa.enabled && qa.allow_release);
}

#[test]
fn unknown_reserved_names_are_rejected_by_name() {
    for name in [
        "DETTIVO_MOCK_CAMERA",
        "DETTIVO_E2E_JUMP",
        "DETTIVO_QA_TURBO",
    ] {
        let err = QaEnv::from_vars(
            &vars(&[("DETTIVO_QA_MODE", "1"), (name, "1")]),
            BuildKind::Debug,
        )
        .unwrap_err();
        assert_eq!(err.variable, name);
    }
    assert!(QaEnv::from_vars(&vars(&[("DETTIVO_IPC_SOCKET", "/x")]), BuildKind::Debug).is_ok());
}

#[test]
fn hooks_need_qa_mode() {
    let err = QaEnv::from_vars(&vars(&[("DETTIVO_E2E_SEED", "1")]), BuildKind::Debug).unwrap_err();
    assert_eq!(err.variable, "DETTIVO_E2E_SEED");
    assert!(err.message.contains("DETTIVO_QA_MODE"));
}

#[test]
fn every_hook_parses_and_bad_values_name_the_variable() {
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("mic.wav");
    std::fs::write(&wav, b"RIFF").unwrap();
    let qa = QaEnv::from_vars(
        &vars(&[
            ("DETTIVO_QA_MODE", "1"),
            ("DETTIVO_MOCK_MIC", wav.to_str().unwrap()),
            ("DETTIVO_MOCK_SYSTEM_AUDIO", wav.to_str().unwrap()),
            ("DETTIVO_MOCK_INSERT", "1"),
            ("DETTIVO_MOCK_INPUT", "1"),
            ("DETTIVO_MOCK_A11Y", "1"),
            ("DETTIVO_MOCK_LLM", "fixture:/tmp/llm"),
            (
                "DETTIVO_MOCK_ENGINE",
                "whisper=fixture:/tmp/w, llm=fixture:/tmp/l",
            ),
            ("DETTIVO_FORCE_CPU", "1"),
            ("DETTIVO_MODEL_SERVER", "http://127.0.0.1:8080"),
            ("DETTIVO_E2E_COMPLETE", "1"),
            ("DETTIVO_E2E_STEP", "models"),
            ("DETTIVO_E2E_SEED", "1"),
            ("DETTIVO_E2E_OPEN", "settings.audio"),
            ("DETTIVO_E2E_ROUTE", "devices"),
            ("DETTIVO_E2E_EXPORT_DIR", "/tmp/exports"),
            ("DETTIVO_E2E_OSD_STATE", "inserted"),
            ("DETTIVO_QA_PACING", "/tmp/pacing.json"),
            ("DETTIVO_QA_PLANT", "basic_control"),
            ("DETTIVO_QA_CANARY", "1"),
        ]),
        BuildKind::Debug,
    )
    .unwrap();
    assert_eq!(qa.mock_mic.as_deref(), Some(wav.as_path()));
    assert!(qa.mock_insert && qa.mock_input && qa.mock_a11y && qa.force_cpu);
    assert_eq!(
        qa.mock_llm,
        Some(MockLlm::Fixture(PathBuf::from("/tmp/llm")))
    );
    assert_eq!(qa.mock_engines.len(), 2);
    assert_eq!(qa.mock_engines[1].0, "llm");
    assert_eq!(qa.model_server.as_deref(), Some("http://127.0.0.1:8080"));
    assert!(qa.e2e_complete && qa.e2e_seed);
    assert_eq!(qa.e2e_step, Some(Step::Models));
    assert_eq!(qa.e2e_open, Some(Route::Settings(Some("audio".into()))));
    assert_eq!(qa.e2e_route.as_deref(), Some("devices"));
    assert_eq!(
        qa.e2e_export_dir.as_deref(),
        Some(std::path::Path::new("/tmp/exports"))
    );
    assert_eq!(qa.e2e_osd_state.as_deref(), Some("inserted"));
    assert_eq!(
        qa.qa_pacing.as_deref(),
        Some(std::path::Path::new("/tmp/pacing.json"))
    );
    assert_eq!(qa.qa_plant.as_deref(), Some("basic_control"));
    assert!(qa.qa_canary);

    let implied = QaEnv::from_vars(&vars(&[("DETTIVO_MOCK_MODE", "1")]), BuildKind::Debug).unwrap();
    assert!(implied.mock_insert, "mock mode implies the mock inserter");
    let kept = QaEnv::from_vars(
        &vars(&[("DETTIVO_MOCK_MODE", "1"), ("DETTIVO_MOCK_INSERT", "0")]),
        BuildKind::Debug,
    )
    .unwrap();
    assert!(!kept.mock_insert, "an explicit 0 keeps the real chain");
    assert_eq!(
        QaEnv::from_vars(
            &vars(&[("DETTIVO_QA_MODE", "1"), ("DETTIVO_MOCK_LLM", "echo")]),
            BuildKind::Debug
        )
        .unwrap()
        .mock_llm,
        Some(MockLlm::Echo)
    );
    let bad = [
        ("DETTIVO_MOCK_MIC", "/nonexistent/x.wav"),
        ("DETTIVO_MOCK_LLM", "parrot"),
        ("DETTIVO_MOCK_ENGINE", "whisper"),
        ("DETTIVO_MODEL_SERVER", "ftp://x"),
        ("DETTIVO_E2E_STEP", "finish"),
        ("DETTIVO_E2E_OPEN", "garage"),
        ("DETTIVO_E2E_OSD_STATE", "sleeping"),
        ("DETTIVO_QA_PLANT", "serif_label"),
    ];
    for (var, value) in bad {
        let err = QaEnv::from_vars(
            &vars(&[("DETTIVO_QA_MODE", "1"), (var, value)]),
            BuildKind::Debug,
        )
        .unwrap_err();
        assert_eq!(err.variable, var, "{value}");
    }
}

#[test]
fn the_known_list_covers_every_prefix_and_docs_table() {
    for name in KNOWN {
        assert!(
            RESERVED_PREFIXES.iter().any(|p| name.starts_with(p)) || name.starts_with("DETTIVO_"),
            "{name}"
        );
    }
    let docs =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/qa.md")).unwrap();
    for name in KNOWN {
        assert!(
            docs.contains(&format!("`{name}")),
            "{name} is not in docs/qa.md"
        );
    }
}
