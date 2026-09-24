//! The bench report's types: the spread of a measurement, a step's
//! status, a figure beside its target, the step, the load average and
//! the report itself (`schema_version` 1, docs/qa.md).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::host;
use crate::nfr::{self, Target, Tier};
use crate::stats::percentile;

/// The report schema this binary writes.
pub const SCHEMA_VERSION: u32 = 1;

/// The spread of one measurement: every sample, so a bad run is never
/// averaged away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spread {
    /// Samples taken.
    pub iterations: usize,
    /// Every sample, in run order.
    pub samples: Vec<u64>,
    /// Nearest-rank p50.
    pub p50: u64,
    /// Nearest-rank p95.
    pub p95: u64,
    /// The smallest sample.
    pub min: u64,
    /// The largest sample.
    pub max: u64,
}

impl Spread {
    /// The spread of `samples`; `None` for no samples.
    pub fn of(samples: Vec<u64>) -> Option<Self> {
        Some(Self {
            iterations: samples.len(),
            p50: percentile(&samples, 0.5)?,
            p95: percentile(&samples, 0.95)?,
            min: *samples.iter().min()?,
            max: *samples.iter().max()?,
            samples,
        })
    }
}

/// What a step produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Numbers were taken.
    Measured,
    /// A model, a fixture or a tool is missing; the reason names it.
    Skipped,
    /// The measurement does not exist yet (a placeholder).
    NotAvailable,
    /// The step ran and broke; the reason says how.
    Failed,
}

/// A measurement beside its target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    /// `NFR-1`, `NFR-2`, ...
    pub nfr: String,
    /// What is compared.
    pub metric: String,
    /// The unit.
    pub unit: String,
    /// The calibrated target (ADR 0029).
    pub target: f64,
    /// The masterplan's initial figure.
    pub initial: f64,
    /// `at_most` or `at_least`.
    pub direction: nfr::Direction,
    /// The figure measured, when one was.
    pub value: Option<f64>,
    /// Whether the figure meets the target; `None` without a figure.
    pub met: Option<bool>,
}

impl Comparison {
    /// `value` against `target`.
    pub fn of(target: &Target, value: Option<f64>) -> Self {
        Self {
            nfr: target.nfr.to_string(),
            metric: target.metric.to_string(),
            unit: target.unit.to_string(),
            target: target.calibrated,
            initial: target.initial,
            direction: target.direction,
            value,
            met: value.map(|v| target.met(v)),
        }
    }
}

/// One step of the report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    /// The step name.
    pub name: String,
    /// What it produced.
    pub status: Status,
    /// Why, when not measured; a missing model names its download.
    pub reason: Option<String>,
    /// The measurement beside its target, when the step has one.
    pub comparison: Option<Comparison>,
    /// One row per engine (or per measurement) with its numbers, spread,
    /// model, backend and reason.
    pub results: Vec<Value>,
    /// Wall time.
    pub duration_ms: u64,
}

impl Step {
    /// A step that took no numbers.
    pub fn without(name: &str, status: Status, reason: String) -> Self {
        Self {
            name: name.into(),
            status,
            reason: Some(reason),
            comparison: None,
            results: Vec::new(),
            duration_ms: 0,
        }
    }
}

/// The machine's load average (`/proc/loadavg`, 1, 5 and 15 minutes)
/// when the run started and when it ended, so a report taken on a busy
/// machine says so beside its numbers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LoadAverage {
    /// At the start of the run.
    pub start: Option<[f64; 3]>,
    /// At the end of the run.
    pub end: Option<[f64; 3]>,
}

/// The load average right now.
pub fn load_average() -> Option<[f64; 3]> {
    let text = std::fs::read_to_string("/proc/loadavg").ok()?;
    let mut parts = text.split_whitespace().map(|p| p.parse::<f64>().ok());
    Some([parts.next()??, parts.next()??, parts.next()??])
}

/// The report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// `1`.
    pub schema_version: u32,
    /// Unix time the run started.
    pub generated_unix: u64,
    /// The date, `YYYY-MM-DD`.
    pub date: String,
    /// The commit the binaries were built from.
    pub git_sha: String,
    /// `--quick`.
    pub quick: bool,
    /// Iterations of the timed steps.
    pub iterations: usize,
    /// `gpu` or `cpu`.
    pub tier: Tier,
    /// Why that tier, as the daemon reported it.
    pub tier_reason: String,
    /// `--cpu` (`DETTIVO_FORCE_CPU=1`).
    pub forced_cpu: bool,
    /// What the CPU run is until a CPU VM exists.
    pub notes: Vec<String>,
    /// The machine.
    pub host: host::Host,
    /// The machine's load average at the start and the end of the run.
    #[serde(default)]
    pub load_average: LoadAverage,
    /// The engine binaries measured.
    pub engines: Vec<host::EngineBinary>,
    /// The models measured, with the catalogue's checksums.
    pub models: Vec<host::ModelRef>,
    /// The fixtures, with their hashes.
    pub fixtures: Vec<host::FixtureRef>,
    /// The steps, in run order.
    pub steps: Vec<Step>,
    /// Every target the suite knows, initial and calibrated.
    pub targets: Vec<Target>,
    /// The meeting lane's figures, filed by `dettivo-qa pack meetings
    /// --record` (ADR 0039); absent until the pack has run on the host
    /// and tier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meetings: Option<super::meetings::MeetingsBlock>,
}

/// `YYYY-MM-DD` for a Unix time (UTC).
pub fn date_of(unix: u64) -> String {
    let days = unix / 86_400;
    // Civil-from-days (Howard Hinnant), enough for a report file name.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spread_keeps_every_sample_and_the_comparison_judges_by_direction() {
        let s = Spread::of(vec![300, 100, 200]).unwrap();
        assert_eq!(
            (s.p50, s.p95, s.min, s.max, s.iterations),
            (200, 300, 100, 300, 3)
        );
        assert_eq!(s.samples, [300, 100, 200]);
        assert!(Spread::of(Vec::new()).is_none());
        let c = Comparison::of(&nfr::NFR1_FIRST_INSERT_GPU, Some(900.0));
        assert_eq!((c.nfr.as_str(), c.met), ("NFR-1", Some(true)));
        let c = Comparison::of(&nfr::NFR4_STT_REALTIME_GPU, Some(3.0));
        assert_eq!(c.met, Some(false));
        assert_eq!(Comparison::of(&nfr::NFR6_IDLE_RSS, None).met, None);
    }

    #[test]
    fn the_load_average_reads_three_figures() {
        let load = load_average().expect("/proc/loadavg");
        assert!(load.iter().all(|l| *l >= 0.0));
    }

    #[test]
    fn dates_come_from_unix_time() {
        assert_eq!(date_of(0), "1970-01-01");
        assert_eq!(date_of(1_788_912_000), "2026-09-09");
        assert_eq!(date_of(951_782_400), "2000-02-29");
    }
}
