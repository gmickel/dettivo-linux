//! The macOS scorer (`scripts/polish_finetune/score_results.py`) ported
//! case for case: one flag set per row, the ten summary metrics with
//! their macOS names, and the cuts by preset and by language. A row
//! without a gold rewrite is scored on the gates alone and left out of
//! the exact-match rate, which is what the macOS bench does with a
//! missing `gold`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::dataset::Sample;

/// What the product did with one row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Runtime {
    /// The text Enhanced would have inserted.
    pub output: String,
    /// The model that answered, or `deterministic`.
    pub model: String,
    /// The engine backend (`vulkan`, `cpu`, `mock`).
    pub backend: String,
    /// The whole `polish.test` round trip.
    pub wall_ms: u64,
    /// The guard's reason when every rewrite was rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard_rejection: Option<String>,
    /// The deterministic text went in instead of a rewrite.
    #[serde(default)]
    pub fallback_used: bool,
}

/// One row's flags, the macOS `score_row`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flags {
    /// The output equals the gold rewrite; `None` without a gold.
    pub exact_match: Option<bool>,
    /// Every protected span survived.
    pub protected_ok: bool,
    /// Every required fragment survived.
    pub required_ok: bool,
    /// No forbidden fragment appeared.
    pub forbidden_ok: bool,
    /// Every structure fragment survived.
    pub structure_ok: bool,
    /// The language anchors (the required fragments of a non-English
    /// row) survived.
    pub language_ok: bool,
    /// The guard accepted a rewrite.
    pub guard_ok: bool,
    /// A rewrite went in rather than the deterministic text.
    pub fallback_ok: bool,
}

fn contains_all(text: &str, fragments: &[String]) -> bool {
    fragments.iter().all(|f| text.contains(f.as_str()))
}

fn contains_none(text: &str, fragments: &[String]) -> bool {
    fragments.iter().all(|f| !text.contains(f.as_str()))
}

/// Scores one row.
pub fn score_row(sample: &Sample, runtime: &Runtime) -> Flags {
    let out = runtime.output.as_str();
    let anchors: &[String] = if sample.language != "en" {
        &sample.required_fragments
    } else {
        &[]
    };
    Flags {
        exact_match: sample.gold.as_deref().map(|g| out == g),
        protected_ok: contains_all(out, &sample.protected_spans),
        required_ok: contains_all(out, &sample.required_fragments),
        forbidden_ok: contains_none(out, &sample.forbidden_fragments),
        structure_ok: contains_all(out, &sample.structure_fragments),
        language_ok: contains_all(out, anchors),
        guard_ok: runtime
            .guard_rejection
            .as_deref()
            .is_none_or(|r| r.is_empty()),
        fallback_ok: !runtime.fallback_used,
    }
}

/// The summary metrics, named as the macOS summary names them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    /// Outputs equal to the gold, over the rows that have one.
    pub exact_match_rate: f64,
    /// Rows that kept every protected span.
    pub protected_token_preservation_rate: f64,
    /// Rows that kept every required fragment.
    pub required_fragment_retention_rate: f64,
    /// Rows where a forbidden fragment appeared.
    pub forbidden_fragment_violation_rate: f64,
    /// Rows that kept every structure fragment.
    pub structure_retention_rate: f64,
    /// Rows that kept their language anchors.
    pub language_anchor_retention_rate: f64,
    /// Rows whose every rewrite the guard rejected.
    pub guard_rejection_rate: f64,
    /// Rows where the deterministic text went in.
    pub fallback_rate: f64,
    /// The median round trip.
    pub median_wall_ms: u64,
    /// The 95th percentile round trip (the macOS nearest-rank index).
    pub p95_wall_ms: u64,
}

