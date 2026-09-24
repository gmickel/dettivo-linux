//! The inherited macOS cases and the explicit Linux processor deltas.
//! Changed processor expectations retain `macos_expected` in the fixture
//! and follow enabled-only operations (ADR 0054).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use dettivo_language::polish::{
    PostProcessor, Style, Transform, add_at_prefix_to_file_paths, apply_post_processors, rewrite,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Suite {
    source: String,
    #[serde(default)]
    transforms: Vec<String>,
    #[serde(default)]
    post_processors: Vec<String>,
    #[serde(default)]
    style: Option<String>,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    name: String,
    input: String,
    #[serde(default)]
    expected: Option<String>,
    #[serde(default)]
    transforms: Option<Vec<String>>,
    #[serde(default)]
    post_processors: Option<Vec<String>>,
    #[serde(default)]
    style: Option<String>,
    #[serde(default)]
    contains: Vec<String>,
    #[serde(default)]
    not_contains: Vec<String>,
}

fn goldens_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens")
}

fn load(name: &str) -> Suite {
    let path = goldens_dir().join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn transform(name: &str) -> Transform {
    Transform::ALL
        .into_iter()
        .find(|t| t.as_str() == name)
        .unwrap_or_else(|| panic!("unknown transform {name}"))
}

fn processor(name: &str) -> PostProcessor {
    PostProcessor::ALL
        .into_iter()
        .find(|p| p.as_str() == name)
        .unwrap_or_else(|| panic!("unknown post-processor {name}"))
}

fn style(name: &str) -> Style {
    Style::parse(name).unwrap_or_else(|| panic!("unknown style {name}"))
}

fn run_case(suite: &Suite, case: &Case) -> String {
    let transforms: BTreeSet<Transform> = case
        .transforms
        .as_ref()
        .unwrap_or(&suite.transforms)
        .iter()
        .map(|t| transform(t))
        .collect();
    let processors: Vec<PostProcessor> = case
        .post_processors
        .as_ref()
        .unwrap_or(&suite.post_processors)
        .iter()
        .map(|p| processor(p))
        .collect();
    let chosen = case
        .style
        .as_deref()
        .or(suite.style.as_deref())
        .unwrap_or("asDictated");
    rewrite(&case.input, &transforms, &processors, style(chosen))
}

#[test]
fn the_deterministic_pass_matches_the_macos_cases() {
    let suite = load("polish_fallback.json");
    for case in &suite.cases {
        let got = run_case(&suite, case);
        let want = case.expected.as_deref().expect("an expected output");
        assert_eq!(
            got, want,
            "{} / {} ({})",
            suite.source, case.name, case.input
        );
    }
    assert!(suite.cases.len() >= 18, "the golden set must not shrink");
}

#[test]
fn the_post_processor_pipeline_matches_the_registered_linux_outputs() {
    let suite = load("post_processors.json");
    for case in &suite.cases {
        let processors: Vec<PostProcessor> = case
            .post_processors
            .as_ref()
            .unwrap_or(&suite.post_processors)
            .iter()
            .map(|p| processor(p))
            .collect();
        let got = apply_post_processors(&processors, &case.input);
        let want = case.expected.as_deref().expect("an expected output");
        assert_eq!(
            got, want,
            "{} / {} ({})",
            suite.source, case.name, case.input
        );
    }
    assert!(suite.cases.len() >= 37, "the golden set must not shrink");
}

#[test]
fn at_path_insertion_matches_the_macos_cases() {
    let suite = load("at_prefix.json");
    for case in &suite.cases {
        let got = add_at_prefix_to_file_paths(&case.input);
        let want = case.expected.as_deref().expect("an expected output");
        assert_eq!(
            got, want,
            "{} / {} ({})",
            suite.source, case.name, case.input
        );
        // Every case is idempotent: running the processor again changes
        // nothing, which is what keeps a re-polished insertion stable.
        assert_eq!(add_at_prefix_to_file_paths(&got), got, "{}", case.name);
    }
    assert!(suite.cases.len() >= 35, "the golden set must not shrink");
}

#[test]
fn the_warp_history_regressions_still_hold() {
    let suite = load("warp_regressions.json");
    for case in &suite.cases {
        let got = run_case(&suite, case);
        for needle in &case.contains {
            assert!(
                got.contains(needle.as_str()),
                "{} / {}: {got:?} lost {needle:?}",
                suite.source,
                case.name
            );
        }
        for needle in &case.not_contains {
            assert!(
                !got.contains(needle.as_str()),
                "{} / {}: {got:?} grew {needle:?}",
                suite.source,
                case.name
            );
        }
    }
    assert!(suite.cases.len() >= 6, "the golden set must not shrink");
}
