//! Bounded transfer loops shared by client adapters, with one cancellation boundary.
use base64::Engine as _;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};
use std::path::Path;

/// The original remote failure or a local file/format failure.
#[derive(Debug)]
pub enum Error<E> {
    /// The adapter's unmodified error.
    Remote(E),
    /// Reading, decoding, writing or publishing a file failed.
    Local(io::Error),
}
/// A failed transfer stage and its contractual cancellation reason.
#[derive(Debug)]
pub struct Failure<E> {
    /// Reason sent with transfer.cancel.
    pub reason: &'static str,
    /// Original failure, preserved when cancellation also fails.
    pub error: Error<E>,
}
impl<E> Failure<E> {
    /// A remote stage failure.
    pub fn remote(reason: &'static str, error: E) -> Self {
        Self {
            reason,
            error: Error::Remote(error),
        }
    }
    /// A local stage failure.
    pub fn local(reason: &'static str, error: io::Error) -> Self {
        Self {
            reason,
            error: Error::Local(error),
        }
    }
}

/// Runs all work after begin, cancelling once on any failure without replacing its cause.
pub fn guarded<E, T>(
    call: impl Fn(&str, Value) -> Result<Value, E>,
    transfer: &str,
    action: impl FnOnce() -> Result<T, Failure<E>>,
) -> Result<T, Error<E>> {
    action().map_err(|failure| {
        let _ = call(
            "transfer.cancel",
            json!({"transfer_id":transfer,"reason":failure.reason}),
        );
        failure.error
    })
}

/// Uploads from a reader, incrementally hashing chunks, then commits.
pub fn upload<E>(
    call: impl Fn(&str, Value) -> Result<Value, E>,
    transfer: &str,
    reader: &mut impl Read,
    chunk_bytes: usize,
) -> Result<(), Failure<E>> {
    if chunk_bytes == 0 {
        return Err(Failure::local(
            "stream_chunk_failed",
            io::Error::new(io::ErrorKind::InvalidData, "upload chunk size is zero"),
        ));
    }
    let mut buffer = vec![0; chunk_bytes.min(1024 * 1024)];
    let mut hash = Sha256::new();
    let mut total = 0u64;
    loop {
        let n = match reader.read(&mut buffer) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            r => r.map_err(|e| Failure::local("stream_chunk_failed", e))?,
        };
        if n == 0 {
            break;
        }
        total += 1;
        hash.update(&buffer[..n]);
        call(
            "transfer.chunk",
            json!({"transfer_id":transfer,"seq":total,
            "data_b64":base64::engine::general_purpose::STANDARD.encode(&buffer[..n])}),
        )
        .map_err(|e| Failure::remote("stream_chunk_failed", e))?;
    }
    call(
        "transfer.commit",
        json!({"transfer_id":transfer,"total_chunks":total,
        "sha256":format!("{:x}", hash.finalize())}),
    )
    .map_err(|e| Failure::remote("stream_commit_failed", e))?;
    Ok(())
}

/// Pulls directly into a writer, incrementally hashes and checks the final acknowledgment.
pub fn download<E>(
    call: impl Fn(&str, Value) -> Result<Value, E>,
    transfer: &str,
    writer: &mut impl Write,
) -> Result<u64, Failure<E>> {
    let mut seq = 1u64;
    let mut bytes = 0u64;
    let mut hash = Sha256::new();
    loop {
        let pull = call("transfer.pull", json!({"transfer_id":transfer,"seq":seq}))
            .map_err(|e| Failure::remote("pull_failed", e))?;
        let malformed = || {
            Failure::local(
                "pull_failed",
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "transfer.pull response malformed",
                ),
            )
        };
        let encoded = pull["data_b64"].as_str().ok_or_else(malformed)?;
        if encoded.len() > crate::transport::MAX_RESPONSE_BYTES {
            return Err(malformed());
        }
        let data = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| malformed())?;
        let eof = pull["eof"].as_bool().ok_or_else(malformed)?;
        if data.is_empty() && !eof {
            return Err(malformed());
        }
        writer
            .write_all(&data)
            .map_err(|e| Failure::local("write_failed", e))?;
        hash.update(&data);
        bytes += data.len() as u64;
        if eof {
            break;
        }
        seq += 1;
    }
    writer
        .flush()
        .map_err(|e| Failure::local("write_failed", e))?;
    call(
        "transfer.commit",
        json!({"transfer_id":transfer,"total_chunks":seq,
        "sha256":format!("{:x}", hash.finalize())}),
    )
    .map_err(|e| Failure::remote("stream_commit_failed", e))?;
    Ok(bytes)
}

