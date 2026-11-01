//! Reads a clipboard pipe until its end, a deadline or a byte limit, so a
//! source that never closes its end cannot hold the insertion.

use std::io::Read;
use std::os::fd::AsRawFd;
use std::time::Instant;

/// The bytes from `pipe`, or why they were not read whole.
pub(crate) fn bounded(
    mut pipe: impl Read + AsRawFd,
    until: Instant,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err("the source did not answer in time".into());
        }
        let mut fd = libc::pollfd {
            fd: pipe.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ms = i32::try_from(left.as_millis().max(1)).unwrap_or(i32::MAX);
        // SAFETY: `fd` is one valid pollfd that outlives the call.
        #[allow(unsafe_code)]
        let ready = unsafe { libc::poll(&mut fd, 1, ms) };
        if ready < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(format!("poll: {e}"));
        }
        if ready == 0 {
            continue;
        }
        let n = match pipe.read(&mut chunk) {
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(format!("read: {e}")),
        };
        if n == 0 {
            return Ok(out);
        }
        if out.len() + n > limit {
            return Err("over the snapshot budget".into());
        }
        out.extend_from_slice(&chunk[..n]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    #[test]
    fn a_pipe_is_read_to_its_end_within_the_limit_and_the_deadline() {
        let (mut tx, rx) = UnixStream::pair().unwrap();
        tx.write_all(b"image bytes").unwrap();
        drop(tx);
        let soon = Instant::now() + Duration::from_millis(500);
        assert_eq!(bounded(rx, soon, 64).unwrap(), b"image bytes");

        let (mut tx, rx) = UnixStream::pair().unwrap();
        tx.write_all(&[0; 100]).unwrap();
        drop(tx);
        assert_eq!(
            bounded(rx, soon, 64).unwrap_err(),
            "over the snapshot budget"
        );

        let (_open, rx) = UnixStream::pair().unwrap();
        let started = Instant::now();
        let quick = started + Duration::from_millis(100);
        assert_eq!(
            bounded(rx, quick, 64).unwrap_err(),
            "the source did not answer in time"
        );
        assert!(started.elapsed() < Duration::from_millis(400));
    }
}
