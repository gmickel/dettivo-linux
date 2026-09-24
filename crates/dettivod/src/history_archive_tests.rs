use super::*;
use dettivo_session::Policy;
use std::time::Duration;

#[test]
fn rejected_dictation_archive_removes_only_its_new_artifacts() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = Paths::from_env(|key| match key {
        "HOME" | "XDG_RUNTIME_DIR" => Some(tmp.path().into()),
        _ => None,
    });
    let loaded = Loaded::load(&paths.config_file, |_| None);
    let history = History::open(&paths, &loaded).unwrap();
    history.retention.write().unwrap().keep_audio = true;
    history.retention.write().unwrap().artifacts = ArtifactPolicy::Keep;
    let db = rusqlite::Connection::open(loaded.db_path(&paths)).unwrap();
    db.execute_batch("CREATE TRIGGER reject_dictation BEFORE INSERT ON dictations BEGIN SELECT RAISE(ABORT, 'injected archive failure'); END;").unwrap();
    let transcript = Transcript {
        job_id: "job_dict_test".into(),
        id: "00000000-0000-4000-8000-000000000099".into(),
        text: "retained in session memory".into(),
        raw_text: "private raw".into(),
        polished_text: None,
        notice: None,
        policy_hash: None,
        language: "en".into(),
        duration_ms: 100,
        silent: false,
        insertion: None,
        target: None,
        timings: None,
        policy: Policy {
            provider: "whisper".into(),
            model: "tiny".into(),
            model_path: "tiny.bin".into(),
            language: "en".into(),
            mode: "raw".into(),
            vocabulary: vec![],
            replacements: vec![],
            spoken_punctuation: true,
            protect_tokens: true,
            max_duration: Duration::from_secs(10),
            silence_peak_threshold: 0.01,
            keep_audio: true,
            level_interval_ms: 100,
            config_hash: "test".into(),
            rules: Default::default(),
            llm: Default::default(),
        },
    };
    let sibling = history.artifacts.dir("unrelated");
    std::fs::create_dir_all(&sibling).unwrap();
    std::fs::write(sibling.join("metadata.json"), "keep").unwrap();
    let take = tmp.path().join("take");
    std::fs::create_dir_all(&take).unwrap();
    dettivo_storage::seed::write_silence(&take.join("microphone.wav")).unwrap();
    assert!(
        history
            .archive(&transcript, &take)
            .unwrap_err()
            .contains("injected archive failure")
    );
    assert!(!history.artifacts.dir(&transcript.id).exists());
    assert_eq!(
        std::fs::read_to_string(sibling.join("metadata.json")).unwrap(),
        "keep"
    );
}
