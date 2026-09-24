//! The transfer service's unit tests: ordered chunks, the hash on commit,
//! the upload limit, downloads pulled to eof, expiry.

use super::*;
use std::os::unix::fs::{PermissionsExt, symlink};

fn caps() -> TransferCaps {
    TransferCaps {
        chunk_max_bytes: 4,
        max_inflight: 4,
        timeout_seconds: 120,
    }
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[test]
fn uploads_take_ordered_chunks_and_commit_against_the_hash() {
    let dir = tempfile::tempdir().unwrap();
    let t = Transfers::new(dir.path(), caps());
    let begun = t.begin(Direction::Upload, "audio/wav", 8).unwrap();
    assert_eq!(begun.chunk_max_bytes, 4);
    assert!(begun.expires_at.ends_with('Z'));
    let id = begun.transfer_id.clone();
    assert_eq!(id, "xfer_in_1");
    let far = t.chunk(&id, 9, "AAAA").unwrap_err();
    assert_eq!(far.app_code(), AppCode::RateLimitedLocal);
    assert!(far.data.retryable);
    assert_eq!(
        t.chunk(&id, 2, "AAAA").unwrap_err().app_code(),
        AppCode::InvalidParams
    );
    assert_eq!(t.chunk(&id, 1, &b64(b"abcd")).unwrap().next_seq, 2);
    assert_eq!(t.chunk(&id, 2, &b64(b"ef")).unwrap().next_seq, 3);
    assert_eq!(
        t.chunk(&id, 3, &b64(b"toolong")).unwrap_err().app_code(),
        AppCode::InvalidParams
    );
    let wrong = t.commit(&id, 2, "00").unwrap_err();
    assert!(wrong.message.contains("sha256"));
    assert_eq!(
        t.commit(&id, 1, "00").unwrap_err().app_code(),
        AppCode::InvalidParams
    );
    let digest = format!("{:x}", Sha256::digest(b"abcdef"));
    assert!(t.commit(&id, 2, &digest).unwrap().committed);
    let upload = t.upload(&id).unwrap();
    assert_eq!(std::fs::read(&upload.path).unwrap(), b"abcdef");
    assert_eq!(upload.content_type, "audio/wav");
    t.finish_upload(&id);
    assert!(!upload.path.exists());
    assert_eq!(
        t.chunk("xfer_missing", 1, "AAAA").unwrap_err().message,
        "Transfer not found"
    );
    assert_eq!(
        t.cancel(&id, "client_abort").unwrap_err().app_code(),
        AppCode::NotFound
    );
    let err = t.begin(Direction::Upload, "text/csv", 1).unwrap_err();
    assert!(err.message.contains("audio/flac"), "{}", err.message);
    assert!(
        t.begin(Direction::Upload, "audio/wav", MAX_UPLOAD_BYTES + 1)
            .is_err()
    );
    t.set_max_upload(5);
    let small = t
        .begin(Direction::Upload, "audio/wav", 0)
        .unwrap()
        .transfer_id;
    assert_eq!(t.chunk(&small, 1, &b64(b"abcd")).unwrap().next_seq, 2);
    let err = t.chunk(&small, 2, &b64(b"ef")).unwrap_err();
    assert!(
        err.message.contains("transfer.max_upload_bytes"),
        "{}",
        err.message
    );
    let kept = t.take_upload(&small);
    assert!(kept.exists(), "take_upload leaves the file to the caller");
    assert_eq!(t.count(), 0);
    let _ = std::fs::remove_file(kept);
}

#[test]
fn downloads_are_bound_then_pulled_to_eof_and_cancel_drops_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let t = Transfers::new(dir.path(), caps());
    let id = t
        .begin(Direction::Download, "application/json", 0)
        .unwrap()
        .transfer_id;
    assert_eq!(id, "xfer_out_1");
    assert_eq!(t.pull(&id, 1).unwrap_err().app_code(), AppCode::Conflict);
    t.bind_export(&id, b"0123456789").unwrap();
    let first = t.pull(&id, 1).unwrap();
    assert_eq!(first.data_b64, b64(b"0123"));
    assert!(!first.eof);
    let last = t.pull(&id, 3).unwrap();
    assert_eq!(last.data_b64, b64(b"89"));
    assert!(last.eof);
    assert!(t.pull(&id, 4).is_err());
    assert!(t.commit(&id, 3, "").unwrap().committed);
    let empty = t
        .begin(Direction::Download, "text/plain", 0)
        .unwrap()
        .transfer_id;
    t.bind_export(&empty, b"").unwrap();
    let only = t.pull(&empty, 1).unwrap();
    assert!(only.eof && only.data_b64.is_empty());
    assert!(t.cancel(&id, "done").unwrap().cancelled);
    assert!(!dir.path().join("exports").join(&id).exists());
    assert_eq!(t.count(), 1);
    assert_eq!(t.expire(), 0);
}

#[test]
fn transfer_storage_is_private_and_preserves_io_and_expiry() {
    for direction in [Direction::Upload, Direction::Download] {
        for existing in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let name = if direction == Direction::Upload {
                "transfers"
            } else {
                "exports"
            };
            let dir = root.path().join(name);
            if existing {
                std::fs::create_dir(&dir).unwrap();
                std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
            }
            let t = Transfers::new(root.path(), caps());
            let id = t.begin(direction, "audio/wav", 0).unwrap().transfer_id;
            let path = dir.join(&id);
            if direction == Direction::Upload {
                t.chunk(&id, 1, &b64(b"data")).unwrap();
                t.commit(&id, 1, &format!("{:x}", Sha256::digest(b"data")))
                    .unwrap();
                assert_eq!(t.upload(&id).unwrap().path, path);
            } else {
                t.bind_export(&id, b"longer").unwrap();
                t.bind_export(&id, b"data").unwrap();
                assert_eq!(t.pull(&id, 1).unwrap().data_b64, b64(b"data"));
            }
            assert_eq!(
                std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(std::fs::read(&path).unwrap(), b"data");
            t.inner.lock().unwrap().get_mut(&id).unwrap().expires = Instant::now();
            assert_eq!(t.expire(), 1);
            assert!(!path.exists());
        }
    }
}

#[test]
fn transfer_creation_preserves_existing_files_and_refuses_symlink_directories() {
    for (direction, name, prefix) in [
        (Direction::Upload, "transfers", "xfer_in"),
        (Direction::Download, "exports", "xfer_out"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let outside = root.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o755)).unwrap();
        let dir = root.path().join(name);
        symlink(&outside, &dir).unwrap();
        let t = Transfers::new(root.path(), caps());
        assert!(t.begin(direction, "audio/wav", 0).is_err());
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
        assert_eq!(
            std::fs::metadata(&outside).unwrap().permissions().mode() & 0o777,
            0o755
        );
        std::fs::remove_file(&dir).unwrap();
        std::fs::create_dir(&dir).unwrap();
        let stale = dir.join(format!("{prefix}_1"));
        std::fs::write(&stale, b"preserve").unwrap();
        symlink(&stale, dir.join(format!("{prefix}_2"))).unwrap();
        let t = Transfers::new(root.path(), caps());
        let id = t.begin(direction, "audio/wav", 0).unwrap().transfer_id;
        assert_eq!(id, format!("{prefix}_3"));
        if direction == Direction::Download {
            t.bind_export(&id, b"data").unwrap();
        } else {
            t.chunk(&id, 1, &b64(b"data")).unwrap();
        }
        assert_eq!(std::fs::read(stale).unwrap(), b"preserve");
        assert_eq!(std::fs::read(dir.join(id)).unwrap(), b"data");
    }
}
