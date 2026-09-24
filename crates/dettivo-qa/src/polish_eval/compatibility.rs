use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

use super::{
    Options, Sample, Targets,
    report::{self, Report},
};

/// Non-text provenance required before comparing promotion metrics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comparison {
    /// Canonical selected sample content, including gold and scoring fragments.
    pub dataset_sha256: String,
    /// The scorer and dataset reader used by this build.
    pub scorer_sha256: String,
    /// The complete promotion targets.
    pub targets_sha256: String,
    /// The pipeline binaries, runner, host identity and execution switches.
    pub execution_sha256: String,
}

fn hash(value: &impl Serialize) -> Result<String, String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

impl Comparison {
    pub(super) fn capture(
        repo: &Path,
        opts: &Options,
        rows: &[Sample],
        targets: &Targets,
        engine: Option<&Path>,
    ) -> Result<Self, String> {
        let daemon = crate::scenarios::binary(repo, "dettivod")?;
        let daemon_hash = report::sha256(&daemon).ok_or("cannot fingerprint evaluation daemon")?;
        let host = ["/etc/machine-id", "/proc/sys/kernel/random/boot_id"]
            .into_iter()
            .filter_map(|path| std::fs::read_to_string(path).ok())
            .find(|id| !id.trim().is_empty())
            .ok_or("cannot fingerprint evaluation host: no machine or boot identity")?;
        let engine_hash = engine
            .map(|p| report::sha256(p).ok_or("cannot fingerprint evaluation engine"))
            .transpose()?;
        Ok(Self {
            dataset_sha256: hash(&rows)?,
            scorer_sha256: hash(&(include_str!("scorer.rs"), include_str!("dataset.rs")))?,
            targets_sha256: hash(targets)?,
            execution_sha256: hash(&(
                daemon_hash,
                engine_hash,
                host.trim(),
                include_str!("runner.rs"),
                opts.timeout_ms,
                opts.cpu,
                opts.engine_only,
            ))?,
        })
    }
}

pub(super) fn mismatch(
    report: &Report,
    expected: &Comparison,
    split: &str,
    rows: &[Sample],
) -> Option<String> {
    let Some(actual) = report.comparison.as_ref() else {
        return Some("incumbent report lacks comparison metadata; rerun the incumbent".into());
    };
    let reason = if report.split != split {
        "selected split differs"
    } else if report.rows != rows.len()
        || report.row_ids != rows.iter().map(|s| s.id.clone()).collect::<Vec<_>>()
    {
        "selected row identities differ"
    } else if actual.dataset_sha256 != expected.dataset_sha256 {
        "selected dataset content differs"
    } else if actual.scorer_sha256 != expected.scorer_sha256 {
        "scoring rules differ"
    } else if actual.targets_sha256 != expected.targets_sha256 {
        "promotion targets differ"
    } else if actual.execution_sha256 != expected.execution_sha256 {
        "execution conditions differ"
    } else {
        return None;
    };
    Some(reason.into())
}
