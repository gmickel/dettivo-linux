//! R2 against the fixture model server: a download lands in the model
//! layout with a verified manifest and reports progress, a truncated
//! transfer resumes with a `Range` request, a cancelled one leaves a
//! `.part`, and a wrong checksum quarantines the file.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use dettivo_speech::catalogue::{Catalogue, ModelEntry, ModelKind};
use dettivo_speech::download::{self, DownloadState, Progress};
use dettivo_speech::fixture_server::FixtureServer;
use dettivo_speech::models::{ModelStore, Readiness};
use sha2::{Digest, Sha256};

/// A 1 MiB pseudo-random model file served by the fixture server.
fn fixture(server_dir: &std::path::Path) -> (Vec<u8>, String) {
    let mut bytes = Vec::with_capacity(1 << 20);
    let mut x: u32 = 0x1234_5678;
    for _ in 0..(1 << 20) {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        bytes.push((x & 0xff) as u8);
    }
    std::fs::write(server_dir.join("ggml-test.bin"), &bytes).unwrap();
    let sha = format!("{:x}", Sha256::digest(&bytes));
    (bytes, sha)
}

fn entry(url: &str, sha: &str) -> ModelEntry {
    ModelEntry {
        provider: "whisper".into(),
        id: "test".into(),
        display_name: "Test".into(),
        kind: ModelKind::Stt,
        file_name: "ggml-test.bin".into(),
        size_bytes: 1 << 20,
        url: format!("{url}/ggml-test.bin"),
        sha256: sha.into(),
        license: "MIT".into(),
        redistribution: "test".into(),
        languages: vec!["en".into()],
        english_only: true,
        quantization: None,
        roles: Vec::new(),
        default: false,
        available: true,
        recommended_for: None,
        unpack: None,
        files: Vec::new(),
    }
}

struct Rig {
    _server_dir: tempfile::TempDir,
    _models_dir: tempfile::TempDir,
    server: FixtureServer,
    store: ModelStore,
    entry: ModelEntry,
}

fn rig() -> Rig {
    let server_dir = tempfile::tempdir().unwrap();
    let models_dir = tempfile::tempdir().unwrap();
    let (_bytes, sha) = fixture(server_dir.path());
    let server = FixtureServer::serve(server_dir.path().to_path_buf());
    let entry = entry(&server.url, &sha);
    let store = ModelStore::new(models_dir.path(), Catalogue::builtin());
    Rig {
        _server_dir: server_dir,
        _models_dir: models_dir,
        server,
        store,
        entry,
    }
}

fn collect() -> (
    Arc<Mutex<Vec<Progress>>>,
    impl Fn(&Progress) + Send + 'static,
) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s = seen.clone();
    (seen, move |p: &Progress| s.lock().unwrap().push(p.clone()))
}

#[test]
fn a_download_lands_verified_with_progress() {
    let r = rig();
    let (seen, on) = collect();
    let d = download::start(r.store.clone(), r.entry.clone(), on);
    let done = d.join();
    assert_eq!(done.state, DownloadState::Done, "{done:?}");
    assert_eq!(done.bytes_done, 1 << 20);
    assert_eq!(done.bytes_total, 1 << 20);
    assert_eq!(r.store.readiness(&r.entry), Readiness::Ready);
    assert!(r.store.manifest(&r.entry).unwrap().verified);
    assert!(!r.store.part_path(&r.entry).exists());
    assert!(!r.store.part_meta_path(&r.entry).exists());
    let states: Vec<DownloadState> = seen
        .lock()
        .unwrap()
        .iter()
        .map(|p| p.state.clone())
        .collect();
    assert_eq!(states.last(), Some(&DownloadState::Done));
    assert_eq!(r.server.seen().len(), 1);
    assert!(r.server.seen()[0].range.is_none());
}

#[test]
fn a_truncated_transfer_resumes_with_a_range_request() {
    let r = rig();
    *r.server.behaviour.truncate_after.lock().unwrap() = Some(300_000);
    let (_seen, on) = collect();
    let first = download::start(r.store.clone(), r.entry.clone(), on).join();
    assert_eq!(first.state, DownloadState::Failed, "{first:?}");
    assert!(first.error.is_some(), "{first:?}");
    assert_eq!(r.server.seen()[0].path, "/ggml-test.bin");
    let partial = std::fs::metadata(r.store.part_path(&r.entry))
        .unwrap()
        .len();
    assert!(partial > 0 && partial < (1 << 20), "{partial}");
    assert!(matches!(
        r.store.readiness(&r.entry),
        Readiness::Partial { bytes_done } if bytes_done == partial
    ));

    let (_seen, on) = collect();
    let second = download::start(r.store.clone(), r.entry.clone(), on).join();
    assert_eq!(second.state, DownloadState::Done, "{second:?}");
    assert!(second.resumed);
    let seen = r.server.seen();
    assert_eq!(seen.len(), 2);
    assert_eq!(
        seen[1].range.as_deref(),
        Some(format!("bytes={partial}-").as_str())
    );
    assert_eq!(r.store.readiness(&r.entry), Readiness::Ready);
    let on_disk = std::fs::read(r.store.path(&r.entry)).unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(&on_disk)), r.entry.sha256);
}