/// Downloads beside the destination and atomically publishes only after successful completion.
pub fn download_file<E>(
    call: impl Fn(&str, Value) -> Result<Value, E>,
    transfer: &str,
    path: &Path,
) -> Result<u64, Failure<E>> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let local = |e| Failure::local("write_failed", e);
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(local)?;
    let bytes = download(call, transfer, &mut temp)?;
    temp.as_file().sync_all().map_err(local)?;
    temp.persist(path).map_err(|e| local(e.error))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    #[test]
    fn streaming_failures_cancel_once_and_preserve_the_original_error() {
        for failing in [
            "transfer.chunk",
            "transfer.commit",
            "transcripts.import",
            "writer",
            "reader",
        ] {
            let calls = RefCell::new(Vec::new());
            let call = |method: &str, _: Value| -> Result<Value, &'static str> {
                calls.borrow_mut().push(method.to_string());
                if method == failing || method == "transfer.cancel" {
                    return Err("original remote error");
                }
                Ok(json!({"data_b64":"YQ==", "eof":true}))
            };
            let result = guarded(call, "x", || {
                if failing == "writer" {
                    download(call, "x", &mut BrokenIo).map(|_| ())
                } else {
                    if failing == "reader" {
                        upload(call, "x", &mut BrokenIo, 4)?;
                    } else {
                        upload(call, "x", &mut &b"abcdefgh"[..], 4)?;
                    }
                    call("transcripts.import", json!({}))
                        .map_err(|e| Failure::remote("import_failed", e))?;
                    Ok(())
                }
            });
            assert!(result.is_err());
            assert_eq!(
                calls
                    .borrow()
                    .iter()
                    .filter(|m| *m == "transfer.cancel")
                    .count(),
                1
            );
            assert_eq!(calls.borrow().last().unwrap(), "transfer.cancel");
            match result.unwrap_err() {
                Error::Remote(e) => assert_eq!(e, "original remote error"),
                Error::Local(e) => assert_eq!(e.to_string(), "disk failure"),
            }
        }
    }
    struct BrokenIo;
    impl Read for BrokenIo {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("disk failure"))
        }
    }
    impl Write for BrokenIo {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("disk failure"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn a_large_reader_and_writer_keep_only_one_chunk_and_hash_incrementally() {
        let size = 32 * 1024 * 1024;
        let seen = RefCell::new((0usize, Sha256::new()));
        let call = |method: &str, params: Value| -> Result<Value, ()> {
            if method == "transfer.chunk" {
                let data = base64::engine::general_purpose::STANDARD
                    .decode(params["data_b64"].as_str().unwrap())
                    .unwrap();
                assert!(data.len() <= 64 * 1024);
                seen.borrow_mut().0 += data.len();
                seen.borrow_mut().1.update(data);
            } else if method == "transfer.commit" {
                assert_eq!(seen.borrow().0, size);
                assert_eq!(
                    params["sha256"],
                    format!("{:x}", seen.borrow().1.clone().finalize())
                );
            }
            Ok(json!({}))
        };
        upload(call, "x", &mut io::repeat(42).take(size as u64), 64 * 1024).unwrap();
        let seq = RefCell::new(0);
        let call = |method: &str, _: Value| -> Result<Value, ()> {
            if method == "transfer.pull" {
                *seq.borrow_mut() += 1;
                return Ok(
                    json!({"data_b64":base64::engine::general_purpose::STANDARD.encode([42;64*1024]),"eof":*seq.borrow() == size/(64*1024)}),
                );
            }
            Ok(json!({}))
        };
        assert_eq!(download(call, "x", &mut io::sink()).unwrap(), size as u64);
    }
}
