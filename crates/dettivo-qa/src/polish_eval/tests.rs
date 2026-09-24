//! The scorer and the gate against cases ported from
//! `score_results.py` and `check_promotion.py`: every macOS metric with
//! one passing and one failing row, the percentile and median the way
//! Python computes them, and the gate's three outcomes (absolute
//! failure, relative regression, no latency improvement) with the
//! Linux backend rule.

use std::collections::BTreeMap;

use super::dataset::Sample;
use super::gate::{self, Incumbent, Rule, Targets};
use super::scorer::{self, Runtime, score_row};

fn sample(id: &str) -> Sample {
    Sample {
        id: id.into(),
        split: "heldout".into(),
        input: "open src/app/index.ts and ping @cedric".into(),
        gold: Some("Open src/app/index.ts and ping @cedric.".into()),
        preset: "generic".into(),
        style: None,
        language: "en".into(),
        app_bundle_id: None,
        protected_spans: vec!["src/app/index.ts".into(), "@cedric".into()],
        required_fragments: vec!["ping".into()],
        forbidden_fragments: vec!["I have opened".into()],
        structure_fragments: vec![],
    }
}

fn runtime(output: &str, wall_ms: u64) -> Runtime {
    Runtime {
        output: output.into(),
        model: "qwen3-1.7b".into(),
        backend: "vulkan".into(),
        wall_ms,
        guard_rejection: None,
        fallback_used: false,
    }
}

#[test]
fn every_metric_has_a_passing_and_a_failing_row() {
    let s = sample("a");
    let pass = score_row(&s, &runtime("Open src/app/index.ts and ping @cedric.", 300));
    assert_eq!(pass.exact_match, Some(true));
    assert!(pass.protected_ok && pass.required_ok && pass.forbidden_ok);
    assert!(pass.structure_ok && pass.language_ok && pass.guard_ok && pass.fallback_ok);

    // protected: a path dropped.
    let f = score_row(&s, &runtime("Open the index file and ping @cedric.", 300));
    assert!(!f.protected_ok && f.exact_match == Some(false));
    // required: the verb gone.
    let f = score_row(&s, &runtime("Open src/app/index.ts, @cedric.", 300));
    assert!(!f.required_ok);
    // forbidden: the model answered instead of rewriting.
    let f = score_row(
        &s,
        &runtime("I have opened src/app/index.ts and ping @cedric.", 300),
    );
    assert!(!f.forbidden_ok);
    // structure: a list marker kept or lost.
    let mut listed = s.clone();
    listed.structure_fragments = vec!["- ".into()];
    assert!(score_row(&listed, &runtime("- ping @cedric src/app/index.ts", 1)).structure_ok);
    assert!(!score_row(&listed, &runtime("ping @cedric src/app/index.ts", 1)).structure_ok);
    // language anchors: the required fragments of a non-English row.
    let mut german = s.clone();
    german.language = "de".into();
    german.required_fragments = vec!["bitte".into()];
    assert!(score_row(&german, &runtime("bitte src/app/index.ts @cedric", 1)).language_ok);
    assert!(!score_row(&german, &runtime("please src/app/index.ts @cedric", 1)).language_ok);
    assert!(score_row(&s, &runtime("src/app/index.ts @cedric", 1)).language_ok);
    // guard and fallback flags come from the runtime.
    let mut rejected = runtime("Open src/app/index.ts and ping @cedric.", 300);
    rejected.guard_rejection = Some("protected token dropped".into());
    rejected.fallback_used = true;
    let f = score_row(&s, &rejected);
    assert!(!f.guard_ok && !f.fallback_ok);
    let mut empty = rejected.clone();
    empty.guard_rejection = Some(String::new());
    assert!(score_row(&s, &empty).guard_ok);
    // no gold: scored on the gates alone.
    let mut nogold = s.clone();
    nogold.gold = None;
    assert_eq!(score_row(&nogold, &runtime("x", 1)).exact_match, None);
}

