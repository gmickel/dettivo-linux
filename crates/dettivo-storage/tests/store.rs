//! R1: migrations from empty and from every recorded version, WAL and
//! foreign keys on, every field round-trips, delete per artifact policy,
//! the retention sweep, concurrent readers during a write, a busy database
//! and a failed migration that leaves its backup and names itself.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dettivo_storage::dictations::ListFilter;
use dettivo_storage::item::{DictationItem, ItemStatus, SourceKind};
use dettivo_storage::migrate::{self, Migration};
use dettivo_storage::retention::{self, ArtifactPolicy, Artifacts, Retention};
use dettivo_storage::time::unix_from_iso;
use dettivo_storage::{Store, StoreError};
use serde_json::json;

fn full_item() -> DictationItem {
    let mut item = DictationItem::new(SourceKind::Rerun);
    item.app_id = "org.gnome.TextEditor".into();
    item.app_name = "Text Editor".into();
    item.status = ItemStatus::Failed;
    item.mode = "raw".into();
    item.stt_provider = "whisper".into();
    item.stt_model = "tiny.en".into();
    item.language = "en".into();
    item.raw_text = "hello comma world".into();
    item.final_text = "Hello, world.".into();
    item.title = "Hello".into();
    item.summary = "A greeting".into();
    item.duration_ms = 1234;
    item.error_code = Some("engine".into());
    item.error_message = Some("crashed".into());
    item.audio_path = Some("/tmp/x.wav".into());
    item.insertion = Some(json!({"outcome": "inserted", "method": "paste"}));
    item
}

fn wav(path: &Path, samples: &[i16]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for s in samples {
        w.write_sample(*s).unwrap();
    }
    w.finalize().unwrap();
}

#[test]
fn an_empty_database_migrates_to_the_current_version_with_wal_and_foreign_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dettivo.db");
    let store = Store::open(&path).unwrap();
    assert_eq!(store.schema_version().unwrap(), migrate::CURRENT_VERSION);
    assert_eq!(
        store.last_migration().unwrap().as_deref(),
        Some(migrate::MIGRATIONS.last().unwrap().name)
    );
    assert_eq!(store.pragmas().unwrap(), ("wal".to_string(), true));
    assert!(store.size_bytes() > 0);
    assert!(
        !path.with_extension("db.bak-0").exists(),
        "no backup for a new file"
    );
    drop(store);
    // A second open runs nothing and changes nothing.
    let again = Store::open(&path).unwrap();
    assert_eq!(again.schema_version().unwrap(), migrate::CURRENT_VERSION);
    assert_eq!(again.count().unwrap(), 0);
}

/// Every `tests/fixtures/v<N>.sql` is a database dumped at schema version
/// N; opening it must migrate to the current version and keep its rows.
#[test]
fn every_recorded_version_migrates_forward_and_keeps_its_rows() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut checked = 0;
    for entry in std::fs::read_dir(&fixtures).unwrap().flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let Some(version) = name
            .strip_prefix('v')
            .and_then(|s| s.strip_suffix(".sql"))
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("dettivo.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(&std::fs::read_to_string(&path).unwrap())
            .unwrap();
        assert_eq!(migrate::current_version(&conn).unwrap(), version, "{name}");
        drop(conn);
        let store = Store::open(&db).unwrap();
        assert_eq!(
            store.schema_version().unwrap(),
            migrate::CURRENT_VERSION,
            "{name}"
        );
        let item = store
            .get("11111111-1111-4111-8111-111111111111")
            .unwrap()
            .unwrap_or_else(|| panic!("{name}: the fixture row is gone"));
        assert_eq!(item.final_text, "Hello, world.");
        assert_eq!(
            store.search("hello", 10, None).unwrap().hits.len(),
            1,
            "{name}"
        );
        if version >= 4 {
            // The meeting row and its rebuilt FTS entry survive 0005.
            let meeting = store
                .get_meeting("22222222-2222-4222-8222-222222222222")
                .unwrap()
                .unwrap_or_else(|| panic!("{name}: the meeting row is gone"));
            assert_eq!(meeting.final_text, "The budget is agreed.");
            assert!(meeting.notes_markdown.is_empty() && meeting.analysis.is_none());
            let hits = store.search_meetings("budget", 10).unwrap();
            assert_eq!(hits.len(), 1, "{name}");
            assert_eq!(hits[0].matched_field, "title");
        }
        if version < migrate::CURRENT_VERSION {
            let mut backup = db.as_os_str().to_owned();
            backup.push(format!(".bak-{version}"));
            assert!(PathBuf::from(backup).is_file(), "{name}: backup missing");
        }
        checked += 1;
    }
    assert!(
        checked >= 1,
        "no version fixtures under {}",
        fixtures.display()
    );
}

