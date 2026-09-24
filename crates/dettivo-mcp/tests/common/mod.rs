//! Shared helpers for the MCP server tests: a seeded daemon in a private
//! XDG tree with QA mode and the mock inserter on, and the server binary.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::TempDir;

/// The server binary this crate built.
pub const SERVER: &str = env!("CARGO_BIN_EXE_dettivo-mcp");

/// The daemon binary: built beside this crate's binary by `cargo test
/// --workspace`; built on demand when a single-crate run skipped it.
pub fn daemon_binary() -> PathBuf {
    let candidate = Path::new(SERVER).parent().unwrap().join("dettivod");
    if !candidate.is_file() {
        let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-p", "dettivod"])
            .status()
            .expect("cargo build -p dettivod");
        assert!(status.success());
    }
    candidate
}

/// A private tree for one daemon.
pub struct Tree {
    pub dir: TempDir,
}

impl Tree {
    pub fn new() -> Self {
        Self {
            dir: tempfile::Builder::new()
                .prefix("dtm")
                .tempdir_in("/tmp")
                .unwrap(),
        }
    }

    pub fn root(&self) -> PathBuf {
        self.dir.path().to_path_buf()
    }

    pub fn socket(&self) -> PathBuf {
        self.root().join("run/dettivo/dettivo.sock")
    }

    pub fn config_file(&self) -> PathBuf {
        self.root().join("cfg/dettivo/config.toml")
    }

    pub fn write_config(&self, text: &str) {
        std::fs::create_dir_all(self.config_file().parent().unwrap()).unwrap();
        std::fs::write(self.config_file(), text).unwrap();
    }

    /// The isolated environment, with no inherited `DETTIVO_*` and no
    /// display or session bus (a test daemon never types into the desktop).
    pub fn env(&self, cmd: &mut Command) {
        for var in [
            "DETTIVO_CONFIG",
            "DETTIVO_IPC_SOCKET",
            "DETTIVO_IPC_TOKEN",
            "DETTIVO_DATA_DIR",
            "DETTIVO_QA",
            "WAYLAND_DISPLAY",
            "DISPLAY",
            "HYPRLAND_INSTANCE_SIGNATURE",
            "DBUS_SESSION_BUS_ADDRESS",
        ] {
            cmd.env_remove(var);
        }
        cmd.env("HOME", self.root())
            .env("XDG_CONFIG_HOME", self.root().join("cfg"))
            .env("XDG_STATE_HOME", self.root().join("state"))
            .env("XDG_DATA_HOME", self.root().join("data"))
            .env("XDG_CACHE_HOME", self.root().join("cache"))
            .env("XDG_RUNTIME_DIR", self.root().join("run"));
    }
}

/// The local test model and fixture clip (`scripts/models/fetch-test-model.sh`),
/// when present: the models directory and the clip.
pub fn local_model() -> Option<(PathBuf, PathBuf)> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let models = data.join("dettivo/models");
    let model = models.join("whisper/tiny.en/ggml-tiny.en.bin");
    let wav = models.join("fixtures/jfk.wav");
    (model.is_file() && wav.is_file()).then_some((models, wav))
}

/// A tree with the local models linked in and tiny.en selected, so an
/// import transcribes through the real engine beside the daemon binary.
pub fn tree_with_models(models: &Path) -> Tree {
    let tree = Tree::new();
    let data = tree.root().join("data/dettivo");
    std::fs::create_dir_all(&data).unwrap();
    link_test_models(models, &data);
    let bin_dir = Path::new(SERVER).parent().unwrap().to_path_buf();
    if !bin_dir.join("dettivo-engine-whisper").is_file() {
        let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-q", "-p", "dettivo-engine-whisper"])
            .status()
            .expect("cargo build");
        assert!(status.success());
    }
    tree.write_config(&format!(
        "[engines]\ndirectory = \"{}\"\n[speech]\nmodel = \"tiny.en\"\n",
        bin_dir.display()
    ));
    tree
}

/// A running seeded daemon with QA mode and the mock inserter on.
pub struct Daemon {
    pub tree: Tree,
    pub child: Child,
}

impl Daemon {
    pub fn spawn(tree: Tree, env: &[(&str, &str)]) -> Self {
        let mut cmd = Command::new(daemon_binary());
        tree.env(&mut cmd);
        cmd.env("DETTIVO_QA_MODE", "1")
            .env("DETTIVO_E2E_SEED", "1")
            .env("DETTIVO_MOCK_INSERT", "1");
        for (k, v) in env {
            cmd.env(k, v);
        }
        let child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        wait_ready(&tree.socket());
        Self { tree, child }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Polls until `system.ping` answers on `socket`.
pub fn wait_ready(socket: &Path) {
    let start = Instant::now();
    loop {
        if let Ok(mut s) = UnixStream::connect(socket) {
            let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
            if s.write_all(
                b"{\"jsonrpc\":\"2.0\",\"id\":\"r\",\"method\":\"system.ping\",\"params\":{}}\n",
            )
            .is_ok()
            {
                let mut line = String::new();
                if BufReader::new(s).read_line(&mut line).is_ok_and(|n| n > 0) {
                    return;
                }
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "daemon not ready on {}",
            socket.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

/// Links the local test models into `data/dettivo/models` the way the QA
/// profiles do (ADR 0043): the weights as symlinks, the manifest as a
/// copy, the fixture clips as a link. The daemon under test rewrites a
/// manifest when it re-verifies a model, so the real directory is never
/// the tree it sees.
pub fn link_test_models(models: &Path, data: &Path) {
    let tree = data.join("models");
    let tiny = tree.join("whisper/tiny.en");
    std::fs::create_dir_all(&tiny).unwrap();
    std::os::unix::fs::symlink(
        models.join("whisper/tiny.en/ggml-tiny.en.bin"),
        tiny.join("ggml-tiny.en.bin"),
    )
    .unwrap();
    let manifest = models.join("whisper/tiny.en/manifest.json");
    if manifest.is_file() {
        std::fs::copy(&manifest, tiny.join("manifest.json")).unwrap();
    }
    std::os::unix::fs::symlink(models.join("fixtures"), tree.join("fixtures")).unwrap();
}
