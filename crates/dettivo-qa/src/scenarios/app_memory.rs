//! Five fresh release app processes per desktop reference mode (ADR 0059).
//! Uses the existing private app socket and scenario evidence, without screenshots.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::app_memory_launch::AppProcess;
use super::app_support::{self, FIRST_FRAME_BUDGET_MS, app_command, app_env, daemon_config};
use super::daemon::DaemonHandle;
use super::{Context, Scenario, binary, build_profile};
use crate::driver::Driver;

/// An explicit reference mode. These probes are selected by id, not by `drive all`.
pub struct AppMemory(pub &'static str);

impl Scenario for AppMemory {
    fn id(&self) -> &'static str {
        match self.0 {
            "wayland" => "app_memory.wayland",
            "xvfb" => "app_memory.xvfb",
            _ => "app_memory.unsupported",
        }
    }

    fn summary(&self) -> &'static str {
        "five fresh release Home launches with connected five-second idle memory and startup gates"
    }

    fn needs_driver(&self) -> bool {
        false
    }

    fn run(&self, _driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        // Run-time checks produce failures in the runner, never successful skips.
        let mut report = json!({"mode": self.0, "required_launches": 5, "samples": [], "within_budget": false, "asserted": true, "rss_budget_kb": app_support::RSS_BUDGET_KB, "anonymous_budget_kb": app_support::ANONYMOUS_BUDGET_KB, "pss_budget_kb": null});
        let result = self.measure(ctx, &mut report);
        if let Err(why) = &result {
            report["error"] = json!(why);
        }
        write_json(&ctx.evidence_dir.join("startup.json"), &report)?;
        ctx.evidence.push("startup.json".into());
        result
    }
}

impl AppMemory {
    fn measure(&self, ctx: &mut Context<'_>, report: &mut Value) -> Result<(), String> {
        if build_profile() != "release" || cfg!(debug_assertions) {
            return Err("reference memory measurement requires DETTIVO_BUILD_PROFILE=release and `just build-release`".into());
        }
        let display_env = reference_environment(self.0)?;
        let app = binary(ctx.repo_root, "dettivo-app")?;
        let daemon = binary(ctx.repo_root, "dettivod")?;
        let cli = binary(ctx.repo_root, "dettivo")?;
        let cache = fs::read_to_string(ctx.repo_root.join("build/qt/CMakeCache.txt"))
            .map_err(|e| e.to_string())?;
        if !cache
            .lines()
            .any(|l| l == "CMAKE_BUILD_TYPE:STRING=Release")
        {
            return Err("Qt CMake build is not Release".into());
        }
        report["build"] = json!({
            "git_sha": crate::bench::host::git_sha(ctx.repo_root),
            "working_tree_status": String::from_utf8_lossy(&std::process::Command::new("git").args(["status", "--porcelain"]).current_dir(ctx.repo_root).output().map_err(|e| e.to_string())?.stdout),
            "qt_configuration": "Release", "rust_profile": "release",
            "app": identity(&app)?, "daemon": identity(&daemon)?, "cli": identity(&cli)?,
            "qa": identity(&std::env::current_exe().map_err(|e| e.to_string())?)?,
        });
        report["host"] =
            serde_json::to_value(crate::bench::host::Host::detect()).map_err(|e| e.to_string())?;
        report["display_environment"] = json!(display_env);
        report["reference_window"] = json!({"logical_width": 1280, "logical_height": 820, "native_placement": "one-shot Hyprland exec float/size rule for this process only", "scale": "native compositor DPR, unchanged"});
        report["fixture"] = json!({"seed": "DETTIVO_E2E_SEED=1", "route": "home", "theme": "builtin-dark", "capture": "mock mode, no capture requested", "actions": "no playback, downloads or analysis requested", "cache": "private profile, first launch cold; later launches reuse its cache", "profile": ctx.profile.root});
        let mut env = display_env;
        env.insert(
            "XDG_RUNTIME_DIR".into(),
            ctx.profile.root.join("run").to_string_lossy().into_owned(),
        );
        ctx.profile.extend_env(&env);
        let seed = BTreeMap::from([("DETTIVO_E2E_SEED".into(), "1".into())]);
        let daemon = DaemonHandle::spawn_logged(
            &daemon,
            ctx.profile,
            &daemon_config(ctx.repo_root),
            &seed,
            ctx.timeout,
            Some(&ctx.evidence_dir.join("daemon.log")),
        )?;
        report["excluded_processes"] = json!({"daemon_pid": daemon.pid(), "engines": "separate processes excluded; none requested"});
        let mut samples = Vec::new();
        for launch in 1..=5 {
            let dir = ctx.evidence_dir.join(format!("launch-{launch}"));
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let mut sample = json!({"launch": launch, "within_budget": false});
            let result = self.sample(ctx, &app, &dir, &mut sample);
            if let Err(error) = result {
                sample["error"] = json!(error);
            }
            write_json(&dir.join("sample.json"), &sample)?;
            ctx.evidence.push(format!("launch-{launch}/sample.json"));
            samples.push(sample);
            report["samples"] = json!(samples);
            write_json(&ctx.evidence_dir.join("startup.json"), report)?;
        }
        for key in [
            "rss_kb_after_5s",
            "pss_kb_after_5s",
            "anonymous_kb_after_5s",
            "first_frame_ms",
        ] {
            let maximum = samples.iter().filter_map(|s| s[key].as_u64()).max();
            report["maxima"][key] = json!(maximum);
        }
        let pass = samples.len() == 5
            && samples
                .iter()
                .all(|s| s["within_budget"] == true && s.get("error").is_none());
        report["within_budget"] = json!(pass);
        if pass {
            Ok(())
        } else {
            Err("one or more designated launches failed; see every sample in startup.json".into())
        }
    }

