//! Shared helpers for the CLI end-to-end tests: a temporary XDG tree, a
//! daemon spawned from the workspace build, and the `dettivo` binary run
//! against it.
#![allow(dead_code)]

use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::TempDir;

pub const CLI: &str = env!("CARGO_BIN_EXE_dettivo");

/// The daemon binary: built beside this crate's binary by `cargo test
/// --workspace`; built on demand when a single-crate run skipped it.
pub fn daemon_binary() -> PathBuf {
    let candidate = Path::new(CLI).parent().unwrap().join("dettivod");
    if !candidate.is_file() {
        let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-p", "dettivod"])
            .status()
            .expect("cargo build -p dettivod");
        assert!(status.success());
    }
    candidate
}

pub struct Tree {
    pub dir: TempDir,
}

impl Tree {
    pub fn new() -> Self {
        Self {
            dir: tempfile::Builder::new()
                .prefix("dtc")
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

pub struct Daemon {
    pub tree: Tree,
    pub child: Child,
}

impl Daemon {
    pub fn spawn(tree: Tree, env: &[(&str, &str)]) -> Self {
        let mut cmd = Command::new(daemon_binary());
        tree.env(&mut cmd);
        for (k, v) in env {
            cmd.env(k, v);
        }
        let child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let start = Instant::now();
        while UnixStream::connect(tree.socket()).is_err() {
            assert!(start.elapsed() < Duration::from_secs(5), "daemon not ready");
            thread::sleep(Duration::from_millis(10));
        }
        Self { tree, child }
    }

    /// Runs the CLI against this daemon.
    pub fn cli(&self, args: &[&str]) -> Output {
        cli_in(&self.tree, args, &[])
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn cli_in(tree: &Tree, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(CLI);
    tree.env(&mut cmd);
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.args(args).output().unwrap()
}

pub fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

pub fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}