#[test]
fn the_summary_computes_the_macos_rates_cuts_and_latencies() {
    let mut rows = Vec::new();
    let mut de = sample("de");
    de.language = "de".into();
    de.preset = "email".into();
    de.required_fragments = vec!["bitte".into()];
    de.gold = None;
    rows.push((
        sample("a"),
        runtime("Open src/app/index.ts and ping @cedric.", 100),
    ));
    rows.push((
        sample("b"),
        runtime("I have opened src/app/index.ts and ping @cedric.", 300),
    ));
    let mut fell = runtime("Open src/app/index.ts and ping @cedric.", 200);
    fell.fallback_used = true;
    fell.guard_rejection = Some("no-op".into());
    fell.model = "deterministic".into();
    rows.push((sample("c"), fell));
    rows.push((de, runtime("bitte src/app/index.ts @cedric", 900)));
    let s = scorer::summarize(&rows).unwrap();
    assert_eq!(s.count, 4);
    assert_eq!(s.gold_rows, 3);
    assert_eq!(s.model, "qwen3-1.7b");
    assert_eq!(s.backend, "vulkan");
    let m = &s.metrics;
    assert!((m.exact_match_rate - 2.0 / 3.0).abs() < 1e-9);
    assert_eq!(m.protected_token_preservation_rate, 1.0);
    assert_eq!(m.required_fragment_retention_rate, 1.0);
    assert_eq!(m.forbidden_fragment_violation_rate, 0.25);
    assert_eq!(m.structure_retention_rate, 1.0);
    assert_eq!(m.language_anchor_retention_rate, 1.0);
    assert_eq!(m.guard_rejection_rate, 0.25);
    assert_eq!(m.fallback_rate, 0.25);
    assert_eq!(m.median_wall_ms, 250);
    assert_eq!(m.p95_wall_ms, 900);
    assert_eq!(s.cuts["preset"]["generic"].count, 3);
    assert_eq!(s.cuts["preset"]["email"].count, 1);
    assert_eq!(s.cuts["language"]["de"].median_wall_ms, 900);
    assert_eq!(s.cuts["preset"]["generic"].fallback_rate, 1.0 / 3.0);
    assert!(scorer::summarize(&[]).is_none());
}

#[test]
fn percentile_and_median_match_python() {
    // round((n-1)*0.95) with n=10 is round(8.55) = 9: the last value.
    let ten: Vec<u64> = (1..=10).collect();
    assert_eq!(scorer::percentile(&ten, 0.95), 10);
    // n=20: round(18.05) = 18 -> the 19th value.
    let twenty: Vec<u64> = (1..=20).collect();
    assert_eq!(scorer::percentile(&twenty, 0.95), 19);
    assert_eq!(scorer::percentile(&[], 0.95), 0);
    assert_eq!(scorer::median(&[3, 1, 2]), 2);
    assert_eq!(scorer::median(&[4, 1, 2, 3]), 2);
    assert_eq!(scorer::median(&[]), 0);
}

fn targets() -> Targets {
    let mut hard = BTreeMap::new();
    hard.insert(
        "protected_token_preservation_rate".to_string(),
        Rule {
            minimum: Some(1.0),
            maximum: None,
        },
    );
    hard.insert(
        "fallback_rate".to_string(),
        Rule {
            minimum: None,
            maximum: Some(0.03),
        },
    );
    hard.insert(
        "p95_wall_ms".to_string(),
        Rule {
            minimum: None,
            maximum: Some(1400.0),
        },
    );
    Targets {
        id: "test-targets".into(),
        hard_gates: hard,
        relative_gates: gate::RelativeGates {
            must_match_or_beat_incumbent_hard_gates: true,
            must_improve_one_of: vec!["median_wall_ms".into(), "p95_wall_ms".into()],
        },
        rest: BTreeMap::new(),
    }
}

