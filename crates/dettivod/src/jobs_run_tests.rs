use super::*;
use dettivo_core::{config::Loaded, paths::Paths};
use dettivo_engine_proto::{Backend, RecognizeResult};
use dettivo_speech::{Capabilities, EngineError, PreloadSource, RecognizeRequest, SttEngine};
use dettivo_storage::{
    item::SourceKind,
    meetings::{MeetingRow, MeetingStatus},
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

struct SilentEngine;
impl SttEngine for SilentEngine {
    fn provider(&self) -> &str {
        "test"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_timestamps: true,
            supports_streaming: false,
            supported_languages: vec![],
            max_duration_seconds: 3600,
            supports_custom_vocabulary: false,
            supports_model_deletion: false,
        }
    }
    fn preload(&self, _: PreloadSource) -> Result<(), EngineError> {
        Ok(())
    }
    fn recognize(&self, _: RecognizeRequest, _: Duration) -> Result<RecognizeResult, EngineError> {
        panic!("silent input must not reach inference")
    }
    fn cancel(&self) {}
    fn backend(&self) -> Option<Backend> {
        None
    }
}

#[derive(Default)]
struct Archive(AtomicUsize);
impl dettivo_meeting::Archive for Archive {
    fn started(&self, _: &MeetingRow) -> Result<(), String> {
        Ok(())
    }
    fn updated(&self, _: &MeetingRow) -> Result<(), String> {
        Ok(())
    }
    fn settled(&self, _: &MeetingRow, _: bool, _: dettivo_storage::retention::ArtifactPolicy) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn rejected_result_commit_reports_failure_and_preserves_recovery_input() {
    for meeting in [false, true] {
        for refuse_all in [false, true] {
            let tmp = tempfile::tempdir().unwrap();
            let paths = Paths::from_env(|key| match key {
                "HOME" | "XDG_RUNTIME_DIR" => Some(tmp.path().into()),
                _ => None,
            });
            let loaded = Loaded::load(&paths.config_file, |_| None);
            let history = History::open(&paths, &loaded).unwrap();
            let mut item = DictationItem::new(SourceKind::AudioImport);
            item.status = ItemStatus::Transcribing;
            let mut row = MeetingRow::new();
            row.id = item.id.clone();
            row.source_kind = "audioImport".into();
            row.status = MeetingStatus::Transcribing;
            if meeting {
                history.begin_meeting(&row).unwrap();
            } else {
                history.begin_item(&item).unwrap();
            }
            let audio = tmp.path().join("temporary.wav");
            dettivo_storage::seed::write_silence(&audio).unwrap();
            let db = rusqlite::Connection::open(loaded.db_path(&paths)).unwrap();
            let table = if meeting { "meetings" } else { "dictations" };
            let condition = if refuse_all {
                "1"
            } else {
                "NEW.status = 'completed'"
            };
            db.execute_batch(&format!("CREATE TRIGGER reject_result BEFORE UPDATE ON {table} WHEN {condition} BEGIN SELECT RAISE(ABORT, 'injected result write failure'); END;")).unwrap();
            let jobs = Arc::new(Jobs::default());
            let cancel = Arc::new(AtomicBool::new(false));
            *jobs.import_running.lock().unwrap() = Some("job_test".into());
            jobs.running.lock().unwrap().insert(
                "job_test".into(),
                crate::jobs::Running {
                    status: crate::jobs::running("job_test"),
                    item_id: item.id.clone(),
                    cancel: cancel.clone(),
                    settling: false,
                },
            );
            let bus = EventBus::new();
            let (tx, mut rx) = tokio::sync::mpsc::channel(64);
            bus.subscribe(1, vec![dettivo_proto::events::Topic::JobProgress], 64, tx);
            let archive = Arc::new(Archive::default());
            let work = Work {
                job_id: "job_test".into(),
                item: item.clone(),
                audio: audio.clone(),
                discard_audio: true,
                decode: None,
                engine: Arc::new(SilentEngine),
                language: "en".into(),
                prompt: None,
                replacements: vec![],
                spoken_punctuation: false,
                protect_tokens: false,
                settings: dettivo_transcribe::Settings::default(),
                cancel,
                meeting: meeting.then_some(row),
                meeting_retention: meeting
                    .then_some((true, dettivo_storage::retention::ArtifactPolicy::Keep)),
                archive: Some(archive.clone()),
            };
            run(&jobs, &history, &bus, work);
            let mut stages = Vec::new();
            while let Ok(line) = rx.try_recv() {
                let event: serde_json::Value = serde_json::from_str(&line).unwrap();
                stages.push(
                    event["params"]["payload"]["stage"]
                        .as_str()
                        .unwrap()
                        .to_string(),
                );
            }
            assert_eq!(
                stages.last().map(String::as_str),
                Some("failed"),
                "meeting={meeting}, all={refuse_all}: {stages:?}"
            );
            assert!(!stages.iter().any(|s| s == "done"));
            assert_eq!(archive.0.load(Ordering::SeqCst), 0);
            assert!(!history.busy().contains(&item.id));
            assert_eq!(jobs.active(), 0);
            assert!(!jobs.import_in_progress());
            let retained = if meeting {
                history.meeting_artifacts().dir(&item.id)
            } else {
                history.artifacts().dir(&item.id)
            }
            .join(dettivo_storage::retention::AUDIO_FILE);
            assert!(
                retained.is_file(),
                "recoverable WAV: {}",
                retained.display()
            );
            if !refuse_all {
                let code = if meeting {
                    let row = history
                        .with_store(|s| s.get_meeting(&item.id))
                        .unwrap()
                        .unwrap();
                    assert_eq!(row.status, MeetingStatus::Failed);
                    row.error_code
                } else {
                    let row = history.with_store(|s| s.get(&item.id)).unwrap().unwrap();
                    assert_eq!(row.status, ItemStatus::Failed);
                    row.error_code
                };
                assert_eq!(code.as_deref(), Some("storage"));
            }
        }
    }
}

struct SettlementGate(std::sync::Barrier);

impl dettivo_meeting::Archive for SettlementGate {
    fn started(&self, _: &MeetingRow) -> Result<(), String> {
        Ok(())
    }
    fn updated(&self, _: &MeetingRow) -> Result<(), String> {
        Ok(())
    }
    fn completed(&self, _: &mut MeetingRow) {
        self.0.wait();
        self.0.wait();
    }
}

#[test]
fn cancellation_is_accepted_only_before_result_settlement() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../tests/fixtures/meeting-recovery-correctness.json"
    ))
    .unwrap();
    for late in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::from_env(|key| match key {
            "HOME" | "XDG_RUNTIME_DIR" => Some(tmp.path().into()),
            _ => None,
        });
        let loaded = Loaded::load(&paths.config_file, |_| None);
        let history = Arc::new(History::open(&paths, &loaded).unwrap());
        let mut item = DictationItem::new(SourceKind::AudioImport);
        item.status = ItemStatus::Transcribing;
        let mut row = MeetingRow::new();
        row.id = item.id.clone();
        row.source_kind = "audioImport".into();
        row.status = MeetingStatus::Transcribing;
        row.notes_markdown = "User notes survive settlement".into();
        history.begin_meeting(&row).unwrap();
        let audio = history
            .meeting_artifacts()
            .dir(&row.id)
            .join(dettivo_storage::retention::AUDIO_FILE);
        dettivo_storage::seed::write_silence(&audio).unwrap();
        let jobs = Arc::new(Jobs::default());
        let cancel = Arc::new(AtomicBool::new(false));
        jobs.running.lock().unwrap().insert(
            "job_test".into(),
            crate::jobs::Running {
                status: crate::jobs::running("job_test"),
                item_id: item.id.clone(),
                cancel: cancel.clone(),
                settling: false,
            },
        );
        let gate = Arc::new(SettlementGate(std::sync::Barrier::new(2)));
        let bus = Arc::new(EventBus::new());
        let (tx, mut rx) = tokio::sync::mpsc::channel(64);
        bus.subscribe(1, vec![dettivo_proto::events::Topic::JobProgress], 64, tx);
        let work = Work {
            job_id: "job_test".into(),
            item: item.clone(),
            audio: audio.clone(),
            discard_audio: false,
            decode: None,
            engine: Arc::new(SilentEngine),
            language: "en".into(),
            prompt: None,
            replacements: vec![],
            spoken_punctuation: false,
            protect_tokens: false,
            settings: dettivo_transcribe::Settings::default(),
            cancel: cancel.clone(),
            meeting: Some(row),
            meeting_retention: Some((true, dettivo_storage::retention::ArtifactPolicy::Keep)),
            archive: late.then_some(gate.clone()),
        };
        if !late {
            assert!(
                jobs.cancel(&item.id, dettivo_proto::runtime::RefKind::Meeting)
                    .is_ok()
            );
            run(&jobs, &history, &bus, work);
        } else {
            let (j, h, b) = (jobs.clone(), history.clone(), bus.clone());
            let worker = std::thread::spawn(move || run(&j, &h, &b, work));
            gate.0.wait();
            assert_eq!(
                history
                    .with_store(|s| s.get_meeting(&item.id))
                    .unwrap()
                    .unwrap()
                    .status,
                MeetingStatus::Transcribing
            );
            let refused = jobs.cancel(&item.id, dettivo_proto::runtime::RefKind::Meeting);
            gate.0.wait();
            worker.join().unwrap();
            assert!(
                refused.is_err(),
                "cancellation cannot be accepted after settlement began"
            );
            assert_eq!(
                serde_json::to_value(refused.unwrap_err()).unwrap()["data"]["details"]["kind"],
                contract["late_cancel_error"]
            );
            assert!(!cancel.load(Ordering::SeqCst));
        }
        let row = history
            .with_store(|s| s.get_meeting(&item.id))
            .unwrap()
            .unwrap();
        assert_eq!(
            row.status,
            if late {
                MeetingStatus::Completed
            } else {
                MeetingStatus::Cancelled
            }
        );
        assert_eq!(row.notes_markdown, "User notes survive settlement");
        assert!(audio.is_file());
        assert!(!history.busy().contains(&row.id));
        assert_eq!(jobs.active(), 0);
        let mut terminal = None;
        while let Ok(line) = rx.try_recv() {
            let event: serde_json::Value = serde_json::from_str(&line).unwrap();
            terminal = Some(
                event["params"]["payload"]["stage"]
                    .as_str()
                    .unwrap()
                    .to_string(),
            );
        }
        assert_eq!(
            terminal.as_deref(),
            Some(if late { "done" } else { "cancelled" })
        );
    }
}
