//! A daemon for a bench step: an isolated profile with the real model
//! directory linked in, the engines from the measured build, the mock
//! microphone feeding the speech fixture, the mock inserter (so the
//! insert leg costs nothing and no window is needed), and
//! `DETTIVO_FORCE_CPU=1` under `--cpu`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};

use super::Bench;
use crate::nfr::Tier;
use crate::profile::{ModelSource, Profile};
use crate::scenarios::daemon::DaemonHandle;
use crate::scenarios::{self, support::InsertDaemon};

/// How long a daemon may take to answer a ping.
pub const START_TIMEOUT: Duration = Duration::from_secs(20);

/// A daemon under a bench profile.
pub struct BenchDaemon {
    /// The daemon, stopped before its profile is removed.
    pub handle: DaemonHandle,
    /// The profile (its tree is removed when this is dropped).
    pub profile: Profile,
    /// The daemon's stderr, under the run directory.
    pub log: PathBuf,
}

/// What a bench daemon is configured with.
#[derive(Debug, Clone)]
pub struct Config {
    /// The speech provider.
    pub provider: String,
    /// The speech model id.
    pub model: String,
    /// The language model id, when one is on disk.
    pub llm_model: Option<String>,
    /// `[engines] stt_idle_seconds` and `llm_idle_seconds`.
    pub idle_seconds: u64,
}

/// A bench daemon's profile, configuration and environment, prepared
/// but not started: `spawn` is the part a startup measurement times.
pub struct Prepared {
    binary: PathBuf,
    profile: Profile,
    toml: String,
    extra: BTreeMap<String, String>,
    log: PathBuf,
}

impl Prepared {
    /// Starts the daemon and waits for its ping.
    pub fn spawn(mut self) -> Result<BenchDaemon, String> {
        let handle = DaemonHandle::spawn_logged(
            &self.binary,
            &mut self.profile,
            &self.toml,
            &self.extra,
            START_TIMEOUT,
            Some(&self.log),
        )?;
        Ok(BenchDaemon {
            profile: self.profile,
            handle,
            log: self.log,
        })
    }
}

impl BenchDaemon {
    /// Starts a daemon for `step` with `config`.
    pub fn start(bench: &Bench, step: &str, config: &Config) -> Result<Self, String> {
        Self::prepare(bench, step, config)?.spawn()
    }

    /// Creates the profile, links the models and writes the environment
    /// for `step`, without starting anything: the harness's own work,
    /// which a startup figure must not include.
    pub fn prepare(bench: &Bench, step: &str, config: &Config) -> Result<Prepared, String> {
        let binary = scenarios::binary(&bench.opts.repo_root, "dettivod")?;
        let source = bench
            .opts
            .models_dir
            .clone()
            .map(|real| ModelSource::new(real, &bench.opts.repo_root));
        let mut profile = Profile::create(&format!("bench-{step}"), source.as_ref())
            .map_err(|e| format!("profile: {e}"))?;
        profile.link_model(&format!("{}/{}", config.provider, config.model))?;
        if let Some(llm) = &config.llm_model {
            profile.link_model(&format!("llm/{llm}"))?;
        }
        let mut toml = format!(
            "[engines]\nstt_idle_seconds = {}\nllm_idle_seconds = {}\n",
            config.idle_seconds, config.idle_seconds
        );
        if let Some(dir) = bench.engines_dir() {
            toml.push_str(&format!("directory = \"{}\"\n", dir.display()));
        }
        toml.push_str(&format!(
            "[speech]\nprovider = \"{}\"\nmodel = \"{}\"\n[dictation]\nlanguage = \"en\"\n",
            config.provider, config.model
        ));
        if let Some(llm) = &config.llm_model {
            toml.push_str(&format!("[llm]\nmodel = \"{llm}\"\n"));
        }
        let mut extra = BTreeMap::new();
        extra.insert(
            "DETTIVO_MOCK_MIC".to_string(),
            bench.fixture.to_string_lossy().into_owned(),
        );
        if bench.opts.cpu {
            extra.insert("DETTIVO_FORCE_CPU".to_string(), "1".to_string());
        }
        // The mock inserter and the fixed focus target, not the whole mock
        // mode: the language model must be the real engine.
        extra.insert("DETTIVO_MOCK_MODE".to_string(), "0".to_string());
        extra.insert("DETTIVO_MOCK_INSERT".to_string(), "1".to_string());
        extra.insert("DETTIVO_MOCK_A11Y".to_string(), "1".to_string());
        let log = bench.run_dir.join(format!("{step}-daemon.log"));
        Ok(Prepared {
            binary,
            profile,
            toml,
            extra,
            log,
        })
    }

    /// The socket as the timing scenario's helpers take it.
    pub fn insert_daemon(&self) -> InsertDaemon {
        InsertDaemon::borrowed(&self.handle)
    }

    /// One request.
    pub fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        self.handle.call(method, params)
    }

    /// The tier and reason the daemon reports right now.
    pub fn tier(&self) -> Result<(Tier, String), String> {
        let caps = self.call("system.capabilities", json!({}))?;
        let platform = &caps["platform"];
        let tier = match platform["tier"].as_str() {
            Some("gpu") => Tier::Gpu,
            Some("cpu") => Tier::Cpu,
            other => return Err(format!("platform.tier is {other:?}")),
        };
        Ok((
            tier,
            platform["tier_reason"].as_str().unwrap_or("").to_string(),
        ))
    }

    /// The `speech.engines` rows.
    pub fn engines(&self) -> Result<Vec<Value>, String> {
        Ok(self.call("speech.engines", json!({}))?["engines"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    /// The daemon's resident set in bytes.
    pub fn rss_bytes(&self) -> Option<u64> {
        resident_bytes(self.handle.pid()?)
    }

    /// Stops the daemon and removes the profile's tree.
    pub fn stop(mut self) {
        self.handle.stop();
    }
}

/// The resident set of `pid` in bytes from `/proc/<pid>/status`.
pub fn resident_bytes(pid: u32) -> Option<u64> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

/// The tier a fresh daemon reports before any engine loads: the override
/// and the device. Also the preflight: a daemon that does not start
/// stops the suite.
pub fn probe_tier(bench: &Bench) -> Result<(Tier, String), String> {
    let (provider, model) = bench
        .models
        .get("whisper")
        .map(|m| (m.provider.clone(), m.id.clone()))
        .unwrap_or_else(|_| ("whisper".into(), "tiny.en".into()));
    let daemon = BenchDaemon::start(
        bench,
        "probe",
        &Config {
            provider,
            model,
            llm_model: None,
            idle_seconds: 300,
        },
    )?;
    let tier = daemon.tier();
    daemon.stop();
    tier
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_resident_set_reads_from_proc() {
        assert!(resident_bytes(std::process::id()).is_some_and(|b| b > 1 << 20));
        assert_eq!(resident_bytes(u32::MAX), None);
    }
}
