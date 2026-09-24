//! A daemon under test: the binary spawned with a private XDG tree under a
//! short `/tmp` path (Unix socket paths are limited to 108 bytes), a
//! one-line client, and a clean stop that returns the captured log.

#![allow(dead_code)]

pub mod diarize;
pub mod meetings;
pub mod subscriber;

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::TempDir;

/// The daemon binary this crate built.
pub const DAEMON: &str = env!("CARGO_BIN_EXE_dettivod");

/// A private tree for one daemon.
pub struct Tree {
    /// The temporary root; removed on drop.
    pub dir: TempDir,
}

impl Tree {
    /// A fresh tree under `/tmp` with short paths.
    pub fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("dtv")
            .tempdir_in("/tmp")
            .expect("tempdir");
        Self { dir }
    }

    pub fn root(&self) -> PathBuf {
        self.dir.path().to_path_buf()
    }

    pub fn config_file(&self) -> PathBuf {
        self.root().join("cfg/dettivo/config.toml")
    }

    pub fn state_file(&self) -> PathBuf {
        self.root().join("state/dettivo/state.toml")
    }

    pub fn socket(&self) -> PathBuf {
        self.root().join("run/dettivo/dettivo.sock")
    }

    pub fn pid_file(&self) -> PathBuf {
        self.root().join("run/dettivo/dettivod.pid")
    }

    /// Writes the configuration file before the daemon starts.
    pub fn write_config(&self, text: &str) {
        let path = self.config_file();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// A command for the daemon binary with this tree's environment and
    /// no inherited `DETTIVO_*` overrides.
    pub fn command(&self) -> Command {
        let mut cmd = Command::new(DAEMON);
        let root = self.root();
        for var in [
            "DETTIVO_CONFIG",
            "DETTIVO_IPC_SOCKET",
            "DETTIVO_IPC_TOKEN",
            "DETTIVO_DATA_DIR",
            "DETTIVO_QA",
            "RUST_LOG",
            "LISTEN_FDS",
            "LISTEN_PID",
            // No display: a test daemon must never type into the desktop
            // that runs the tests. A test that wants insertion sets the
            // QA mock.
            "WAYLAND_DISPLAY",
            "DISPLAY",
            "HYPRLAND_INSTANCE_SIGNATURE",
            // No session bus either: a test daemon must never bind global
            // shortcuts through the desktop's real portal. A test that
            // wants a bus starts a private one.
            "DBUS_SESSION_BUS_ADDRESS",
        ] {
            cmd.env_remove(var);
        }
        cmd.env("HOME", &root)
            .env("XDG_CONFIG_HOME", root.join("cfg"))
            .env("XDG_STATE_HOME", root.join("state"))
            .env("XDG_DATA_HOME", root.join("data"))
            // Transfer spools and export files live under the cache home;
            // an inherited one is shared by every daemon the parallel
            // tests spawn, and their counter-based transfer ids collide.
            .env("XDG_CACHE_HOME", root.join("cache"))
            .env("XDG_RUNTIME_DIR", root.join("run"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd
    }
}

/// A running daemon.
pub struct Daemon {
    pub tree: Tree,
    child: Child,
    stderr: Arc<Mutex<Vec<u8>>>,
    stdout: Arc<Mutex<Vec<u8>>>,
}

impl Daemon {
    /// Spawns with extra environment and waits for the socket.
    pub fn spawn(tree: Tree, env: &[(&str, &str)]) -> Self {
        let mut cmd = tree.command();
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().expect("spawn dettivod");
        let stderr = capture(child.stderr.take().unwrap());
        let stdout = capture(child.stdout.take().unwrap());
        let daemon = Self {
            tree,
            child,
            stderr,
            stdout,
        };
        daemon.wait_ready(Duration::from_secs(5));
        daemon
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Polls until the daemon answers a ping: the socket is bound before
    /// the state file is written and the server loop starts, so a mere
    /// connection is not readiness.
    pub fn wait_ready(&self, budget: Duration) {
        let start = Instant::now();
        while start.elapsed() < budget {
            if let Ok(mut s) = UnixStream::connect(self.tree.socket()) {
                let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                let ping = "{\"jsonrpc\":\"2.0\",\"id\":\"ready\",\"method\":\"system.ping\",\"params\":{}}\n";
                if s.write_all(ping.as_bytes()).is_ok() {
                    let mut line = String::new();
                    if std::io::BufReader::new(s)
                        .read_line(&mut line)
                        .is_ok_and(|n| n > 0)
                    {
                        return;
                    }
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!(
            "daemon not ready within {budget:?}; log so far:\n{}",
            String::from_utf8_lossy(&self.stderr.lock().unwrap())
        );
    }

    /// One connection, one line, one answer.
    pub fn call(&self, line: &str) -> serde_json::Value {
        let mut conn = Conn::open(&self.tree.socket());
        conn.send(line)
    }

    /// `result` with its own read window, for calls that wait on a real
    /// engine (a cold model load on a loaded machine outruns the default).
    pub fn result_within(
        &self,
        method: &str,
        params: serde_json::Value,
        window: Duration,
    ) -> serde_json::Value {
        let req = serde_json::json!({"jsonrpc":"2.0","id":"1","method":method,"params":params});
        let mut conn = Conn::open_with_timeout(&self.tree.socket(), window);
        let resp = conn.send(&req.to_string());
        assert!(resp.get("result").is_some(), "{method} failed: {resp}");
        resp["result"].clone()
    }

    /// Sends a request built from method and params, returns the response.
    pub fn request(&self, method: &str, params: serde_json::Value) -> serde_json::Value {
        let req = serde_json::json!({"jsonrpc":"2.0","id":"1","method":method,"params":params});
        self.call(&req.to_string())
    }

    /// The result of a successful call, panicking on an error response.
    pub fn result(&self, method: &str, params: serde_json::Value) -> serde_json::Value {
        let resp = self.request(method, params);
        assert!(resp.get("result").is_some(), "{method} failed: {resp}");
        resp["result"].clone()
    }

    /// SIGTERM, wait, return the captured log.
    pub fn stop(mut self) -> String {
        self.shutdown()
    }

    /// `stop`, keeping the tree for a second daemon on the same files.
    pub fn stop_keep(mut self) -> (String, Tree) {
        let log = self.shutdown();
        (log, std::mem::replace(&mut self.tree, Tree::new()))
    }

    /// SIGKILL (a crash, a power loss), keeping the tree for a second
    /// daemon that recovers what the first left behind.
    pub fn kill_keep(mut self) -> Tree {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(self.tree.socket());
        let _ = std::fs::remove_file(self.tree.pid_file());
        std::mem::replace(&mut self.tree, Tree::new())
    }

    fn shutdown(&mut self) -> String {
        let _ = Command::new("kill")
            .args(["-TERM", &self.child.id().to_string()])
            .status();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    assert!(status.success(), "daemon exited with {status}");
                    break;
                }
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
                Ok(None) => {
                    let _ = self.child.kill();
                    panic!("daemon did not stop within 10 s after SIGTERM");
                }
                Err(e) => panic!("wait: {e}"),
            }
        }
        thread::sleep(Duration::from_millis(50));
        String::from_utf8_lossy(&self.stderr.lock().unwrap()).into_owned()
    }

    /// The log captured so far.
    pub fn log(&self) -> String {
        String::from_utf8_lossy(&self.stderr.lock().unwrap()).into_owned()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn capture<R: Read + Send + 'static>(mut reader: R) -> Arc<Mutex<Vec<u8>>> {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let sink = buf.clone();
    thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        while let Ok(n) = reader.read(&mut chunk) {
            if n == 0 {
                break;
            }
            sink.lock().unwrap().extend_from_slice(&chunk[..n]);
        }
    });
    buf
}

/// The local tiny.en model and the jfk.wav fixture, when both exist.
pub fn local_model() -> Option<(PathBuf, PathBuf)> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let model = data.join("dettivo/models/whisper/tiny.en/ggml-tiny.en.bin");
    let wav = data.join("dettivo/models/fixtures/jfk.wav");
    (model.is_file() && wav.is_file()).then_some((model, wav))
}

/// A tree whose models directory links the local tiny.en in and whose
/// config selects it, with the engine binary from the build directory;
/// `extra` is appended to the configuration file.
pub fn tree_with_tiny(model: &std::path::Path, extra: &str) -> Tree {
    let tree = Tree::new();
    let models = tree.root().join("data/dettivo/models/whisper/tiny.en");
    std::fs::create_dir_all(&models).unwrap();
    std::os::unix::fs::symlink(model, models.join("ggml-tiny.en.bin")).unwrap();
    let bin_dir = std::path::Path::new(DAEMON).parent().unwrap().to_path_buf();
    if !bin_dir.join("dettivo-engine-whisper").is_file() {
        let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-q", "-p", "dettivo-engine-whisper"])
            .status()
            .expect("cargo build");
        assert!(status.success());
    }
    tree.write_config(&format!(
        "[engines]\ndirectory = \"{}\"\n[speech]\nmodel = \"tiny.en\"\n[dictation]\nlanguage = \"en\"\nreplacements = {{ ask = \"ASK\" }}\n{extra}",
        bin_dir.display()
    ));
    tree
}

/// Polls `check` every 20 ms until it holds or `budget` passes.
pub fn wait_for(budget: Duration, mut check: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < budget {
        if check() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}

/// A persistent connection for multi-line exchanges.
pub struct Conn {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl Conn {
    pub fn open(socket: &std::path::Path) -> Self {
        Self::open_with_timeout(socket, Duration::from_secs(30))
    }

    /// A connection whose reads give up after `window`.
    pub fn open_with_timeout(socket: &std::path::Path, window: Duration) -> Self {
        let stream = UnixStream::connect(socket).expect("connect");
        stream.set_read_timeout(Some(window)).unwrap();
        let writer = stream.try_clone().unwrap();
        Self {
            reader: BufReader::new(stream),
            writer,
        }
    }

    /// Writes one line and reads one line back.
    pub fn send(&mut self, line: &str) -> serde_json::Value {
        self.writer.write_all(line.as_bytes()).unwrap();
        self.writer.write_all(b"\n").unwrap();
        self.read()
    }

    /// Writes raw bytes (no newline added).
    pub fn write_raw(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).unwrap();
    }

    /// Reads one response line.
    pub fn read(&mut self) -> serde_json::Value {
        self.try_read().expect("read one response line")
    }

    /// Reads one response line, reporting a timeout or close as an error.
    pub fn try_read(&mut self) -> Result<serde_json::Value, String> {
        let mut answer = String::new();
        let n = self
            .reader
            .read_line(&mut answer)
            .map_err(|e| format!("read line: {e}"))?;
        if n == 0 {
            return Err("connection closed".into());
        }
        serde_json::from_str(answer.trim_end()).map_err(|e| format!("response is not JSON: {e}"))
    }
}