impl Metrics {
    /// The metrics as a name-to-value map, the shape the gate reads.
    pub fn as_map(&self) -> BTreeMap<String, f64> {
        let v = serde_json::to_value(self).unwrap_or_default();
        v.as_object()
            .map(|o| {
                o.iter()
                    .filter_map(|(k, v)| v.as_f64().map(|f| (k.clone(), f)))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// One cut of the rows (a preset, a language).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cut {
    /// Rows in the cut.
    pub count: usize,
    /// Exact matches over the cut's rows with a gold.
    pub exact_match_rate: f64,
    /// Protected spans kept over the cut.
    pub protected_token_preservation_rate: f64,
    /// Deterministic fallbacks over the cut.
    pub fallback_rate: f64,
    /// The cut's median round trip.
    pub median_wall_ms: u64,
}

/// The summary: metrics, the cuts, the model and backend most rows
/// reported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// Rows scored.
    pub count: usize,
    /// Rows with a gold rewrite.
    pub gold_rows: usize,
    /// The model most rows named.
    pub model: String,
    /// The backend most rows ran on.
    pub backend: String,
    /// The ten metrics.
    pub metrics: Metrics,
    /// `preset` and `language` cuts.
    pub cuts: BTreeMap<String, BTreeMap<String, Cut>>,
}

/// The macOS percentile: the value at `round((n - 1) * pct)` of the
/// sorted list.
pub fn percentile(values: &[u64], pct: f64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() - 1) as f64 * pct).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

/// The macOS `statistics.median`, as an integer the way the summary
/// stores it.
pub fn median(values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2
    }
}

fn rate(flags: &[Flags], pick: impl Fn(&Flags) -> bool) -> f64 {
    if flags.is_empty() {
        return 0.0;
    }
    flags.iter().filter(|f| pick(f)).count() as f64 / flags.len() as f64
}

fn exact_rate(flags: &[Flags]) -> f64 {
    let with_gold: Vec<bool> = flags.iter().filter_map(|f| f.exact_match).collect();
    if with_gold.is_empty() {
        return 0.0;
    }
    with_gold.iter().filter(|m| **m).count() as f64 / with_gold.len() as f64
}

fn most_common<'a>(names: impl Iterator<Item = &'a str>) -> String {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for n in names {
        *counts.entry(n).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(n, _)| n.to_string())
        .unwrap_or_default()
}

fn cut(rows: &[(&Sample, &Runtime)]) -> Cut {
    let flags: Vec<Flags> = rows.iter().map(|(s, r)| score_row(s, r)).collect();
    let latencies: Vec<u64> = rows.iter().map(|(_, r)| r.wall_ms).collect();
    Cut {
        count: rows.len(),
        exact_match_rate: exact_rate(&flags),
        protected_token_preservation_rate: rate(&flags, |f| f.protected_ok),
        fallback_rate: rate(&flags, |f| !f.fallback_ok),
        median_wall_ms: median(&latencies),
    }
}

/// Summarises every scored row; `None` for no rows.
pub fn summarize(rows: &[(Sample, Runtime)]) -> Option<Summary> {
    if rows.is_empty() {
        return None;
    }
    let flags: Vec<Flags> = rows.iter().map(|(s, r)| score_row(s, r)).collect();
    let latencies: Vec<u64> = rows.iter().map(|(_, r)| r.wall_ms).collect();
    let mut cuts = BTreeMap::new();
    for (name, key) in [
        (
            "preset",
            (|s: &Sample| s.preset.clone()) as fn(&Sample) -> String,
        ),
        ("language", |s: &Sample| s.language.clone()),
    ] {
        let mut grouped: BTreeMap<String, Vec<(&Sample, &Runtime)>> = BTreeMap::new();
        for (s, r) in rows {
            grouped.entry(key(s)).or_default().push((s, r));
        }
        cuts.insert(
            name.to_string(),
            grouped
                .into_iter()
                .map(|(k, members)| (k, cut(&members)))
                .collect(),
        );
    }
    Some(Summary {
        count: rows.len(),
        gold_rows: rows.iter().filter(|(s, _)| s.gold.is_some()).count(),
        model: most_common(rows.iter().map(|(_, r)| r.model.as_str())),
        backend: most_common(rows.iter().map(|(_, r)| r.backend.as_str())),
        metrics: Metrics {
            exact_match_rate: exact_rate(&flags),
            protected_token_preservation_rate: rate(&flags, |f| f.protected_ok),
            required_fragment_retention_rate: rate(&flags, |f| f.required_ok),
            forbidden_fragment_violation_rate: 1.0 - rate(&flags, |f| f.forbidden_ok),
            structure_retention_rate: rate(&flags, |f| f.structure_ok),
            language_anchor_retention_rate: rate(&flags, |f| f.language_ok),
            guard_rejection_rate: 1.0 - rate(&flags, |f| f.guard_ok),
            fallback_rate: 1.0 - rate(&flags, |f| f.fallback_ok),
            median_wall_ms: median(&latencies),
            p95_wall_ms: percentile(&latencies, 0.95),
        },
        cuts,
    })
}
