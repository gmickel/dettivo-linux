//! A fixture model server for tests (std only): serves files from a
//! directory over HTTP/1.1 with `Range` support, and can cut a transfer
//! short, ignore ranges or serve corrupted bytes on request. Records every
//! request so a test can assert that a resume asked for a range. Used by
//! this crate's download tests and by the daemon's `speech.models.*` tests.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// One request the server saw.
#[derive(Debug, Clone)]
pub struct SeenRequest {
    /// The request path.
    pub path: String,
    /// The `Range` header, when sent.
    pub range: Option<String>,
}

/// The server's switches.
#[derive(Debug, Default)]
pub struct Behaviour {
    /// Close the connection after this many body bytes, once.
    pub truncate_after: Mutex<Option<u64>>,
    /// Flip the first byte of the body.
    pub corrupt: AtomicBool,
    /// Ignore `Range` and answer 200 from the start.
    pub ignore_range: AtomicBool,
}

/// A running fixture server.
pub struct FixtureServer {
    /// `http://127.0.0.1:<port>`.
    pub url: String,
    /// Every request seen so far.
    pub requests: Arc<Mutex<Vec<SeenRequest>>>,
    /// The switches.
    pub behaviour: Arc<Behaviour>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl FixtureServer {
    /// Serves `dir` on a free loopback port.
    pub fn serve(dir: PathBuf) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let behaviour = Arc::new(Behaviour::default());
        let stop = Arc::new(AtomicBool::new(false));
        let (r, b, s) = (requests.clone(), behaviour.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        handle(stream, &dir, &r, &b);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            url: format!("http://127.0.0.1:{port}"),
            requests,
            behaviour,
            stop,
            thread: Some(thread),
        }
    }

    /// The requests seen so far.
    pub fn seen(&self) -> Vec<SeenRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn handle(mut stream: TcpStream, dir: &Path, requests: &Mutex<Vec<SeenRequest>>, b: &Behaviour) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.is_empty() {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    let mut range = None;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).is_err() || h.trim().is_empty() {
            break;
        }
        if let Some(v) = h
            .strip_prefix("Range:")
            .or_else(|| h.strip_prefix("range:"))
        {
            range = Some(v.trim().to_string());
        }
    }
    requests.lock().unwrap().push(SeenRequest {
        path: path.clone(),
        range: range.clone(),
    });
    let file = dir.join(path.trim_start_matches('/'));
    let Ok(mut body) = std::fs::read(&file) else {
        let _ = stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return;
    };
    if b.corrupt.load(Ordering::Relaxed) && !body.is_empty() {
        body[0] ^= 0xff;
    }
    let total = body.len();
    let start = range
        .as_deref()
        .filter(|_| !b.ignore_range.load(Ordering::Relaxed))
        .and_then(|r| r.strip_prefix("bytes="))
        .and_then(|r| r.trim_end_matches('-').parse::<usize>().ok())
        .filter(|s| *s < total);
    let (status, head, slice) = match start {
        Some(s) => (
            "206 Partial Content",
            format!("Content-Range: bytes {s}-{}/{total}\r\n", total - 1),
            &body[s..],
        ),
        None => ("200 OK", String::new(), &body[..]),
    };
    let cut = b.truncate_after.lock().unwrap().take();
    let send = match cut {
        Some(n) => &slice[..(n as usize).min(slice.len())],
        None => slice,
    };
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n{head}Connection: close\r\n\r\n",
        slice.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(send);
    let _ = stream.flush();
    let _ = stream.shutdown(std::net::Shutdown::Both);
    // Drain anything the client still sends so the close is orderly.
    let mut sink = [0u8; 64];
    let _ = reader.read(&mut sink);
}
