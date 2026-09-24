//! Nonblocking Unix connect keeps a saturated accept queue inside the request deadline.
#![allow(unsafe_code)]
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

pub(super) fn connect(path: &Path, deadline: Instant) -> io::Result<UnixStream> {
    let bytes = path.as_os_str().as_bytes();
    // SAFETY: all-zero sockaddr_un is a valid initial value; the family and path are filled below.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    if bytes.is_empty() || bytes.len() >= address.sun_path.len() || bytes.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid Unix socket path",
        ));
    }
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (to, from) in address.sun_path.iter_mut().zip(bytes) {
        *to = *from as libc::c_char;
    }
    // SAFETY: socket has no pointer arguments and returns a fresh owned descriptor.
    let fd = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            0,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fd is the fresh valid socket above, transferred exactly once into its owner.
    let stream = unsafe { UnixStream::from_raw_fd(fd) };
    loop {
        super::remaining(deadline)?;
        // SAFETY: address is initialized and its full size is provided for the duration of connect.
        let result = unsafe {
            libc::connect(
                stream.as_raw_fd(),
                (&address as *const libc::sockaddr_un).cast(),
                std::mem::size_of_val(&address) as libc::socklen_t,
            )
        };
        if result == 0 {
            stream.set_nonblocking(false)?;
            return Ok(stream);
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EAGAIN) | Some(libc::EINTR) => {
                // AF_UNIX reports a full accept queue as EAGAIN, without a pending connection.
                std::thread::sleep(super::remaining(deadline)?.min(Duration::from_millis(5)));
            }
            _ => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    #[test]
    fn a_full_accept_queue_cannot_hold_a_request_past_its_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ipc");
        let listener = UnixListener::bind(&path).unwrap();
        // SAFETY: a live listener descriptor, reducing its backlog for a deterministic saturation test.
        assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 0) }, 0);
        let _queued = UnixStream::connect(&path).unwrap();
        let start = Instant::now();
        let error = connect(&path, start + Duration::from_millis(30)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(start.elapsed() < Duration::from_secs(1));
    }
}
