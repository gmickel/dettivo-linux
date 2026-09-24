//! The promotion gate (`scripts/polish_finetune/check_promotion.py` on
//! macOS) ported rule for rule: the absolute hard gates from
//! `qa/polish-eval/targets.json` (the macOS promotion targets), the
//! relative gates against an incumbent report (match or beat every hard
//! gate, improve one of the named latency metrics). Relative promotion
//! requires compatible report provenance and the same backend. A missing
//! requested comparison fails promotion independently of absolute acceptance.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// One hard gate: a floor, a ceiling, or both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Rule {
    /// The value may not fall under this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    /// The value may not rise over this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
}

/// The relative gates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelativeGates {
    /// Every hard-gate metric must match or beat the incumbent's.
    pub must_match_or_beat_incumbent_hard_gates: bool,
    /// At least one of these must improve on the incumbent.
    pub must_improve_one_of: Vec<String>,
}

/// `targets.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Targets {
    /// The targets id (`dettivo-polish-private-v1`).
    pub id: String,
    /// Metric name to rule.
    pub hard_gates: BTreeMap<String, Rule>,
    /// The relative gates.
    pub relative_gates: RelativeGates,
    /// Anything else the macOS file carries, kept as is.
    #[serde(flatten)]
    pub rest: BTreeMap<String, serde_json::Value>,
}

impl Targets {
    /// Reads the file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// One hard gate as it came out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateRow {
    /// The metric.
    pub metric: String,
    /// The candidate's value; `None` when the report lacks the metric.
    pub value: Option<f64>,
    /// The rule.
    pub rule: Rule,
    /// Whether it passed.
    pub pass: bool,
}

/// The relative comparison, when one was made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relative {
    /// The incumbent report's model.
    pub incumbent_model: String,
    /// The incumbent's backend.
    pub incumbent_backend: String,
    /// The metrics that regressed against the incumbent, one line each.
    pub regressions: Vec<String>,
    /// The latency metrics that improved.
    pub improved_metrics: Vec<String>,
}

/// The decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    /// The targets id.
    pub targets: String,
    /// Every hard gate stated as pass or fail.
    pub hard_gates: Vec<GateRow>,
    /// The relative comparison, or `None` without an incumbent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative: Option<Relative>,
    /// Why the relative gates were not run, when an incumbent was given
    /// but not compared (incompatible provenance or a different backend).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative_skipped: Option<String>,
    /// One line per failure, the macOS wording.
    pub failures: Vec<String>,
    /// Whether the absolute gates passed, independently of requested promotion.
    #[serde(default)]
    pub absolute_passed: bool,
    /// No failure.
    pub passed: bool,
}

/// The metrics where a higher value is better.
pub const HIGHER_IS_BETTER: &[&str] = &[
    "protected_token_preservation_rate",
    "required_fragment_retention_rate",
    "language_anchor_retention_rate",
    "structure_retention_rate",
    "exact_match_rate",
];

/// The metrics where a lower value is better.
pub const LOWER_IS_BETTER: &[&str] = &[
    "forbidden_fragment_violation_rate",
    "guard_rejection_rate",
    "fallback_rate",
    "median_wall_ms",
    "p95_wall_ms",
];

/// An incumbent's side of the comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct Incumbent {
    /// Its model id.
    pub model: String,
    /// Its backend.
    pub backend: String,
    /// Its metrics.
    pub metrics: BTreeMap<String, f64>,
    /// Why the source report cannot be compared with the selected run.
    pub incompatible: Option<String>,
}

/// Checks `candidate` (its metrics and backend) against the targets and,
/// when given, the incumbent.
pub fn check(
    targets: &Targets,
    candidate: &BTreeMap<String, f64>,
    candidate_backend: &str,
    incumbent: Option<&Incumbent>,
) -> Decision {
    let mut failures = Vec::new();
    let mut hard_gates = Vec::new();
    for (metric, rule) in &targets.hard_gates {
        let value = candidate.get(metric).copied();
        let mut pass = true;
        match value {
            None => {
                pass = false;
                failures.push(format!("{metric} missing from the candidate report"));
            }
            Some(v) => {
                if let Some(min) = rule.minimum {
                    if v < min {
                        pass = false;
                        failures.push(format!("{metric} below minimum: {v} < {min}"));
                    }
                }
                if let Some(max) = rule.maximum {
                    if v > max {
                        pass = false;
                        failures.push(format!("{metric} above maximum: {v} > {max}"));
                    }
                }
            }
        }
        hard_gates.push(GateRow {
            metric: metric.clone(),
            value,
            rule: rule.clone(),
            pass,
        });
    }
    let absolute_passed = failures.is_empty();
    let mut relative = None;
    let mut relative_skipped = None;
    if let Some(inc) = incumbent {
        if let Some(reason) = &inc.incompatible {
            relative_skipped = Some(reason.clone());
        } else if inc.backend != candidate_backend {
            relative_skipped = Some(format!(
                "the incumbent ran on {} and the candidate on {candidate_backend}; the relative gates compare reports from one backend only",
                inc.backend
            ));
        } else {
            let mut regressions = Vec::new();
            if targets
                .relative_gates
                .must_match_or_beat_incumbent_hard_gates
            {
                for metric in HIGHER_IS_BETTER {
                    if let (Some(c), Some(i)) = (candidate.get(*metric), inc.metrics.get(*metric)) {
                        if c < i {
                            regressions.push(format!("{metric} regressed vs incumbent: {c} < {i}"));
                        }
                    }
                }
                for metric in LOWER_IS_BETTER {
                    if let (Some(c), Some(i)) = (candidate.get(*metric), inc.metrics.get(*metric)) {
                        if c > i {
                            regressions.push(format!("{metric} regressed vs incumbent: {c} > {i}"));
                        }
                    }
                }
            }
            let improved: Vec<String> = targets
                .relative_gates
                .must_improve_one_of
                .iter()
                .filter(|m| {
                    candidate.get(m.as_str()).copied().unwrap_or(0.0)
                        < inc.metrics.get(m.as_str()).copied().unwrap_or(0.0)
                })
                .cloned()
                .collect();
            failures.extend(regressions.iter().cloned());
            if improved.is_empty() {
                failures.push(
                    "candidate did not improve required latency/footprint metric against incumbent"
                        .into(),
                );
            }
            relative = Some(Relative {
                incumbent_model: inc.model.clone(),
                incumbent_backend: inc.backend.clone(),
                regressions,
                improved_metrics: improved,
            });
        }
    }
    if let Some(reason) = &relative_skipped {
        failures.push(format!("requested promotion unavailable: {reason}"));
    }
    Decision {
        targets: targets.id.clone(),
        hard_gates,
        relative,
        relative_skipped,
        absolute_passed,
        passed: failures.is_empty(),
        failures,
    }
}
