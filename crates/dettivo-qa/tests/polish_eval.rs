//! fn-30 R2 and R3 in CI: the sample set through a daemon of the harness's
//! own with the recorded rewrites produces the golden metrics, a row with
//! an unknown preset fails naming the row, a report never carries a
//! row's text, and the checked-in reports under `docs/reports/polish-eval`
//! hold to that. The daemon-backed cases skip by name when `dettivod` is
//! not built (`just build-rust` first).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dettivo_qa::polish_eval::{self, Options};
use serde_json::Value;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn daemon_built(repo: &Path) -> bool {
    let built = repo.join("target/debug/dettivod").is_file();
    if !built {
        eprintln!("skip: target/debug/dettivod is not built (just build-rust)");
    }
    built
}

fn fixtures(repo: &Path) -> PathBuf {
    repo.join("crates/dettivo-qa/fixtures/polish-eval")
}

fn options(repo: &Path, set: PathBuf, out: Option<PathBuf>) -> Options {
    Options {
        set,
        model: format!("fixture:{}", fixtures(repo).join("rewrites").display()),
        split: "heldout".into(),
        incumbent: None,
        engine_only: false,
        out,
        golden: Some(fixtures(repo).join("golden.json")),
        cpu: true,
        experiments_dir: None,
        models_dir: None,
        outputs: None,
        targets: None,
        baseline: None,
        baseline_model: None,
        timeout_ms: 10_000,
        engines_dir: None,
    }
}

/// Every key at any depth of a JSON value.
fn keys(value: &Value, into: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                into.push(k.clone());
                keys(v, into);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| keys(v, into)),
        _ => {}
    }
}

/// The keys a report may never carry: anything that holds a row's text.
const TEXT_KEYS: &[&str] = &[
    "input", "output", "gold", "expected", "polished", "enhanced", "raw",
];

#[test]
fn the_sample_set_with_the_recorded_rewrites_matches_the_golden() {
    let repo = repo();
    if !daemon_built(&repo) {
        return;
    }
    let out = tempfile::Builder::new()
        .prefix("dtv-polish-eval")
        .tempdir_in("/tmp")
        .unwrap();
    let outcome = polish_eval::run(
        &repo,
        &options(
            &repo,
            fixtures(&repo).join("sample.jsonl"),
            Some(out.path().to_path_buf()),
        ),
    )
    .unwrap();
    assert!(
        outcome.golden_mismatches.is_empty(),
        "{:?}",
        outcome.golden_mismatches
    );
    let report = &outcome.report;
    assert_eq!(report.rows, 12);
    assert_eq!(report.gold_rows, 11);
    assert_eq!(report.backend, "mock");
    assert_eq!(report.model, "mock-fixture");
    assert_eq!(report.row_ids.len(), 12);
    assert_eq!(report.row_ids[0], "sample-001");
    assert_eq!(report.gate.hard_gates.len(), 9);
    // The staged failures: the guard rejected one rewrite, one carried a
    // forbidden fragment, one row has no gold.
    assert!((report.metrics.guard_rejection_rate - 1.0 / 12.0).abs() < 1e-9);
    assert!((report.metrics.forbidden_fragment_violation_rate - 1.0 / 12.0).abs() < 1e-9);
    assert!(!report.gate.passed);

    // The written report carries no text and the README names it.
    let path = outcome.path.clone().unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let value: Value = serde_json::from_str(&text).unwrap();
    let mut found = Vec::new();
    keys(&value, &mut found);
    for k in TEXT_KEYS {
        assert!(!found.iter().any(|f| f == k), "{k} in {}", path.display());
    }
    assert!(!text.contains("ping @cedric"), "row text in the report");
    let readme = std::fs::read_to_string(out.path().join("README.md")).unwrap();
    assert!(readme.contains("mock-fixture"), "{readme}");
    assert!(readme.contains("sample.jsonl"), "{readme}");

    // A golden that disagrees names the metric.
    let wrong = out.path().join("wrong.json");
    std::fs::write(&wrong, r#"{"metrics": {"guard_rejection_rate": 0.0}}"#).unwrap();
    let mismatches = polish_eval::golden_mismatches(&wrong, &report.metrics.as_map()).unwrap();
    assert_eq!(mismatches.len(), 1, "{mismatches:?}");
    assert!(
        mismatches[0].starts_with("guard_rejection_rate: got 0.0833"),
        "{mismatches:?}"
    );
}

#[test]
fn a_row_with_an_unknown_preset_fails_naming_the_row() {
    let repo = repo();
    if !daemon_built(&repo) {
        return;
    }
    let dir = tempfile::Builder::new()
        .prefix("dtv-polish-set")
        .tempdir_in("/tmp")
        .unwrap();
    let set = dir.path().join("bad.jsonl");
    std::fs::write(
        &set,
        "{\"id\":\"bad-001\",\"input\":\"hello there\",\"preset\":\"haiku\"}\n",
    )
    .unwrap();
    let err = polish_eval::run(&repo, &options(&repo, set, None)).unwrap_err();
    assert!(err.starts_with("row bad-001:"), "{err}");
    assert!(err.contains("haiku"), "{err}");
}

#[test]
fn the_checked_in_reports_carry_metrics_and_row_ids_only() {
    let dir = repo().join(polish_eval::REPORTS_DIR);
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        let mut found = Vec::new();
        keys(&value, &mut found);
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let is_run = name != polish_eval::BASELINE && !name.starts_with("dictionary-eval");
        for k in TEXT_KEYS {
            assert!(!found.iter().any(|f| f == k), "{name} carries a {k} key");
        }
        if is_run {
            let report: polish_eval::report::Report = serde_json::from_value(value).unwrap();
            assert_eq!(report.row_ids.len(), report.rows, "{name}");
            assert!(
                !report.model_sha256.as_deref().unwrap_or("x").is_empty(),
                "{name}"
            );
            let metrics: BTreeMap<String, f64> = report.metrics.as_map();
            assert_eq!(metrics.len(), 10, "{name}");
            seen += 1;
        }
    }
    assert!(seen >= 1, "no run report under {}", dir.display());
}
