//! GPU process presence and aggregate activity are retained as diagnostics.
//! Neither counter attributes inference work to the engine, so they cannot
//! establish workload proof. Missing required engine identity remains a failure.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// How often the GPU is read.
pub const INTERVAL: Duration = Duration::from_millis(500);

/// One reading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    /// Milliseconds since the sampler started.
    pub at_ms: u64,
    /// The watched parent's engine children (`dettivo-engine-*`) alive now.
    pub engine_pids: Vec<u32>,
    /// Pids the NVIDIA process table lists.
    pub nvidia_pids: Vec<u32>,
    /// The highest `gpu_busy_percent` across the AMD devices.
    pub amd_busy_percent: Option<u64>,
}

/// `gpu-samples.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Samples {
    /// `nvidia-smi`, `amd`, or `none` with the reason.
    pub counter: String,
    /// Why no counter, when none.
    pub counter_reason: Option<String>,
    /// The parent whose children were watched (daemon or QA CLI).
    pub daemon_pid: u32,
    /// Every reading.
    pub samples: Vec<Sample>,
}

/// The counter this machine offers.
pub fn counter() -> (String, Option<String>) {
    let nvidia = Command::new("nvidia-smi")
        .arg("--query-gpu=name")
        .arg("--format=csv,noheader")
        .output()
        .is_ok_and(|o| o.status.success());
    if nvidia {
        return ("nvidia-smi".into(), None);
    }
    if !amd_busy_paths().is_empty() {
        return ("amd".into(), None);
    }
    (
        "none".into(),
        Some("neither nvidia-smi nor /sys/class/drm/*/device/gpu_busy_percent is available".into()),
    )
}

fn amd_busy_paths() -> Vec<PathBuf> {
    std::fs::read_dir("/sys/class/drm")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path().join("device/gpu_busy_percent"))
        .filter(|p| p.is_file())
        .collect()
}

fn amd_busy() -> Option<u64> {
    amd_busy_paths()
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok()?.trim().parse::<u64>().ok())
        .max()
}

/// The pids in `nvidia-smi`'s process table: every row with a pid in
/// its third numeric column, which is both compute and graphics clients.
pub fn nvidia_pids_from(table: &str) -> Vec<u32> {
    let mut pids = Vec::new();
    let mut in_table = false;
    for line in table.lines() {
        if line.contains("Processes:") {
            in_table = true;
            continue;
        }
        if !in_table || !line.starts_with('|') {
            continue;
        }
        let fields: Vec<&str> = line.trim_matches('|').split_whitespace().collect();
        // GPU, GI, CI, PID, Type, name..., memory.
        if fields.len() >= 5
            && let Ok(pid) = fields[3].parse::<u32>()
            && !pids.contains(&pid)
        {
            pids.push(pid);
        }
    }
    pids
}

fn nvidia_pids() -> Vec<u32> {
    Command::new("nvidia-smi")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| nvidia_pids_from(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

/// The `dettivo-engine-*` children of `daemon_pid`, from `/proc`.
pub fn engine_pids(daemon_pid: u32) -> Vec<u32> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let stat = std::fs::read_to_string(entry.path().join("stat")).unwrap_or_default();
        let ppid = stat
            .rsplit(')')
            .next()
            .and_then(|rest| rest.split_whitespace().nth(1))
            .and_then(|p| p.parse::<u32>().ok());
        if ppid != Some(daemon_pid) {
            continue;
        }
        let cmdline = std::fs::read(entry.path().join("cmdline")).unwrap_or_default();
        let exe = cmdline.split(|b| *b == 0).next().unwrap_or(&[]);
        if String::from_utf8_lossy(exe).contains("dettivo-engine-") {
            out.push(pid);
        }
    }
    out
}

/// The sampler thread over one throughput job.
pub struct Sampler {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    shared: Arc<Mutex<Samples>>,
    out: PathBuf,
}