#[test]
fn a_failed_migration_names_itself_and_leaves_the_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dettivo.db");
    let store = Store::open(&path).unwrap();
    store.insert(&full_item()).unwrap();
    drop(store);
    let mut migrations: Vec<Migration> = migrate::MIGRATIONS.to_vec();
    migrations.push(Migration {
        version: migrate::CURRENT_VERSION + 1,
        name: "9999-broken",
        sql: "CREATE TABLE dictations (again TEXT);",
    });
    let err = Store::open_with(&path, &migrations, Duration::from_millis(100)).unwrap_err();
    let StoreError::Migration {
        name,
        backup,
        message,
    } = err.clone()
    else {
        panic!("{err}");
    };
    assert_eq!(name, "9999-broken");
    assert!(message.contains("already exists"), "{message}");
    let backup = backup.expect("backup path");
    assert!(backup.is_file());
    assert!(
        backup
            .to_string_lossy()
            .ends_with(&format!("dettivo.db.bak-{}", migrate::CURRENT_VERSION))
    );
    assert!(err.to_string().contains("9999-broken") && err.to_string().contains(".bak-"));
    // The database is untouched and the backup opens with the row in it.
    let store = Store::open(&path).unwrap();
    assert_eq!(store.count().unwrap(), 1);
    let restored = Store::open(&backup).unwrap();
    assert_eq!(restored.count().unwrap(), 1);
}

#[test]
fn every_field_reads_back_and_updates_stick() {
    let store = Store::in_memory().unwrap();
    let mut item = full_item();
    store.insert(&item).unwrap();
    assert_eq!(store.get(&item.id).unwrap().unwrap(), item);
    assert_eq!(store.latest().unwrap().unwrap().id, item.id);
    item.final_text = "Changed.".into();
    item.status = ItemStatus::Completed;
    item.error_code = None;
    item.error_message = None;
    item.audio_path = None;
    store.update(&item).unwrap();
    let back = store.get(&item.id).unwrap().unwrap();
    assert_eq!(back.final_text, "Changed.");
    assert_eq!(back.status, ItemStatus::Completed);
    assert!(back.updated_at >= item.created_at);
    assert_eq!(back.error_code, None);
    assert!(
        store
            .get("00000000-0000-4000-8000-000000000000")
            .unwrap()
            .is_none()
    );
    let mut other = DictationItem::new(SourceKind::Dictation);
    other.id = item.id.clone();
    assert!(store.insert(&other).is_err(), "duplicate ids are refused");
    let mut ghost = full_item();
    ghost.id = "00000000-0000-4000-8000-000000000001".into();
    assert!(matches!(store.update(&ghost), Err(StoreError::NotFound(_))));
    // A re-run keeps its link, and deleting the original clears it.
    let mut rerun = DictationItem::new(SourceKind::Rerun);
    rerun.rerun_of_item_id = Some(item.id.clone());
    store.insert(&rerun).unwrap();
    assert_eq!(store.reruns_of(&item.id).unwrap().len(), 1);
    store.delete_row(&item.id).unwrap();
    assert_eq!(
        store.get(&rerun.id).unwrap().unwrap().rerun_of_item_id,
        None
    );
    assert!(matches!(
        store.delete_row(&item.id),
        Err(StoreError::NotFound(_))
    ));
    assert_eq!(
        store.fail_stale_jobs("daemon restarted").unwrap(),
        0,
        "nothing was transcribing"
    );
}

#[test]
fn delete_removes_what_the_artifact_policy_says() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let artifacts = Artifacts::new(dir.path().join("dictations"));
    let cases = [
        (ArtifactPolicy::Keep, false, false),
        (ArtifactPolicy::AudioOnly, false, true),
        (ArtifactPolicy::None, true, true),
    ];
    for (policy, wav_survives, sidecar_survives) in cases {
        let mut item = DictationItem::new(SourceKind::Dictation);
        let wav_path = artifacts.dir(&item.id).join(retention::AUDIO_FILE);
        wav(&wav_path, &[1, 2, 3]);
        let sidecar = artifacts.dir(&item.id).join(retention::METADATA_FILE);
        std::fs::write(&sidecar, "{}").unwrap();
        item.audio_path = Some(wav_path.to_string_lossy().into_owned());
        store.insert(&item).unwrap();
        let removed = retention::delete_item(&store, &artifacts, policy, &item.id).unwrap();
        assert_eq!(removed.id, item.id);
        assert!(store.get(&item.id).unwrap().is_none(), "{policy:?}: row");
        assert_eq!(wav_path.exists(), wav_survives, "{policy:?}: audio");
        assert_eq!(sidecar.exists(), sidecar_survives, "{policy:?}: sidecar");
    }
    assert!(matches!(
        retention::delete_item(&store, &artifacts, ArtifactPolicy::Keep, "nope"),
        Err(StoreError::NotFound(_))
    ));
}

