//! One engine process: spawned with the protocol on its stdin/stdout,
//! stderr captured (last lines kept, redacted of anything that is not a
//! plain log line), requests serialised, events routed to the caller.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dettivo_engine_proto::{Frame, FrameError, Kind, read_frame, write_frame};
use serde_json::{Value, json};

use crate::EngineError;

/// How many stderr lines are kept for a crash report.
pub const STDERR_TAIL: usize = 20;

/// One decoded frame with its attachments, or the reader's failure.
type Framed = Result<(Frame, Vec<Vec<u8>>), String>;

/// A live engine process.
pub struct EngineProcess {
    child: Child,
    /// The engine's stdin; `None` once terminated, which closes it.
    writer: Option<BufWriter<std::process::ChildStdin>>,
    frames: Receiver<Framed>,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    next_id: u64,
    /// Path the process was started from.
    pub binary: PathBuf,
    /// When the engine last finished a request.
    pub last_used: Instant,
}

/// The reserved drop-in for a later `dettivo-engines-cuda` package: an
/// engine here wins over the default install (ADR 0034).
pub const CUDA_DROP_IN_DIR: &str = "/usr/lib/dettivo/engines-cuda";
/// Where the package installs the engine binaries.
pub const SYSTEM_ENGINE_DIR: &str = "/usr/lib/dettivo/engines";

/// The directories searched for an engine, in order: `directory` when
/// set, the CUDA drop-in, the package's engine directory, the directory of
/// the running daemon, then every `PATH` entry.
pub fn search_dirs(directory: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = directory {
        dirs.push(dir.to_path_buf());
    }
    dirs.push(PathBuf::from(CUDA_DROP_IN_DIR));
    dirs.push(PathBuf::from(SYSTEM_ENGINE_DIR));
    if let Some(exe) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        dirs.push(exe);
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs
}

/// Finds `binary` through the directories of [`search_dirs`]; the error
/// names every directory searched.
pub fn find_binary(binary: &str, directory: Option<&Path>) -> Result<PathBuf, EngineError> {
    let mut searched = Vec::new();
    for dir in search_dirs(directory) {
        let candidate = dir.join(binary);
        if candidate.is_file() {
            return Ok(candidate);
        }
        searched.push(dir);
    }
    Err(EngineError::NotFound {
        binary: binary.to_string(),
        searched,
    })
}

