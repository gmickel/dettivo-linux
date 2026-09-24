//! The model store's tests: readiness from the disk, model sets, the
//! verification identities and quarantine.

use super::*;

fn store() -> (tempfile::TempDir, ModelStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = ModelStore::new(dir.path(), Catalogue::builtin());
    (dir, store)
}

#[test]
fn readiness_follows_the_disk() {
    let (_dir, store) = store();
    let entry = store.catalogue().find("whisper", "tiny").unwrap().clone();
    assert_eq!(store.readiness(&entry), Readiness::Missing);
    std::fs::create_dir_all(store.dir(&entry)).unwrap();
    std::fs::write(store.part_path(&entry), b"abc").unwrap();
    assert_eq!(
        store.readiness(&entry),
        Readiness::Partial { bytes_done: 3 }
    );
    std::fs::remove_file(store.part_path(&entry)).unwrap();
    std::fs::write(store.path(&entry), b"not a model").unwrap();
    assert_eq!(store.readiness(&entry), Readiness::Unverified);
    assert_eq!(store.verify(&entry).unwrap(), Readiness::Quarantined);
    assert!(!store.path(&entry).exists());
    assert_eq!(store.readiness(&entry), Readiness::Quarantined);
    assert_eq!(store.quarantined_count(), 1);
}

#[test]
fn a_model_set_is_ready_when_every_file_landed_and_verified() {
    let (_dir, store) = store();
    let mut entry = store
        .catalogue()
        .find("diarize", "diarization")
        .unwrap()
        .clone();
    let files = entry.files();
    assert_eq!(store.readiness(&entry), Readiness::Missing);
    assert_eq!(store.load_path(&entry), store.dir(&entry));
    std::fs::create_dir_all(store.dir(&entry)).unwrap();
    std::fs::write(store.file_path(&entry, &files[0]), b"seg").unwrap();
    assert_eq!(
        store.readiness(&entry),
        Readiness::Partial {
            bytes_done: files[0].size_bytes
        }
    );
    std::fs::write(store.file_part_path(&entry, &files[1]), b"ab").unwrap();
    assert_eq!(
        store.readiness(&entry),
        Readiness::Partial {
            bytes_done: files[0].size_bytes + 2
        }
    );
    std::fs::remove_file(store.file_part_path(&entry, &files[1])).unwrap();
    std::fs::write(store.file_path(&entry, &files[1]), b"emb").unwrap();
    assert_eq!(store.readiness(&entry), Readiness::Unverified);
    // The checksums the fake files must have: the member's for the
    // unpacked segmentation model, the download's for the embedding.
    entry.unpack.as_mut().unwrap().sha256 = format!("{:x}", Sha256::digest(b"seg"));
    entry.files[0].sha256 = format!("{:x}", Sha256::digest(b"emb"));
    assert_eq!(store.verify(&entry).unwrap(), Readiness::Ready);
    assert_eq!(store.readiness(&entry), Readiness::Ready);
    // The manifest names every file under its checksum; a catalogue
    // that changes the embedding checksum alone is a new set, and one
    // that hashes the old embedding is quarantined.
    let manifest = store.manifest(&entry).unwrap();
    assert_eq!(
        manifest
            .files
            .iter()
            .map(|f| f.file_name.as_str())
            .collect::<Vec<_>>(),
        ["segmentation.onnx", "embedding.onnx"]
    );
    let mut updated = entry.clone();
    updated.files[0].sha256 = format!("{:x}", Sha256::digest(b"emb2"));
    assert_eq!(store.readiness(&updated), Readiness::Unverified);
    assert_eq!(store.readiness(&entry), Readiness::Ready);
    // A file replaced in place (the same bytes, a later time) is
    // hashed again before it counts as verified.
    std::thread::sleep(std::time::Duration::from_millis(5));
    std::fs::write(store.file_path(&entry, &files[1]), b"emb").unwrap();
    assert_eq!(store.readiness(&entry), Readiness::Unverified);
    assert_eq!(store.verify(&entry).unwrap(), Readiness::Ready);
    assert_eq!(store.readiness(&entry), Readiness::Ready);
    // A manifest from before the identities were recorded is not
    // verified until the files hash once more.
    let legacy = serde_json::json!({
        "provider": "diarize", "id": "diarization", "sha256": entry.sha256,
        "size_bytes": entry.size_bytes, "downloaded_at": 1, "catalogue_version": 1,
        "verified": true
    });
    std::fs::write(store.manifest_path(&entry), legacy.to_string()).unwrap();
    assert_eq!(store.readiness(&entry), Readiness::Unverified);
    assert_eq!(store.verify(&entry).unwrap(), Readiness::Ready);
    // One corrupt file quarantines the whole set.
    std::fs::write(store.file_path(&entry, &files[1]), b"bad").unwrap();
    assert_eq!(store.verify(&entry).unwrap(), Readiness::Quarantined);
    assert!(!store.file_path(&entry, &files[0]).exists());
    assert_eq!(store.readiness(&entry), Readiness::Quarantined);
    std::fs::write(store.file_path(&entry, &files[0]), b"seg").unwrap();
    std::fs::write(store.file_path(&entry, &files[1]), b"emb").unwrap();
    assert!(store.delete(&entry).unwrap());
    assert!(!store.file_path(&entry, &files[1]).exists());
}

#[test]
fn a_matching_file_is_verified_and_remembered() {
    let (_dir, store) = store();
    let mut entry = store.catalogue().find("whisper", "tiny").unwrap().clone();
    let bytes = b"hello model";
    entry.sha256 = format!("{:x}", Sha256::digest(bytes));
    std::fs::create_dir_all(store.dir(&entry)).unwrap();
    std::fs::write(store.path(&entry), bytes).unwrap();
    assert_eq!(store.verify(&entry).unwrap(), Readiness::Ready);
    assert!(store.manifest(&entry).unwrap().verified);
    assert_eq!(store.readiness(&entry), Readiness::Ready);
    assert!(store.delete(&entry).unwrap());
    assert_eq!(store.readiness(&entry), Readiness::Missing);
    assert!(!store.delete(&entry).unwrap());
}