fn metrics(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

#[test]
fn the_gate_states_every_hard_gate_and_names_a_failure() {
    let good = metrics(&[
        ("protected_token_preservation_rate", 1.0),
        ("fallback_rate", 0.0),
        ("p95_wall_ms", 900.0),
        ("median_wall_ms", 400.0),
    ]);
    let d = gate::check(&targets(), &good, "vulkan", None);
    assert!(d.passed, "{d:?}");
    assert_eq!(d.hard_gates.len(), 3);
    assert!(d.hard_gates.iter().all(|g| g.pass));
    assert!(d.relative.is_none());

    let bad = metrics(&[
        ("protected_token_preservation_rate", 0.9),
        ("fallback_rate", 0.1),
        ("p95_wall_ms", 2000.0),
    ]);
    let d = gate::check(&targets(), &bad, "vulkan", None);
    assert!(!d.passed);
    assert_eq!(
        d.failures,
        [
            "fallback_rate above maximum: 0.1 > 0.03",
            "p95_wall_ms above maximum: 2000 > 1400",
            "protected_token_preservation_rate below minimum: 0.9 < 1",
        ]
    );
    let missing = metrics(&[("protected_token_preservation_rate", 1.0)]);
    let d = gate::check(&targets(), &missing, "vulkan", None);
    assert!(
        d.failures
            .iter()
            .any(|f| f == "fallback_rate missing from the candidate report")
    );
}

#[test]
fn the_relative_gates_follow_check_promotion() {
    let candidate = metrics(&[
        ("protected_token_preservation_rate", 1.0),
        ("fallback_rate", 0.0),
        ("p95_wall_ms", 900.0),
        ("median_wall_ms", 400.0),
        ("exact_match_rate", 0.8),
    ]);
    let inc = Incumbent {
        incompatible: None,
        model: "qwen3-4b-instruct-2507".into(),
        backend: "vulkan".into(),
        metrics: metrics(&[
            ("protected_token_preservation_rate", 1.0),
            ("fallback_rate", 0.0),
            ("p95_wall_ms", 1300.0),
            ("median_wall_ms", 700.0),
            ("exact_match_rate", 0.9),
        ]),
    };
    let d = gate::check(&targets(), &candidate, "vulkan", Some(&inc));
    let rel = d.relative.as_ref().unwrap();
    assert_eq!(
        rel.regressions,
        ["exact_match_rate regressed vs incumbent: 0.8 < 0.9"]
    );
    assert_eq!(rel.improved_metrics, ["median_wall_ms", "p95_wall_ms"]);
    assert!(!d.passed);

    // Faster but no better on quality passes; equal latency fails on the
    // improvement rule.
    let mut better = candidate.clone();
    better.insert("exact_match_rate".into(), 0.9);
    assert!(gate::check(&targets(), &better, "vulkan", Some(&inc)).passed);
    let mut same = better.clone();
    same.insert("p95_wall_ms".into(), 1300.0);
    same.insert("median_wall_ms".into(), 700.0);
    let d = gate::check(&targets(), &same, "vulkan", Some(&inc));
    assert_eq!(
        d.failures,
        ["candidate did not improve required latency/footprint metric against incumbent"]
    );

    // A different backend cannot establish the requested promotion.
    let d = gate::check(&targets(), &better, "cpu", Some(&inc));
    assert!(d.relative.is_none());
    assert!(d.relative_skipped.as_deref().unwrap().contains("vulkan"));
    assert!(d.absolute_passed);
    assert!(!d.passed);
}

#[test]
fn the_macos_targets_file_loads_with_its_extra_sections() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let t = Targets::load(&repo.join(super::TARGETS)).unwrap();
    assert_eq!(t.hard_gates.len(), 9);
    assert_eq!(
        t.hard_gates["protected_token_preservation_rate"].minimum,
        Some(1.0)
    );
    assert_eq!(t.hard_gates["p95_wall_ms"].maximum, Some(1400.0));
    assert!(t.relative_gates.must_match_or_beat_incumbent_hard_gates);
    assert!(t.rest.contains_key("confirmation"));
}

/// qa-packs/F14 (fn-43): a real candidate's checksum is taken while its
/// file exists; a file that cannot be hashed is an evidence failure for
/// a real candidate and nothing for a mock.
#[test]
fn a_real_candidate_needs_its_checksum_while_the_file_exists() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("model.gguf");
    std::fs::write(&file, b"gguf").unwrap();
    let hashed = super::model_checksum(true, Some(file.to_str().unwrap())).unwrap();
    assert_eq!(hashed.as_deref().map(str::len), Some(64));
    std::fs::remove_file(&file).unwrap();
    assert!(super::model_checksum(true, Some(file.to_str().unwrap())).is_err());
    assert!(super::model_checksum(true, None).is_err());
    assert_eq!(super::model_checksum(false, None).unwrap(), None);
}