    fn sample(
        &self,
        ctx: &mut Context<'_>,
        app: &Path,
        dir: &Path,
        sample: &mut Value,
    ) -> Result<(), String> {
        app_support::reset_journal(ctx);
        let env = app_env(ctx, "home", None);
        let mut child = AppProcess::spawn(self.0, app, &env, dir)?;
        let pid = child.pid();
        sample["pid"] = json!(pid);
        let started = Instant::now();
        let result = (|| {
            let mut idle_since = None;
            let status = loop {
                if child.exited()? {
                    return Err(format!("app process {pid} exited before sampling"));
                }
                if started.elapsed() > ctx.timeout + Duration::from_secs(5) {
                    return Err("Home never remained connected and idle for five seconds".into());
                }
                match app_command(ctx, &json!({"cmd": "status"})) {
                    Ok(status) => {
                        sample["last_status"] = status.clone();
                        if reference_ready(&status, self.0, pid)? {
                            let since = idle_since.get_or_insert_with(Instant::now);
                            if since.elapsed() >= Duration::from_secs(5) {
                                break status;
                            }
                        } else {
                            idle_since = None;
                        }
                    }
                    Err(_) => idle_since = None,
                }
                std::thread::sleep(Duration::from_millis(100));
            };
            let proc_dir = PathBuf::from(format!("/proc/{pid}"));
            sample["process_executable"] =
                json!(fs::read_link(proc_dir.join("exe")).map_err(|e| e.to_string())?);
            let stat = fs::read_to_string(proc_dir.join("stat")).map_err(|e| e.to_string())?;
            fs::write(dir.join("proc-stat.txt"), stat).map_err(|e| e.to_string())?;
            let raw_status = read_capture(&proc_dir.join("status"), &dir.join("proc-status.txt"))?;
            let rollup = read_capture(
                &proc_dir.join("smaps_rollup"),
                &dir.join("proc-smaps_rollup.txt"),
            )?;
            let memory = app_support::parse_memory(&raw_status, &rollup).ok_or("missing or malformed procfs memory metrics (expected VmRSS, Pss and Anonymous in kB)")?;
            if child.exited()? {
                return Err("app exited during memory sampling".into());
            }
            sample
                .as_object_mut()
                .unwrap()
                .extend(memory.evidence().as_object().unwrap().clone());
            let first = status["first_frame_ms"]
                .as_u64()
                .ok_or("missing first_frame_ms")?;
            sample["first_frame_ms"] = json!(first);
            let journal = app_support::wait_journal(ctx, "first_frame", ctx.timeout)?;
            sample["application_ms"] = journal["application_ms"].clone();
            sample["qml_ms"] = journal["qml_ms"].clone();
            sample["after_qml_to_first_frame_ms"] = json!(
                first
                    .checked_sub(
                        journal["qml_ms"]
                            .as_u64()
                            .ok_or("missing cumulative QML timing")?
                    )
                    .ok_or("invalid cumulative QML timing")?
            );
            sample["first_frame_budget_ms"] = json!(FIRST_FRAME_BUDGET_MS);
            sample["first_frame_within_budget"] = json!(first <= FIRST_FRAME_BUDGET_MS);
            sample["within_budget"] =
                json!(memory.within_budget() && first <= FIRST_FRAME_BUDGET_MS);
            sample["idle_ms"] = json!(idle_since.unwrap().elapsed().as_millis());
            sample["elapsed_since_launch_ms"] = json!(started.elapsed().as_millis());
            sample["sampled_unix_ms"] = json!(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_millis()
            );
            Ok(())
        })();
        if app_support::journal_path(ctx).is_file() {
            fs::copy(app_support::journal_path(ctx), dir.join("app.jsonl"))
                .map_err(|e| e.to_string())?;
        }
        drop(child);
        result
    }
}