#[test]
fn a_retained_take_moves_into_the_item_directory_with_its_sidecar() {
    let dir = tempfile::tempdir().unwrap();
    let artifacts = Artifacts::new(dir.path().join("data/dictations"));
    let session = dir.path().join("state/sessions/job_dict_1");
    wav(&session.join("microphone.wav"), &[1, 2]);
    wav(&session.join("microphone-2.wav"), &[3]);
    std::fs::write(
        session.join("takes.json"),
        r#"{"takes":[{"index":1,"file":"microphone.wav","start_offset_ms":0,"gap_before":false,"samples":2},{"index":2,"file":"microphone-2.wav","start_offset_ms":900,"gap_before":true,"samples":1}]}"#,
    )
    .unwrap();
    let retention = Retention::default();
    let mut item = DictationItem::new(SourceKind::Dictation);
    let kept = artifacts
        .retain_take(&item.id, &session, &retention)
        .unwrap()
        .expect("audio kept");
    assert_eq!(kept, artifacts.dir(&item.id).join("microphone.wav"));
    let reader = hound::WavReader::open(&kept).unwrap();
    assert_eq!(reader.spec().sample_rate, 16_000);
    assert_eq!(reader.len(), 3, "both takes concatenated");
    assert!(!session.join("microphone.wav").exists());
    item.audio_path = Some(kept.to_string_lossy().into_owned());
    artifacts.write_metadata(&item, &retention).unwrap();
    assert!(artifacts.dir(&item.id).join("metadata.json").is_file());
    // Nothing is kept when the policy says so or audio is off.
    let off = Retention {
        keep_audio: false,
        ..Retention::default()
    };
    wav(&session.join("microphone.wav"), &[1]);
    assert_eq!(artifacts.retain_take("x", &session, &off).unwrap(), None);
    let none = Retention {
        artifacts: ArtifactPolicy::None,
        ..Retention::default()
    };
    assert_eq!(artifacts.retain_take("y", &session, &none).unwrap(), None);
    assert_eq!(
        artifacts
            .retain_take("z", &dir.path().join("empty"), &retention)
            .unwrap(),
        None
    );
}

#[test]
fn the_sweep_drops_old_audio_and_rows_beyond_the_maximum_but_skips_busy_items() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::in_memory().unwrap();
    let artifacts = Artifacts::new(dir.path().join("dictations"));
    let days = [
        "2026-01-01T10:00:00Z",
        "2026-01-20T10:00:00Z",
        "2026-02-10T10:00:00Z",
        "2026-02-13T10:00:00Z",
    ];
    let mut ids = Vec::new();
    for created in days {
        let mut item = DictationItem::new(SourceKind::Dictation);
        item.created_at = created.into();
        let path = artifacts.dir(&item.id).join(retention::AUDIO_FILE);
        wav(&path, &[1]);
        item.audio_path = Some(path.to_string_lossy().into_owned());
        store.insert(&item).unwrap();
        ids.push(item.id);
    }
    let now = unix_from_iso("2026-02-14T00:00:00Z");
    let retention = Retention {
        keep_audio: true,
        audio_retention_days: 30,
        max_items: 3,
        artifacts: ArtifactPolicy::Keep,
    };
    let busy: HashSet<String> = [ids[0].clone()].into_iter().collect();
    let report = retention::sweep(&store, &artifacts, &retention, now, &busy).unwrap();
    // ids[1] (25 days old) keeps audio; ids[0] is the oldest but busy, so
    // neither its audio nor its row goes; nothing else is beyond max 3.
    assert_eq!(report.audio_removed, 0);
    assert_eq!(report.rows_removed, 0);
    let free: HashSet<String> = HashSet::new();
    let report = retention::sweep(&store, &artifacts, &retention, now, &free).unwrap();
    assert_eq!(report.rows_removed, 1, "the oldest row is beyond max_items");
    assert!(store.get(&ids[0]).unwrap().is_none());
    assert!(!artifacts.dir(&ids[0]).exists());
    let later = unix_from_iso("2026-03-01T00:00:00Z");
    let report = retention::sweep(&store, &artifacts, &retention, later, &free).unwrap();
    assert_eq!(
        report.audio_removed, 1,
        "the January item is now past 30 days"
    );
    assert_eq!(store.get(&ids[1]).unwrap().unwrap().audio_path, None);
    assert!(!artifacts.dir(&ids[1]).join(retention::AUDIO_FILE).exists());
    assert!(store.get(&ids[2]).unwrap().unwrap().audio_path.is_some());
    assert!(store.get(&ids[3]).unwrap().unwrap().audio_path.is_some());
    let unlimited = Retention {
        audio_retention_days: 0,
        max_items: 0,
        ..retention
    };
    let report = retention::sweep(&store, &artifacts, &unlimited, later, &free).unwrap();
    assert_eq!(report, retention::SweepReport::default());
}