impl Sampler {
    /// Starts reading every `INTERVAL` until `stop`; the samples are
    /// written to `out` when it stops.
    pub fn start(daemon_pid: u32, out: PathBuf) -> Self {
        let (name, reason) = counter();
        let shared = Arc::new(Mutex::new(Samples {
            counter: name.clone(),
            counter_reason: reason,
            daemon_pid,
            samples: Vec::new(),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let (stop_flag, samples) = (stop.clone(), shared.clone());
        let handle = std::thread::spawn(move || {
            let started = Instant::now();
            while !stop_flag.load(Ordering::Relaxed) {
                let sample = Sample {
                    at_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    engine_pids: engine_pids(daemon_pid),
                    nvidia_pids: if name == "nvidia-smi" {
                        nvidia_pids()
                    } else {
                        Vec::new()
                    },
                    amd_busy_percent: if name == "amd" { amd_busy() } else { None },
                };
                if let Ok(mut s) = samples.lock() {
                    s.samples.push(sample);
                }
                std::thread::sleep(INTERVAL);
            }
        });
        Self {
            stop,
            handle: Some(handle),
            shared,
            out,
        }
    }

    /// Stops the thread and writes the samples; returns them.
    pub fn stop(mut self) -> Samples {
        self.halt();
        let samples = self.shared.lock().map(|s| s.clone()).unwrap_or_default();
        if let Ok(json) = serde_json::to_string_pretty(&samples) {
            let _ = std::fs::write(&self.out, json);
        }
        samples
    }
}

impl Sampler {
    fn halt(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// A sampler dropped on an early return (an import that failed to start)
/// stops and joins its thread instead of leaving it polling for the rest
/// of the pack; the samples are not written, the pack failed before them.
impl Drop for Sampler {
    fn drop(&mut self) {
        self.halt();
    }
}

/// The verdict over the samples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Work was attributed to the engine; allocation counters never yield this.
    Pass(String),
    /// No counter, or no GPU work expected; the reason says which.
    Skip(String),
    /// A GPU tier whose engine never appeared.
    Fail(String),
}

/// Judges `samples` for a run on `tier` (`gpu` or `cpu`).
pub fn judge(samples: &Samples, tier: &str, forced_cpu: bool) -> Verdict {
    if samples.counter == "none" {
        return Verdict::Skip(
            samples
                .counter_reason
                .clone()
                .unwrap_or_else(|| "no GPU counter".into()),
        );
    }
    if forced_cpu || tier != "gpu" {
        return Verdict::Skip(format!(
            "the run was on the {tier} tier{}: no GPU work expected",
            if forced_cpu { " (--cpu)" } else { "" }
        ));
    }
    let with_engine: Vec<&Sample> = samples
        .samples
        .iter()
        .filter(|s| !s.engine_pids.is_empty())
        .collect();
    if with_engine.is_empty() {
        return Verdict::Fail(format!(
            "no dettivo-engine process was seen under the daemon in {} samples",
            samples.samples.len()
        ));
    }
    if samples.counter == "nvidia-smi" {
        let mut pids: Vec<u32> = Vec::new();
        for pid in with_engine
            .iter()
            .flat_map(|s| s.engine_pids.iter().copied())
        {
            if !pids.contains(&pid) {
                pids.push(pid);
            }
        }
        let hits = |pid: u32| {
            with_engine
                .iter()
                .filter(|s| s.nvidia_pids.contains(&pid))
                .count()
        };
        return match pids.iter().map(|p| (*p, hits(*p))).find(|(_, n)| *n > 0) {
            Some((pid, n)) => Verdict::Skip(format!(
                "workload attribution unavailable: engine pid {pid} has a GPU allocation in {n} of {} samples; process presence does not prove inference execution",
                with_engine.len()
            )),
            None => Verdict::Fail(format!(
                "the engine pids {pids:?} never appeared in nvidia-smi over {} samples",
                with_engine.len()
            )),
        };
    }
    let busy = with_engine
        .iter()
        .filter_map(|s| s.amd_busy_percent)
        .max()
        .unwrap_or(0);
    Verdict::Skip(format!(
        "workload attribution unavailable: aggregate gpu_busy_percent peaked at {busy}% in {} samples; other processes or devices may supply this activity",
        with_engine.len()
    ))
}

/// Requires the exact CUDA process identity before checking workload attribution.
pub fn judge_diarize(samples: &Samples, engine_pid: u32) -> Verdict {
    if samples.counter != "nvidia-smi" {
        return Verdict::Fail(format!(
            "CUDA diarize pid {engine_pid} requires nvidia-smi process proof; counter is {}",
            samples.counter
        ));
    }
    let mut matching = samples.clone();
    for sample in &mut matching.samples {
        sample.engine_pids.retain(|pid| *pid == engine_pid);
    }
    judge(&matching, "gpu", false)
}

/// Reads the samples the throughput step wrote under `run_dir`.
pub fn read(run_dir: &Path) -> Result<Samples, String> {
    let path = run_dir.join("meeting_throughput/gpu-samples.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("{}: {e} (did meeting_throughput run?)", path.display()))?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(engine: &[u32], nvidia: &[u32], amd: Option<u64>) -> Sample {
        Sample {
            at_ms: 0,
            engine_pids: engine.to_vec(),
            nvidia_pids: nvidia.to_vec(),
            amd_busy_percent: amd,
        }
    }

    #[test]
    fn allocation_and_unrelated_activity_are_not_workload_proof() {
        for counter in ["nvidia-smi", "amd"] {
            let samples = Samples {
                counter: counter.into(),
                samples: vec![sample(&[42], &[42], Some(100))],
                ..Samples::default()
            };
            assert!(
                matches!(judge(&samples, "gpu", false), Verdict::Skip(reason) if reason.contains("attribution"))
            );
        }
    }

    #[test]
    fn the_process_table_yields_compute_and_graphics_pids() {
        let table = "| Processes: |\n|  GPU   GI   CI  PID   Type   Process name   GPU Memory |\n|====|\n|    0   N/A  N/A   2686      G   Hyprland   268MiB |\n|    0   N/A  N/A   4007014   C   bun   428MiB |\n|    0   N/A  N/A   2686      G   again   1MiB |\n";
        assert_eq!(nvidia_pids_from(table), [2686, 4007014]);
        assert!(nvidia_pids_from("no table").is_empty());
    }

    #[test]
    fn the_verdict_needs_the_engine_pid_on_the_gpu_tier_only() {
        let mut samples = Samples {
            counter: "nvidia-smi".into(),
            counter_reason: None,
            daemon_pid: 1,
            samples: vec![
                sample(&[], &[9], None),
                sample(&[42], &[9], None),
                sample(&[42], &[9, 42], None),
            ],
        };
        assert!(
            matches!(judge(&samples, "gpu", false), Verdict::Skip(r) if r.contains("pid 42") && r.contains("1 of 2") && r.contains("attribution"))
        );
        assert!(matches!(judge(&samples, "cpu", false), Verdict::Skip(_)));
        assert!(matches!(judge(&samples, "gpu", true), Verdict::Skip(r) if r.contains("--cpu")));
        samples.samples[2].nvidia_pids = vec![9];
        assert!(matches!(judge(&samples, "gpu", false), Verdict::Fail(r) if r.contains("[42]")));
        samples.samples.clear();
        assert!(matches!(judge(&samples, "gpu", false), Verdict::Fail(_)));
        let amd = Samples {
            counter: "amd".into(),
            counter_reason: None,
            daemon_pid: 1,
            samples: vec![sample(&[7], &[], Some(3)), sample(&[7], &[], Some(55))],
        };
        assert!(
            matches!(judge(&amd, "gpu", false), Verdict::Skip(r) if r.contains("55%") && r.contains("attribution"))
        );
        let none = Samples {
            counter: "none".into(),
            counter_reason: Some("no counter".into()),
            ..Samples::default()
        };
        assert!(matches!(judge(&none, "gpu", false), Verdict::Skip(r) if r == "no counter"));
    }

    #[test]
    fn engine_children_are_read_from_proc() {
        // This test process has no engine children.
        assert!(engine_pids(std::process::id()).is_empty());
    }

    #[test]
    fn diarization_proof_requires_its_exact_pid_and_nvidia_counter() {
        let mut samples = Samples {
            counter: "nvidia-smi".into(),
            samples: vec![sample(&[42, 7], &[7], None)],
            ..Samples::default()
        };
        assert!(matches!(judge_diarize(&samples, 42), Verdict::Fail(_)));
        samples.samples[0].nvidia_pids.push(42);
        assert!(
            matches!(judge_diarize(&samples, 42), Verdict::Skip(r) if r.contains("42") && r.contains("attribution"))
        );
        for counter in ["none", "amd"] {
            samples.counter = counter.into();
            assert!(matches!(judge_diarize(&samples, 42), Verdict::Fail(_)));
        }
    }
}

#[cfg(test)]
mod sampler_tests {
    use super::*;

    /// qa-packs/F13: dropping the sampler (an early `?` in the pack) stops
    /// and joins its thread.
    #[test]
    fn a_dropped_sampler_stops_its_thread() {
        let dir = tempfile::tempdir().unwrap();
        let sampler = Sampler::start(std::process::id(), dir.path().join("gpu-samples.json"));
        let stop = sampler.stop.clone();
        let handle_present = sampler.handle.is_some();
        drop(sampler);
        assert!(handle_present);
        assert!(
            stop.load(Ordering::Relaxed),
            "the drop raised the stop flag"
        );
        assert!(
            !dir.path().join("gpu-samples.json").exists(),
            "a dropped sampler writes nothing"
        );
    }
}