fn reference_ready(status: &Value, mode: &str, pid: u32) -> Result<bool, String> {
    let platform = match mode {
        "wayland" => "wayland",
        "xvfb" => "xcb",
        _ => return Err(format!("unsupported reference mode {mode}")),
    };
    if status["pid"].as_u64() != Some(u64::from(pid)) {
        return Err("status came from a different process".into());
    }
    let env = &status["environment"];
    if env["platform"] != platform {
        return Err(format!(
            "expected actual Qt {platform}, observed {}",
            env["platform"]
        ));
    }
    if env["qt_build"] != "Release" {
        return Err("running Qt app does not identify as Release".into());
    }
    if status["first_frame_ms"].as_u64().is_none_or(|ms| ms == 0) {
        return Ok(false);
    }
    if !matches!(
        env["renderer"].as_str(),
        Some("OpenGL" | "Vulkan" | "Software")
    ) || env["device_pixel_ratio"].as_f64().is_none_or(|v| v <= 0.0)
        || env["window_width"].as_u64().is_none_or(|v| v == 0)
        || env["window_height"].as_u64().is_none_or(|v| v == 0)
    {
        return Err("missing renderer/window/scale metadata".into());
    }
    Ok(env["window_width"] == 1280
        && env["window_height"] == 820
        && status["route"] == "home"
        && status["visible"] == true
        && status["daemon_connected"] == true
        && status["daemon_state"] == "connected"
        && status["dictation_state"] == "idle"
        && status["playback_active"] == false
        && status["meeting_active"] == false
        && status["active_downloads"] == 0
        && matches!(status["analysis_status"].as_str(), Some("" | "idle"))
        && status["first_frame_ms"].as_u64().is_some_and(|ms| ms > 0))
}

fn reference_environment(mode: &str) -> Result<BTreeMap<String, String>, String> {
    if fs::read_to_string("/proc/sys/kernel/hostname")
        .map_err(|e| e.to_string())?
        .trim()
        != "thor"
    {
        return Err(
            "designated desktop reference requires Thor; label other hosts separately".into(),
        );
    }
    match mode {
        "wayland" => {
            let display = PathBuf::from(
                std::env::var("WAYLAND_DISPLAY")
                    .map_err(|_| "native Wayland display unavailable")?,
            );
            let socket = if display.is_absolute() {
                display
            } else {
                PathBuf::from(
                    std::env::var("XDG_RUNTIME_DIR").map_err(|_| "Wayland runtime unavailable")?,
                )
                .join(display)
            };
            if !fs::metadata(&socket).is_ok_and(|m| m.file_type().is_socket()) {
                return Err(format!("Wayland socket unavailable: {}", socket.display()));
            }
            Ok(BTreeMap::from([
                ("QT_QPA_PLATFORM".into(), "wayland".into()),
                (
                    "WAYLAND_DISPLAY".into(),
                    socket.to_string_lossy().into_owned(),
                ),
            ]))
        }
        "xvfb" => {
            let display = std::env::var("DISPLAY").map_err(|_| "Xvfb display unavailable")?;
            let number = display
                .strip_prefix(':')
                .and_then(|s| s.split('.').next())
                .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
                .ok_or("Xvfb requires a local numbered display")?;
            let pid = fs::read_to_string(format!("/tmp/.X{number}-lock"))
                .map_err(|e| format!("Xvfb server lock: {e}"))?;
            let pid: u32 = pid
                .trim()
                .parse()
                .map_err(|_| "malformed Xvfb server pid")?;
            let exe = fs::read_link(format!("/proc/{pid}/exe")).map_err(|e| e.to_string())?;
            if exe.file_name().is_none_or(|name| name != "Xvfb") {
                return Err("display server is not Xvfb".into());
            }
            Ok(BTreeMap::from([
                ("QT_QPA_PLATFORM".into(), "xcb".into()),
                ("DISPLAY".into(), display),
            ]))
        }
        _ => Err(format!("unsupported reference mode {mode}")),
    }
}