impl EngineProcess {
    /// Spawns `binary` with the protocol on its stdio.
    pub fn spawn(binary: &Path, force_cpu: bool) -> Result<Self, EngineError> {
        let mut cmd = Command::new(binary);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if force_cpu {
            cmd.env("DETTIVO_FORCE_CPU", "1");
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| EngineError::Transport(format!("spawn {}: {e}", binary.display())))?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let tail: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
        let sink = tail.clone();
        std::thread::Builder::new()
            .name("dettivo-engine-stderr".into())
            .spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    tracing::debug!(target: "engine", "{}", redact(&line));
                    let mut t = sink.lock().unwrap_or_else(|p| p.into_inner());
                    if t.len() >= STDERR_TAIL {
                        t.pop_front();
                    }
                    t.push_back(redact(&line));
                }
            })
            .map_err(|e| EngineError::Transport(e.to_string()))?;
        let (tx, rx): (Sender<Framed>, Receiver<Framed>) = mpsc::channel();
        std::thread::Builder::new()
            .name("dettivo-engine-stdout".into())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    match read_frame(&mut reader) {
                        Ok(frame) => {
                            if tx.send(Ok(frame)).is_err() {
                                break;
                            }
                        }
                        Err(FrameError::Eof) => break,
                        Err(e) => {
                            let _ = tx.send(Err(e.to_string()));
                            break;
                        }
                    }
                }
            })
            .map_err(|e| EngineError::Transport(e.to_string()))?;
        Ok(Self {
            child,
            writer: Some(BufWriter::new(stdin)),
            frames: rx,
            stderr_tail: tail,
            next_id: 1,
            binary: binary.to_path_buf(),
            last_used: Instant::now(),
        })
    }

    /// The id of the last request sent (what a `cancel` names).
    pub fn last_request_id(&self) -> u64 {
        self.next_id.saturating_sub(1)
    }

    /// The last stderr lines, redacted.
    pub fn stderr_tail(&self) -> String {
        self.stderr_tail
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The process id.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// The process's resident set in bytes (`VmRSS` from `/proc`), when
    /// it can be read.
    pub fn memory_bytes(&self) -> Option<u64> {
        resident_bytes(self.pid())
    }

    /// True while the process runs.
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Sends one request and waits for its response, routing events to
    /// `on_event` meanwhile. Returns the response payload or the error.
    pub fn call(
        &mut self,
        name: &str,
        payload: Value,
        attachments: &[&[u8]],
        timeout: Duration,
        on_event: impl FnMut(&Frame),
    ) -> Result<Value, EngineError> {
        let never = AtomicBool::new(false);
        self.call_cancellable(name, payload, attachments, timeout, &never, on_event)
    }

    /// `call` with a flag the caller may raise from another thread: when
    /// it goes up mid-request a `cancel` for the request is sent (once),
    /// and the engine's answer, `cancelled` for one that honours it, is
    /// returned as the outcome.
    pub fn call_cancellable(
        &mut self,
        name: &str,
        payload: Value,
        attachments: &[&[u8]],
        timeout: Duration,
        cancel: &AtomicBool,
        mut on_event: impl FnMut(&Frame),
    ) -> Result<Value, EngineError> {
        let id = self.next_id;
        self.next_id += 1;
        let mut frame = Frame::request(id, name, payload);
        for a in attachments {
            frame = frame.with_pcm(a.len() as u64);
        }
        let Some(writer) = self.writer.as_mut() else {
            return Err(EngineError::Crashed("engine stdin is closed".into()));
        };
        // One deadline for the whole exchange: a write that stalls on a
        // full pipe spends the same budget as the wait for the answer.
        let deadline = Instant::now() + timeout;
        write_frame(writer, &frame, attachments).map_err(|e| self.crash_or(e.to_string()))?;
        let mut cancel_sent = false;
        loop {
            if !cancel_sent && cancel.load(Ordering::SeqCst) {
                cancel_sent = true;
                let cancel_id = self.next_id;
                self.next_id += 1;
                if let Some(writer) = self.writer.as_mut() {
                    let frame = Frame::request(cancel_id, "cancel", json!({ "request_id": id }));
                    if let Err(e) = write_frame(writer, &frame, &[]) {
                        return Err(self.crash_or(e.to_string()));
                    }
                }
            }
            let remaining = deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(100));
            match self.frames.recv_timeout(remaining) {
                Ok(Ok((reply, _))) => match reply.kind {
                    Kind::Event => on_event(&reply),
                    Kind::Response if reply.id == id => {
                        self.last_used = Instant::now();
                        if reply.name == "error" {
                            let code = reply.payload["code"]
                                .as_str()
                                .unwrap_or("internal")
                                .to_string();
                            let message =
                                reply.payload["message"].as_str().unwrap_or("").to_string();
                            return Err(if code == "model_missing" {
                                EngineError::ModelMissing(message)
                            } else if code == "cancelled" {
                                EngineError::Cancelled
                            } else {
                                EngineError::Engine { code, message }
                            });
                        }
                        return Ok(reply.payload);
                    }
                    Kind::Response => tracing::warn!(
                        expected = id,
                        got = reply.id,
                        "response for another request"
                    ),
                    Kind::Request => tracing::warn!("engine sent a request; ignored"),
                },
                Ok(Err(e)) => return Err(self.crash_or(e)),
                Err(RecvTimeoutError::Timeout) => {
                    if Instant::now() < deadline {
                        continue;
                    }
                    return Err(EngineError::Transport(format!(
                        "{name}: no response within {timeout:?}"
                    )));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(self.crash_or("engine stream closed".into()));
                }
            }
        }
    }

    /// A closed stream is a crash when the process is gone; the exit can
    /// trail the EOF by a moment, so the process gets half a second to
    /// show it before the failure counts as a transport error.
    fn crash_or(&mut self, detail: String) -> EngineError {
        let deadline = Instant::now() + Duration::from_millis(500);
        while self.alive() {
            if Instant::now() >= deadline {
                return EngineError::Transport(detail);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        EngineError::Crashed(format!("{detail}; last stderr:\n{}", self.stderr_tail()))
    }

    /// Terminates the process: closes its stdin (an engine exits on end of
    /// stream), waits briefly, then kills it.
    pub fn terminate(&mut self) {
        if let Some(mut writer) = self.writer.take() {
            let _ = writer.flush();
            drop(writer);
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if !self.alive() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for EngineProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The resident set of `pid` in bytes from `/proc/<pid>/status`.
pub fn resident_bytes(pid: u32) -> Option<u64> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

/// Keeps a log line's shape while dropping anything that looks like
/// content. Engine logs are timings, states and identifiers; a transcript
/// or a prompt never belongs in one, and this makes sure a crash report
/// cannot carry it even if an engine misbehaves. Everything after the
/// first colon is replaced when it runs longer than a short token run or
/// reads like prose, a line without a colon that reads like prose is
/// replaced whole, and every line is bounded.
pub fn redact(line: &str) -> String {
    const KEEP: usize = 120;
    const VALUE_CHARS: usize = 48;
    const PROSE_WORDS: usize = 6;
    let looks_like_prose = |s: &str| {
        let words: Vec<&str> = s.split_whitespace().collect();
        words.len() >= PROSE_WORDS && words.iter().all(|w| w.chars().any(char::is_alphabetic))
    };
    let redacted = match line.split_once(':') {
        Some((key, value)) => {
            let value = value.trim();
            if value.chars().count() > VALUE_CHARS || looks_like_prose(value) {
                format!(
                    "{}: <redacted {} chars>",
                    key.trim_end(),
                    value.chars().count()
                )
            } else {
                line.to_string()
            }
        }
        None if looks_like_prose(line) => format!("<redacted {} chars>", line.chars().count()),
        None => line.to_string(),
    };
    let mut out: String = redacted.chars().take(KEEP).collect();
    if redacted.chars().count() > KEEP {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_binaries_name_the_directories_searched() {
        let dir = tempfile::tempdir().unwrap();
        let err = find_binary("dettivo-engine-nope", Some(dir.path())).unwrap_err();
        match err {
            EngineError::NotFound { binary, searched } => {
                assert_eq!(binary, "dettivo-engine-nope");
                assert!(searched.contains(&dir.path().to_path_buf()));
                assert!(searched.len() > 1, "PATH entries are named too");
            }
            other => panic!("{other:?}"),
        }
        let script = dir.path().join("dettivo-engine-fake");
        std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
        assert_eq!(
            find_binary("dettivo-engine-fake", Some(dir.path())).unwrap(),
            script
        );
    }

    #[test]
    fn the_search_order_is_config_cuda_drop_in_package_dir_daemon_dir_then_path() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = search_dirs(Some(dir.path()));
        assert_eq!(dirs[0], dir.path());
        assert_eq!(dirs[1], Path::new(CUDA_DROP_IN_DIR));
        assert_eq!(dirs[2], Path::new(SYSTEM_ENGINE_DIR));
        let exe_dir = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        assert_eq!(dirs[3], exe_dir);
        let without = search_dirs(None);
        assert_eq!(without[0], Path::new(CUDA_DROP_IN_DIR));
        assert!(without.len() > 3, "PATH entries follow");
    }

    #[test]
    fn the_resident_set_reads_from_proc() {
        let own = resident_bytes(std::process::id()).expect("own VmRSS");
        assert!(own > 1024 * 1024, "{own}");
        assert_eq!(resident_bytes(u32::MAX), None);
    }

    #[test]
    fn redaction_bounds_a_line() {
        assert_eq!(redact("short"), "short");
        let long = "x".repeat(300);
        assert!(redact(&long).ends_with("..."));
        assert!(redact(&long).len() < 130);
    }

    #[test]
    fn redaction_drops_content_and_keeps_timings() {
        assert_eq!(
            redact("whisper_full: load time = 123.4 ms"),
            "whisper_full: load time = 123.4 ms"
        );
        assert_eq!(
            redact("model loaded backend=cpu"),
            "model loaded backend=cpu"
        );
        let leak = "segment: and so my fellow americans ask not what your country can do";
        let out = redact(leak);
        assert!(out.starts_with("segment: <redacted"), "{out}");
        assert!(!out.contains("americans"));
        let bare = "and so my fellow americans ask not what your country";
        assert!(redact(bare).starts_with("<redacted"));
        let key_value = "text: MARKER_PROMPT_TEXT_THAT_IS_LONGER_THAN_FORTY_EIGHT_CHARACTERS_1234";
        assert!(!redact(key_value).contains("MARKER"));
    }
}
