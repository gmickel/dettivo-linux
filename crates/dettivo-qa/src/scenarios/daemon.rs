//! A daemon under a scenario's profile: its configuration is the
//! scenario's settings over the profile's baseline (`profile::with_baseline`,
//! no hotkey backend unless the scenario names one), spawned with the
//! profile's environment plus the scenario's own variables, ready once
//! it answers `system.ping`, stopped with SIGTERM so its socket goes
//! with it.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::profile::Profile;
use crate::replay;

/// A running daemon.
pub struct DaemonHandle {
    child: Option<Child>,
    /// The socket it answers on.
    pub socket: PathBuf,
}

impl DaemonHandle {
    /// Writes `config` over the profile's baseline into the profile,
    /// starts `dettivod` with the profile's environment and `extra`, and
    /// waits for a ping.
    pub fn spawn(
        binary: &Path,
        profile: &mut Profile,
        config: &str,
        extra: &BTreeMap<String, String>,
        timeout: Duration,
    ) -> Result<Self, String> {
        Self::spawn_logged(binary, profile, config, extra, timeout, None)
    }

    /// `spawn` with the daemon's stderr appended to `log` when one is
    /// named, so a step that measures a daemon keeps what it said.
    pub fn spawn_logged(
        binary: &Path,
        profile: &mut Profile,
        config: &str,
        extra: &BTreeMap<String, String>,
        timeout: Duration,
        log: Option<&Path>,
    ) -> Result<Self, String> {
        let stderr = match log {
            Some(path) => std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map(Stdio::from)
                .map_err(|e| format!("{}: {e}", path.display()))?,
            None => Stdio::null(),
        };
        let config_file = profile.root.join("cfg/dettivo/config.toml");
        std::fs::create_dir_all(config_file.parent().unwrap_or(&profile.root))
            .map_err(|e| format!("config dir: {e}"))?;
        std::fs::write(&config_file, crate::profile::with_baseline(config))
            .map_err(|e| format!("config: {e}"))?;
        let mut env = profile.env();
        env.extend(extra.clone());
        let child = crate::profile::command(binary, &env)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(stderr)
            .spawn()
            .map_err(|e| format!("start {}: {e}", binary.display()))?;
        profile.track("dettivod", &child);
        let socket = profile.socket();
        let handle = Self {
            child: Some(child),
            socket,
        };
        let deadline = Instant::now() + timeout;
        while !replay::answers_ping(&handle.socket) {
            if Instant::now() > deadline {
                return Err(format!(
                    "dettivod did not answer on {} within {timeout:?}",
                    handle.socket.display()
                ));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(handle)
    }

    /// One JSON-RPC request; the `result` object, or the error's message.
    pub fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let line = serde_json::json!({
            "jsonrpc": "2.0", "id": "qa", "method": method, "params": params
        })
        .to_string();
        let answer = replay::call(&self.socket, &line).map_err(|e| format!("{method}: {e}"))?;
        if let Some(error) = answer.get("error") {
            return Err(format!(
                "{method}: {}",
                error["message"].as_str().unwrap_or("error")
            ));
        }
        Ok(answer["result"].clone())
    }

    /// A subscription to `topics` on its own connection, positioned after
    /// the `events.subscribe` answer.
    pub fn subscribe(&self, topics: &[&str]) -> Result<EventStream, String> {
        EventStream::open(&self.socket, topics)
    }

    /// SIGKILL (a crash, a power loss): nothing is cleaned up, so the next
    /// daemon on the profile finds what this one left. The socket file is
    /// removed here so the next start can bind it.
    pub fn kill(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_file(&self.socket);
        let _ = std::fs::remove_file(self.socket.with_file_name("dettivod.pid"));
    }

    /// The daemon's process id while it runs.
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    /// SIGTERM, then a bounded wait; the daemon removes its socket.
    pub fn stop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let _ = Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .output();
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// An open `events.subscribe` connection: the notifications arrive as
/// their `params` objects (`topic`, `payload`, `timestamp`).
pub struct EventStream {
    reader: BufReader<UnixStream>,
}

impl EventStream {
    fn open(socket: &Path, topics: &[&str]) -> Result<Self, String> {
        let mut stream = UnixStream::connect(socket).map_err(|e| format!("subscribe: {e}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| e.to_string())?;
        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": "sub", "method": "events.subscribe",
            "params": {"topics": topics, "buffer": 512}
        });
        stream
            .write_all(format!("{request}\n").as_bytes())
            .map_err(|e| format!("subscribe: {e}"))?;
        let mut reader = BufReader::new(stream);
        let mut answer = String::new();
        reader
            .read_line(&mut answer)
            .map_err(|e| format!("subscribe: {e}"))?;
        let parsed: Value = serde_json::from_str(answer.trim_end()).map_err(|e| e.to_string())?;
        if let Some(error) = parsed.get("error") {
            return Err(format!("events.subscribe: {error}"));
        }
        Ok(Self { reader })
    }

    /// How long one read waits for the next notification before `collect`
    /// gives up (ten seconds unless set): a step whose events are minutes
    /// apart, a CPU import of long audio, sets it longer.
    pub fn set_read_timeout(&mut self, timeout: Duration) -> Result<(), String> {
        self.reader
            .get_ref()
            .set_read_timeout(Some(timeout))
            .map_err(|e| e.to_string())
    }

    /// Reads notifications until `done` answers true for one, the daemon
    /// closes the connection or `timeout` passes; every notification read
    /// is returned, the one that satisfied `done` included.
    pub fn collect(
        &mut self,
        mut done: impl FnMut(&Value) -> bool,
        timeout: Duration,
    ) -> Vec<Value> {
        let deadline = Instant::now() + timeout;
        let mut out = Vec::new();
        while Instant::now() < deadline {
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let Ok(value) = serde_json::from_str::<Value>(line.trim_end()) else {
                        continue;
                    };
                    if value["method"] != "events.notify" {
                        continue;
                    }
                    let params = value["params"].clone();
                    let stop = done(&params);
                    out.push(params);
                    if stop {
                        break;
                    }
                }
            }
        }
        out
    }
}

/// The local tiny.en model and the speech fixture, as the profile links
/// them in (`scripts/models/fetch-test-model.sh` fetches both).
pub fn local_model(profile: &Profile) -> Option<(PathBuf, PathBuf)> {
    let models = profile.root.join("data/dettivo/models");
    let model = models.join("whisper/tiny.en/ggml-tiny.en.bin");
    let wav = models.join("fixtures/jfk.wav");
    (model.is_file() && wav.is_file()).then_some((model, wav))
}

#[cfg(test)]
mod startup_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn readiness_failure_terminates_and_reaps_before_profile_drop() {
        let mut profile = Profile::create("startupguard", None).unwrap();
        let script = profile.root.join("never-ready");
        let pid_file = profile.root.join("child.pid");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\necho $$ > {}\nexec sleep 30\n",
                pid_file.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let result = DaemonHandle::spawn(
            &script,
            &mut profile,
            "",
            &BTreeMap::new(),
            Duration::from_millis(100),
        );
        assert!(result.is_err());
        let pid: u32 = std::fs::read_to_string(pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let survived = Path::new(&format!("/proc/{pid}")).exists();
        if survived {
            let _ = Command::new("kill")
                .args(["-KILL", &pid.to_string()])
                .status();
        }
        assert!(
            !survived,
            "readiness timeout left child {pid} alive or unreaped"
        );
    }
}