fn read_capture(source: &Path, destination: &Path) -> Result<String, String> {
    let text = fs::read_to_string(source).map_err(|e| format!("{}: {e}", source.display()))?;
    fs::write(destination, &text).map_err(|e| e.to_string())?;
    Ok(text)
}

fn identity(path: &Path) -> Result<Value, String> {
    let data = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(json!({"path": path, "sha256": format!("{:x}", Sha256::digest(data))}))
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    fs::write(
        path,
        serde_json::to_string_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> Value {
        json!({"pid": 123, "environment": {"platform": "wayland", "qt_build": "Release", "renderer": "OpenGL", "device_pixel_ratio": 1.0, "window_width": 1280, "window_height": 820}, "first_frame_ms": 250, "route": "home", "visible": true, "daemon_connected": true, "daemon_state": "connected", "dictation_state": "idle", "playback_active": false, "meeting_active": false, "active_downloads": 0, "analysis_status": ""})
    }

    #[test]
    fn reference_requires_actual_platform_release_metadata_and_process_identity() {
        assert!(reference_ready(&ready(), "wayland", 123).unwrap());
        for (field, value) in [
            ("platform", json!("xcb")),
            ("qt_build", json!("Debug")),
            ("renderer", Value::Null),
            ("device_pixel_ratio", json!(0)),
            ("window_width", Value::Null),
        ] {
            let mut status = ready();
            status["environment"][field] = value;
            assert!(reference_ready(&status, "wayland", 123).is_err(), "{field}");
        }
        assert!(reference_ready(&ready(), "wayland", 999).is_err());
        assert!(reference_ready(&ready(), "unsupported", 123).is_err());
        let mut status = ready();
        status["environment"]["platform"] = json!("xcb");
        assert!(reference_ready(&status, "xvfb", 123).unwrap());
    }

    #[test]
    fn an_incomplete_or_busy_home_cannot_start_the_idle_clock() {
        for (field, value) in [
            ("daemon_connected", json!(false)),
            ("route", json!("history")),
            ("dictation_state", json!("recording")),
            ("playback_active", json!(true)),
            ("meeting_active", json!(true)),
            ("active_downloads", json!(1)),
            ("analysis_status", json!("running")),
            ("first_frame_ms", json!(-1)),
            ("playback_active", Value::Null),
        ] {
            let mut status = ready();
            status[field] = value;
            assert!(
                !reference_ready(&status, "wayland", 123).unwrap(),
                "{field}"
            );
        }
    }

    #[test]
    fn exited_process_and_unreadable_metrics_are_failures() {
        let mut profile = crate::profile::Profile::create("memory-exit", None).unwrap();
        let evidence = tempfile::tempdir().unwrap();
        let mut timings = crate::evidence::Timings::start();
        let mut files = Vec::new();
        let mut ctx = Context {
            profile: &mut profile,
            evidence_dir: evidence.path(),
            repo_root: evidence.path(),
            timings: &mut timings,
            evidence: &mut files,
            timeout: Duration::from_secs(1),
        };
        let mut sample = json!({"within_budget": false});
        let error = AppMemory("xvfb")
            .sample(
                &mut ctx,
                Path::new("/bin/true"),
                evidence.path(),
                &mut sample,
            )
            .unwrap_err();
        assert!(error.contains("exited before sampling"), "{error}");
        assert_eq!(sample["within_budget"], false);
        assert!(
            read_capture(
                &evidence.path().join("missing"),
                &evidence.path().join("copy")
            )
            .is_err()
        );
        assert!(read_capture(evidence.path(), &evidence.path().join("copy")).is_err());
    }
}
