//! Runs scenarios: preflight, isolated profile, drive, evidence, leak
//! check, receipt.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::driver::{self, DriverError};
use crate::evidence::{Evidence, Outcome, Receipt, Timings};
use crate::profile::{ModelSource, Profile};
use crate::scenarios::{Context, Scenario};

/// Options for a drive.
#[derive(Debug, Clone)]
pub struct Options {
    /// `cua` or `atspi`.
    pub driver: String,
    /// The evidence base directory.
    pub evidence_base: std::path::PathBuf,
    /// The real model directory the test models are hard-linked from,
    /// when it exists; never linked into a profile as a whole.
    pub models_dir: Option<std::path::PathBuf>,
    /// Keep the profile tree after the run.
    pub keep_profile: bool,
    /// Window and label wait.
    pub timeout: Duration,
    /// An explicit run directory (a pack keeps every step under one);
    /// without it the process's own run directory under `evidence_base`.
    pub run_dir: Option<std::path::PathBuf>,
    /// Variables every process of the scenario also gets (a pack's
    /// `--cpu` sets `DETTIVO_FORCE_CPU=1`); empty for a plain drive.
    pub env: std::collections::BTreeMap<String, String>,
}

/// Drives one scenario and writes its receipt. Returns the receipt.
/// The directory component a driver name becomes in the evidence tree: the
/// name itself when it is a known driver, `invalid-driver` otherwise.
pub fn evidence_driver_name(driver: &str) -> &str {
    if driver::by_name(driver).is_ok() {
        driver
    } else {
        "invalid-driver"
    }
}

/// Runs one scenario on one driver into the evidence tree and returns its
/// receipt.
pub fn drive(
    repo_root: &Path,
    scenario: &dyn Scenario,
    opts: &Options,
) -> std::io::Result<Receipt> {
    let started_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // The driver name is part of the evidence path, so it is validated
    // before any directory is created: an unknown name lands in a fixed
    // `.invalid-driver` directory instead of wherever the text points.
    // `scenario_dir` creates the run directory itself, so nothing is made
    // ahead of that check.
    let driver_lookup = driver::by_name(&opts.driver);
    let dir_driver = evidence_driver_name(&opts.driver);
    let evidence = match &opts.run_dir {
        Some(run_dir) => Evidence {
            run_dir: run_dir.clone(),
        },
        None => Evidence::run_for_process(&opts.evidence_base)?,
    };
    let dir = evidence.scenario_dir(scenario.id(), dir_driver)?;
    let mut timings = Timings::start();
    let mut files = Vec::new();
    let mut receipt = Receipt {
        scenario: scenario.id().to_string(),
        driver: opts.driver.clone(),
        outcome: Outcome::Fail,
        reason: None,
        evidence: Vec::new(),
        timings_ms: Vec::new(),
        started_unix,
        duration_ms: 0,
    };

    let mut drv = match driver_lookup {
        Ok(d) => d,
        Err(e) => {
            receipt.reason = Some(e.to_string());
            Evidence::write_receipt(&dir, &receipt)?;
            return Ok(receipt);
        }
    };
    if scenario.needs_driver() {
        if let Err(e) = drv.preflight() {
            receipt.outcome = match e {
                DriverError::Missing(_) => Outcome::Skip,
                _ => Outcome::Fail,
            };
            receipt.reason = Some(e.to_string());
            Evidence::write_receipt(&dir, &receipt)?;
            return Ok(receipt);
        }
    }
    if let Err(reason) = scenario.preflight() {
        receipt.outcome = Outcome::Skip;
        receipt.reason = Some(reason);
        Evidence::write_receipt(&dir, &receipt)?;
        return Ok(receipt);
    }
    timings.mark("preflight");

    let source = opts
        .models_dir
        .clone()
        .map(|real| ModelSource::new(real, repo_root));
    let mut profile = Profile::create(scenario.id(), source.as_ref())?;
    profile.extend_env(&opts.env);
    if opts.keep_profile {
        profile.keep();
    }
    for name in scenario.models() {
        if let Err(missing) = profile.link_model(name) {
            drop(drv);
            receipt.outcome = Outcome::Skip;
            receipt.reason = Some(missing);
            receipt.timings_ms = timings.steps();
            receipt.duration_ms = timings.total_ms();
            Evidence::write_receipt(&dir, &receipt)?;
            return Ok(receipt);
        }
    }
    let result = {
        let mut ctx = Context {
            profile: &mut profile,
            evidence_dir: &dir,
            repo_root,
            timings: &mut timings,
            evidence: &mut files,
            timeout: opts.timeout,
        };
        match scenario.preconditions(&ctx) {
            Ok(()) => scenario.run(drv.as_mut(), &mut ctx),
            Err(missing) => {
                drop(drv);
                receipt.outcome = Outcome::Skip;
                receipt.reason = Some(missing);
                receipt.timings_ms = timings.steps();
                receipt.duration_ms = timings.total_ms();
                Evidence::write_receipt(&dir, &receipt)?;
                return Ok(receipt);
            }
        }
    };
    drop(drv);
    std::thread::sleep(Duration::from_millis(100));
    let leaks = profile.leaks();
    timings.mark("cleanup");

    receipt.evidence = files;
    receipt.timings_ms = timings.steps();
    receipt.duration_ms = timings.total_ms();
    match (result, leaks.is_empty()) {
        (Ok(()), true) => receipt.outcome = Outcome::Pass,
        (Ok(()), false) => {
            receipt.reason = Some(format!("leaked:\n{leaks}"));
        }
        (Err(why), clean) => {
            receipt.reason = Some(if clean {
                why
            } else {
                format!("{why}\nleaked:\n{leaks}")
            });
        }
    }
    Evidence::write_receipt(&dir, &receipt)?;
    Ok(receipt)
}