#[test]
fn a_server_without_ranges_restarts_from_zero() {
    let r = rig();
    *r.server.behaviour.truncate_after.lock().unwrap() = Some(100_000);
    let (_s, on) = collect();
    let _ = download::start(r.store.clone(), r.entry.clone(), on).join();
    r.server
        .behaviour
        .ignore_range
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let (_s, on) = collect();
    let second = download::start(r.store.clone(), r.entry.clone(), on).join();
    assert_eq!(second.state, DownloadState::Done, "{second:?}");
    assert!(!second.resumed);
    assert_eq!(r.store.readiness(&r.entry), Readiness::Ready);
}

#[test]
fn a_cancelled_download_keeps_its_partial_file() {
    let r = rig();
    let (_s, on) = collect();
    let d = download::start(r.store.clone(), r.entry.clone(), on);
    d.cancel();
    let out = d.join();
    // The transfer may finish before the cancel lands on a loopback
    // server; either outcome leaves a consistent store.
    match out.state {
        DownloadState::Cancelled => {
            assert!(r.store.part_path(&r.entry).exists());
            assert!(!r.store.path(&r.entry).exists());
        }
        DownloadState::Done => assert_eq!(r.store.readiness(&r.entry), Readiness::Ready),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_wrong_checksum_is_quarantined() {
    let r = rig();
    r.server
        .behaviour
        .corrupt
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let (_s, on) = collect();
    let out = download::start(r.store.clone(), r.entry.clone(), on).join();
    assert_eq!(out.state, DownloadState::Quarantined, "{out:?}");
    assert_eq!(out.error.as_deref(), Some("checksum mismatch"));
    assert!(!r.store.path(&r.entry).exists());
    assert!(!r.store.part_path(&r.entry).exists());
    assert_eq!(r.store.readiness(&r.entry), Readiness::Quarantined);
    let quarantine = r.store.quarantine_dir(&r.entry);
    let copies: Vec<_> = std::fs::read_dir(&quarantine).unwrap().collect();
    assert_eq!(copies.len(), 1);
    // A retry downloads afresh and lands.
    r.server
        .behaviour
        .corrupt
        .store(false, std::sync::atomic::Ordering::Relaxed);
    let (_s, on) = collect();
    let again = download::start(r.store.clone(), r.entry.clone(), on).join();
    assert_eq!(again.state, DownloadState::Done, "{again:?}");
    assert_eq!(r.store.readiness(&r.entry), Readiness::Ready);
    std::thread::sleep(Duration::from_millis(10));
}

#[test]
fn a_missing_file_fails_naming_the_status() {
    let r = rig();
    let mut entry = r.entry.clone();
    entry.url = format!("{}/nope.bin", r.server.url);
    let (_s, on) = collect();
    let out = download::start(r.store.clone(), entry.clone(), on).join();
    assert_eq!(out.state, DownloadState::Failed);
    assert!(
        out.error.as_deref().unwrap_or("").contains("404"),
        "{out:?}"
    );
    assert_eq!(r.store.readiness(&entry), Readiness::Missing);
}

/// engines/F1: the final progress callback runs after the progress lock is
/// released. The callback here takes a lock the main thread holds while it
/// asks for the progress, which is the order the model service uses; with
/// the callback under the progress lock the two wait for each other.
#[test]
fn the_final_callback_runs_outside_the_progress_lock() {
    let r = rig();
    let status_lock = Arc::new(Mutex::new(()));
    let meet = Arc::new(std::sync::Barrier::new(2));
    let (lock_in_callback, meet_in_callback) = (status_lock.clone(), meet.clone());
    let d = download::start(r.store.clone(), r.entry.clone(), move |p: &Progress| {
        if p.state != DownloadState::Running {
            meet_in_callback.wait();
            let _held = lock_in_callback.lock().unwrap();
        }
    });
    let held = status_lock.lock().unwrap();
    meet.wait();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        tx.send(d.progress()).unwrap();
        d.join();
    });
    let progress = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("progress answered while the final callback waits for the status lock");
    assert_eq!(progress.state, DownloadState::Done, "{progress:?}");
    drop(held);
}
