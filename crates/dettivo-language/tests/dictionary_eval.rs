//! The dictionary-correction eval (fn-30 R4): the twenty public rows of the
//! macOS `scripts/dictionary_correction/eval-v1.jsonl`, in their shape,
//! through `dettivo_language::raw` with the row's dictionary as
//! whole-word replacements (a word maps to its own spelling, which is what
//! the macOS evaluator's `exact` strategy does), scored the way the macOS
//! evaluator scores a strategy: exact match, positive recall, negative
//! safety, ambiguous abstention and the false-positive count. The report
//! under `docs/reports/polish-eval/dictionary-eval-v1.json` is the golden:
//! every figure and every row's outcome must match it, and a row whose
//! correction differs from the macOS `exact` output is listed there by
//! name, so the gap between the two raw layers is a recorded fact rather
//! than a surprise.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One row of the macOS set, plus the `exact` strategy's output.
#[derive(Debug, Deserialize)]
struct Row {
    id: String,
    kind: String,
    dictionary: Vec<String>,
    input: String,
    expected: String,
    macos_exact_output: String,
}

/// The figures the macOS `summarize` computes for one strategy, minus the
/// timings, which describe a machine.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Figures {
    count: usize,
    exact_match_rate: f64,
    positive_recall: f64,
    negative_safety: f64,
    ambiguous_abstention: f64,
    false_positive_count: usize,
}

/// The report file.
#[derive(Debug, Serialize, Deserialize)]
struct Report {
    set: String,
    layer: String,
    linux: Figures,
    macos_exact: Figures,
    macos_hybrid: Figures,
    rows_differing_from_macos_exact: Vec<String>,
    rows_failing_expected: Vec<String>,
    note: String,
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn rows() -> Vec<Row> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/dictionary_eval_v1.jsonl");
    std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// The raw layer with the dictionary as replacements, spoken punctuation
/// off (the macOS evaluator runs on the transcript alone).
fn correct(row: &Row) -> String {
    let replacements: Vec<(String, String)> = row
        .dictionary
        .iter()
        .map(|w| (w.clone(), w.clone()))
        .collect();
    dettivo_language::raw::apply(&row.input, &replacements, false, true)
}

fn rate(hits: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        hits as f64 / total as f64
    }
}

fn figures(outcomes: &[(&Row, bool)]) -> Figures {
    let of = |kind: &str| -> (usize, usize) {
        let rows: Vec<bool> = outcomes
            .iter()
            .filter(|(r, _)| r.kind == kind)
            .map(|(_, ok)| *ok)
            .collect();
        (rows.iter().filter(|ok| **ok).count(), rows.len())
    };
    let (pos_ok, pos) = of("positive");
    let (neg_ok, neg) = of("negative");
    let (amb_ok, amb) = of("ambiguous");
    Figures {
        count: outcomes.len(),
        exact_match_rate: rate(
            outcomes.iter().filter(|(_, ok)| *ok).count(),
            outcomes.len(),
        ),
        positive_recall: rate(pos_ok, pos),
        negative_safety: rate(neg_ok, neg),
        ambiguous_abstention: rate(amb_ok, amb),
        false_positive_count: (neg - neg_ok) + (amb - amb_ok),
    }
}

#[test]
fn the_raw_layer_scores_the_macos_dictionary_set_as_the_report_records() {
    let rows = rows();
    assert_eq!(rows.len(), 20);
    // The raw layer capitalises the first letter of a transcript; the
    // macOS evaluator does not, so the comparison lowers it back for the
    // rows whose expected text starts lowercase.
    let outputs: Vec<(String, String)> = rows
        .iter()
        .map(|r| {
            let out = correct(r);
            let out = match (r.input.chars().next(), out.chars().next()) {
                (Some(i), Some(o)) if i.is_lowercase() && o.is_uppercase() => {
                    i.to_string() + &out[o.len_utf8()..]
                }
                _ => out,
            };
            (out, r.macos_exact_output.clone())
        })
        .collect();
    let linux: Vec<(&Row, bool)> = rows
        .iter()
        .zip(&outputs)
        .map(|(r, (out, _))| (r, out == &r.expected))
        .collect();
    let macos: Vec<(&Row, bool)> = rows
        .iter()
        .map(|r| (r, r.macos_exact_output == r.expected))
        .collect();
    let differing: Vec<String> = rows
        .iter()
        .zip(&outputs)
        .filter(|(_, (out, mac))| out != mac)
        .map(|(r, _)| r.id.clone())
        .collect();
    let failing: Vec<String> = linux
        .iter()
        .filter(|(_, ok)| !ok)
        .map(|(r, _)| r.id.clone())
        .collect();
    let computed = Report {
        set: "scripts/dictionary_correction/eval-v1.jsonl (macOS)".into(),
        layer: "dettivo-language::raw with the dictionary as whole-word replacements".into(),
        linux: figures(&linux),
        macos_exact: figures(&macos),
        macos_hybrid: Figures {
            count: 20,
            exact_match_rate: 1.0,
            positive_recall: 1.0,
            negative_safety: 1.0,
            ambiguous_abstention: 1.0,
            false_positive_count: 0,
        },
        rows_differing_from_macos_exact: differing,
        rows_failing_expected: failing,
        note: String::new(),
    };
    let path = repo().join("docs/reports/polish-eval/dictionary-eval-v1.json");
    let recorded: Report = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        recorded.linux,
        computed.linux,
        "the Linux figures moved; regenerate {}",
        path.display()
    );
    assert_eq!(recorded.macos_exact, computed.macos_exact);
    assert_eq!(
        recorded.rows_differing_from_macos_exact, computed.rows_differing_from_macos_exact,
        "a row's correction differs from what the report records"
    );
    assert_eq!(
        recorded.rows_failing_expected,
        computed.rows_failing_expected
    );
    // The report is text-free: ids and figures only.
    let value: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for key in ["input", "output", "expected", "dictionary"] {
        assert!(value.get(key).is_none(), "{key} in the report");
    }
}
