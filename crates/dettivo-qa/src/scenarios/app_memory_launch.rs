//! Process ownership for the memory probe, including a transient Hyprland
//! launch rule that gives only the private QA window its reference size.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

pub(super) struct AppProcess {
    child: Option<Child>,
    pid: u32,
    start_time: Option<String>,
}

impl AppProcess {
    pub fn spawn(
        mode: &str,
        app: &Path,
        env: &BTreeMap<String, String>,
        dir: &Path,
    ) -> Result<Self, String> {
        let absolute_dir = fs::canonicalize(dir).map_err(|e| e.to_string())?;
        let dir = absolute_dir.as_path();
        let command = crate::profile::command(app, env);
        if mode != "wayland" {
            let mut command = command;
            let child = command
                .stdin(Stdio::null())
                .stdout(Stdio::from(
                    fs::File::create(dir.join("app.stdout")).map_err(|e| e.to_string())?,
                ))
                .stderr(Stdio::from(
                    fs::File::create(dir.join("app.stderr")).map_err(|e| e.to_string())?,
                ))
                .spawn()
                .map_err(|e| format!("app launch: {e}"))?;
            let pid = child.id();
            return Ok(Self {
                child: Some(child),
                pid,
                start_time: process_start(pid),
            });
        }
        let socket = Path::new(env.get("WAYLAND_DISPLAY").ok_or("missing native display")?)
            .file_name()
            .ok_or("missing Wayland socket name")?;
        let instances = Command::new("hyprctl")
            .args(["instances", "-j"])
            .output()
            .map_err(|e| format!("Hyprland instance lookup: {e}"))?;
        let instances: Value = serde_json::from_slice(&instances.stdout)
            .map_err(|e| format!("Hyprland instances: {e}"))?;
        let matches: Vec<_> = instances
            .as_array()
            .ok_or("invalid Hyprland instance list")?
            .iter()
            .filter(|i| i["wl_socket"].as_str().is_some_and(|s| s == socket))
            .collect();
        if matches.len() != 1 {
            return Err(
                "native reference requires exactly one Hyprland instance for the Wayland socket"
                    .into(),
            );
        }
        let instance = matches[0]["instance"]
            .as_str()
            .ok_or("missing Hyprland instance identity")?;
        let pid_file = dir.join("app.pid");
        let environment = command
            .get_envs()
            .filter_map(|(key, value)| {
                value.map(|v| {
                    shell_quote(&format!(
                        "{}={}",
                        key.to_string_lossy(),
                        v.to_string_lossy()
                    ))
                })
            })
            .collect::<Vec<_>>()
            .join(" ");
        let script = format!(
            "printf '%s' $$ > {} || exit 125; exec {} > {} 2> {}",
            shell_quote(&pid_file.to_string_lossy()),
            shell_quote(&app.to_string_lossy()),
            shell_quote(&dir.join("app.stdout").to_string_lossy()),
            shell_quote(&dir.join("app.stderr").to_string_lossy())
        );
        let launch = format!(
            "exec /usr/bin/env -i {environment} /bin/sh -c {}",
            shell_quote(&script)
        );
        let dispatch = format!(
            "hl.dsp.exec_cmd({}, {{float = true, size = {{1280, 820}}}})",
            serde_json::to_string(&launch).map_err(|e| e.to_string())?
        );
        let output = Command::new("hyprctl")
            .args(["-i", instance, "dispatch", &dispatch])
            .output()
            .map_err(|e| e.to_string())?;
        fs::write(
            dir.join("hyprland-launch.txt"),
            [&output.stdout[..], &output.stderr[..]].concat(),
        )
        .map_err(|e| e.to_string())?;
        if !output.status.success() || String::from_utf8_lossy(&output.stdout).trim() != "ok" {
            return Err(
                "Hyprland refused the transient reference-window launch; see hyprland-launch.txt"
                    .into(),
            );
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(text) = fs::read_to_string(&pid_file) {
                if let Ok(pid) = text.trim().parse::<u32>() {
                    return Ok(Self {
                        child: None,
                        pid,
                        start_time: process_start(pid),
                    });
                }
            }
            if Instant::now() > deadline {
                return Err("Hyprland launch never announced its app process".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn exited(&mut self) -> Result<bool, String> {
        if let Some(child) = &mut self.child {
            return child
                .try_wait()
                .map(|exit| exit.is_some())
                .map_err(|e| e.to_string());
        }
        Ok(self.start_time.is_none() || process_start(self.pid) != self.start_time)
    }
}

fn process_start(pid: u32) -> Option<String> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = stat.rsplit_once(") ")?.1;
    if rest.starts_with("Z ") {
        return None;
    }
    rest.split_whitespace().nth(19).map(str::to_owned)
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\"'\"'"))
}

impl Drop for AppProcess {
    fn drop(&mut self) {
        if self.exited().unwrap_or(true) {
            return;
        }
        let _ = Command::new("kill")
            .args(["-TERM", &self.pid.to_string()])
            .output();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !self.exited().unwrap_or(true) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        if !self.exited().unwrap_or(true) {
            let _ = Command::new("kill")
                .args(["-KILL", &self.pid.to_string()])
                .output();
        }
        if let Some(child) = &mut self.child {
            let _ = child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_values_remain_literal_shell_arguments() {
        let literal = "space ' quote $() `command`\nnewline";
        let output = Command::new("sh")
            .args(["-c", &format!("printf '%s' {}", shell_quote(literal))])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), literal);
    }
}
