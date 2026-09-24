//! The evidence directory per run: screenshots, accessibility snapshots,
//! logs and the JSON receipt the flow-next QA pass and the release gate
//! read.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// The outcome of one scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Every assertion held and nothing leaked.
    Pass,
    /// An assertion failed or something leaked.
    Fail,
    /// A precondition (display, bus, tool) was missing.
    Skip,
}

/// The receipt written for every scenario run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    /// Scenario id.
    pub scenario: String,
    /// Driver name.
    pub driver: String,
    /// Outcome.
    pub outcome: Outcome,
    /// Why, when not a pass.
    pub reason: Option<String>,
    /// Evidence files relative to the scenario's evidence directory.
    pub evidence: Vec<String>,
    /// Timings in milliseconds by step name.
    pub timings_ms: Vec<(String, u64)>,
    /// Unix time the run started.
    pub started_unix: u64,
    /// Total wall time in milliseconds.
    pub duration_ms: u64,
}

/// One run's evidence root plus per-scenario directories.
#[derive(Debug, Clone)]
pub struct Evidence {
    /// The run directory (`qa-evidence/<run-id>`).
    pub run_dir: PathBuf,
}

impl Evidence {
    /// Creates `<base>/<run-id>` with a timestamp-based run id.
    /// The run directory for this process under `base`: created on the
    /// first call and reused after, so `drive all` keeps every scenario of
    /// one invocation under one `qa-evidence/<run>/`.
    pub fn run_for_process(base: &Path) -> std::io::Result<Self> {
        static RUNS: std::sync::OnceLock<
            std::sync::Mutex<std::collections::HashMap<PathBuf, PathBuf>>,
        > = std::sync::OnceLock::new();
        let runs = RUNS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
        let mut g = runs.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(dir) = g.get(base) {
            return Ok(Self {
                run_dir: dir.clone(),
            });
        }
        let run = Self::new_run(base)?;
        g.insert(base.to_path_buf(), run.run_dir.clone());
        Ok(run)
    }

    /// A fresh run directory `run-<unix>-<pid>` under `base`.
    pub fn new_run(base: &Path) -> std::io::Result<Self> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        let run_dir = base.join(format!("run-{}-{}", now.as_secs(), std::process::id()));
        std::fs::create_dir_all(&run_dir)?;
        Ok(Self { run_dir })
    }

    /// The directory for one scenario on one driver.
    pub fn scenario_dir(&self, scenario: &str, driver: &str) -> std::io::Result<PathBuf> {
        let dir = self.run_dir.join(format!("{scenario}.{driver}"));
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Writes `receipt.json` into the scenario directory.
    pub fn write_receipt(dir: &Path, receipt: &Receipt) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(receipt).map_err(std::io::Error::other)?;
        std::fs::write(dir.join("receipt.json"), text)
    }
}

/// Collects step timings while a scenario runs.
#[derive(Debug, Default)]
pub struct Timings {
    steps: Vec<(String, u64)>,
    started: Option<std::time::Instant>,
}

impl Timings {
    /// Starts the clock.
    pub fn start() -> Self {
        Self {
            steps: Vec::new(),
            started: Some(std::time::Instant::now()),
        }
    }

    /// Records the time since the last mark under `name`.
    pub fn mark(&mut self, name: &str) {
        let now = std::time::Instant::now();
        let since = self
            .started
            .map(|s| now.duration_since(s))
            .unwrap_or(Duration::ZERO);
        self.steps
            .push((name.to_string(), since.as_millis() as u64));
        self.started = Some(now);
    }

    /// The recorded steps.
    pub fn steps(&self) -> Vec<(String, u64)> {
        self.steps.clone()
    }

    /// Total of every step.
    pub fn total_ms(&self) -> u64 {
        self.steps.iter().map(|(_, ms)| ms).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipts_round_trip_and_land_in_the_scenario_dir() {
        let base = tempfile::tempdir().unwrap();
        let evidence = Evidence::new_run(base.path()).unwrap();
        let dir = evidence
            .scenario_dir("placeholder_window", "atspi")
            .unwrap();
        let receipt = Receipt {
            scenario: "placeholder_window".into(),
            driver: "atspi".into(),
            outcome: Outcome::Pass,
            reason: None,
            evidence: vec!["screenshot.png".into()],
            timings_ms: vec![("launch".into(), 120)],
            started_unix: 1,
            duration_ms: 120,
        };
        Evidence::write_receipt(&dir, &receipt).unwrap();
        let back: Receipt =
            serde_json::from_str(&std::fs::read_to_string(dir.join("receipt.json")).unwrap())
                .unwrap();
        assert_eq!(back, receipt);
        assert!(dir.ends_with("placeholder_window.atspi"));
    }
}