#[test]
fn readers_keep_reading_while_another_connection_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dettivo.db");
    Store::open(&path).unwrap();
    let writer_path = path.clone();
    let writer = std::thread::spawn(move || {
        let store = Store::open(&writer_path).unwrap();
        for i in 0..300 {
            let mut item = DictationItem::new(SourceKind::Dictation);
            item.final_text = format!("row {i}");
            store.insert(&item).unwrap();
        }
    });
    let reader = Store::open(&path).unwrap();
    let mut seen = 0;
    let mut reads = 0;
    while !writer.is_finished() {
        let page = reader
            .list(&ListFilter {
                limit: 50,
                ..ListFilter::default()
            })
            .unwrap();
        seen = seen.max(page.items.len());
        reads += 1;
    }
    writer.join().unwrap();
    assert!(reads > 0);
    assert!(seen <= 50);
    assert_eq!(reader.count().unwrap(), 300);
}

#[test]
fn a_locked_database_is_reported_busy_after_the_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dettivo.db");
    let store = Store::open_with(&path, migrate::MIGRATIONS, Duration::from_millis(150)).unwrap();
    let holder = rusqlite::Connection::open(&path).unwrap();
    holder.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let started = std::time::Instant::now();
    let err = store.insert(&full_item()).unwrap_err();
    assert_eq!(err, StoreError::Busy);
    assert!(
        started.elapsed() >= Duration::from_millis(100),
        "the timeout waited"
    );
    holder.execute_batch("COMMIT").unwrap();
    store.insert(&full_item()).unwrap();
}

#[test]
fn listing_pages_with_a_cursor_and_filters_by_app_and_date() {
    let store = Store::in_memory().unwrap();
    assert_eq!(dettivo_storage::seed::apply(&store).unwrap(), 16);
    assert_eq!(
        dettivo_storage::seed::apply(&store).unwrap(),
        0,
        "idempotent"
    );
    let mut filter = ListFilter {
        limit: 5,
        ..ListFilter::default()
    };
    let first = store.list(&filter).unwrap();
    assert_eq!(first.items.len(), 5);
    assert_eq!(first.items[0].id, dettivo_storage::seed::CONTRACT_ITEM_ID);
    filter.cursor = first.next_cursor.clone();
    let second = store.list(&filter).unwrap();
    assert_eq!(second.items.len(), 5);
    filter.cursor = second.next_cursor.clone();
    let third = store.list(&filter).unwrap();
    assert_eq!(third.items.len(), 2);
    assert_eq!(third.next_cursor, None);
    let mut all: Vec<&str> = first
        .items
        .iter()
        .chain(&second.items)
        .chain(&third.items)
        .map(|i| i.id.as_str())
        .collect();
    all.dedup();
    assert_eq!(all.len(), 12, "pages never overlap");
    let editor = store
        .list(&ListFilter {
            limit: 20,
            app_id: Some(dettivo_storage::seed::APP_EDITOR.0.into()),
            ..ListFilter::default()
        })
        .unwrap();
    assert_eq!(editor.items.len(), 7);
    let day = store
        .list(&ListFilter {
            limit: 20,
            since: Some("2026-02-12".into()),
            until: Some("2026-02-13".into()),
            ..ListFilter::default()
        })
        .unwrap();
    assert_eq!(day.items.len(), 4);
    assert!(
        store
            .list(&ListFilter {
                limit: 5,
                cursor: Some("bogus".into()),
                ..ListFilter::default()
            })
            .is_err()
    );
    assert_eq!(store.all(&ListFilter::default()).unwrap().len(), 12);
}
