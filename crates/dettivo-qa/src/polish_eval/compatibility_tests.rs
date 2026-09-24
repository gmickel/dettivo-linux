use super::*;
use serde_json::json;

#[test]
fn requested_promotion_requires_comparable_reports() {
    let contract: Value =
        serde_json::from_str(include_str!("fixtures/promotion-contract.json")).unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = tempfile::tempdir().unwrap();
    let set = dir.path().join("set.jsonl");
    let sample = json!({"id":"one","split":"heldout","input":"Please send the report to the team tomorrow.","preset":"generic"});
    std::fs::write(&set, sample.to_string()).unwrap();
    let targets = dir.path().join("targets.json");
    std::fs::write(
        &targets,
        json!({
            "id":"test", "hard_gates":{}, "relative_gates":{
                "must_match_or_beat_incumbent_hard_gates":false,
                "must_improve_one_of":["median_wall_ms"]
            }
        })
        .to_string(),
    )
    .unwrap();
    let mut opts = Options {
        set: set.clone(),
        model: "echo".into(),
        split: "heldout".into(),
        incumbent: None,
        engine_only: false,
        out: None,
        golden: None,
        cpu: true,
        experiments_dir: None,
        models_dir: None,
        outputs: None,
        targets: Some(targets),
        baseline: Some(dir.path().join("no-baseline")),
        baseline_model: None,
        timeout_ms: 1000,
        engines_dir: None,
    };
    let first = run(&repo, &opts).unwrap().report;
    assert!(first.gate.passed);
    let mut incumbent = serde_json::to_value(first).unwrap();
    for (key, expected) in contract["absolute_gate"].as_object().unwrap() {
        assert_eq!(&incumbent["gate"][key], expected);
    }
    let fields: Vec<_> = incumbent["comparison"]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    assert_eq!(json!(fields), contract["comparison_fields"]);
    assert!(
        incumbent["comparison"]
            .as_object()
            .unwrap()
            .values()
            .all(|value| {
                value
                    .as_str()
                    .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            })
    );
    incumbent["metrics"]["median_wall_ms"] = json!(100_000);
    let inc_path = dir.path().join("incumbent.json");
    opts.incumbent = Some(inc_path.clone());
    std::fs::write(&inc_path, incumbent.to_string()).unwrap();
    assert!(
        run(&repo, &opts).unwrap().report.gate.passed,
        "matching reports must promote"
    );

    let mut incorrectly_passed = Vec::new();
    for expected in contract["incompatible"].as_array().unwrap() {
        let case = expected["case"].as_str().unwrap();
        let mut inc = incumbent.clone();
        let mut candidate = opts.clone();
        let mut row = sample.clone();
        match case {
            "content" => {
                row["input"] =
                    json!("A completely unrelated evaluation input on the same named set.")
            }
            "split" => candidate.split = "all".into(),
            "rows" => inc["row_ids"] = json!(["another"]),
            "scoring" => inc["comparison"]["scorer_sha256"] = json!("other scorer"),
            "targets" => inc["comparison"]["targets_sha256"] = json!("other targets"),
            "execution" => inc["comparison"]["execution_sha256"] = json!("other runtime"),
            "legacy" => {
                inc.as_object_mut().unwrap().remove("comparison");
            }
            "backend" => inc["backend"] = json!("unrelated-backend"),
            _ => unreachable!(),
        }
        let other = dir.path().join(case).join("set.jsonl");
        std::fs::create_dir_all(other.parent().unwrap()).unwrap();
        std::fs::write(&other, row.to_string()).unwrap();
        candidate.set = other;
        std::fs::write(&inc_path, inc.to_string()).unwrap();
        let gate = run(&repo, &candidate).unwrap().report.gate;
        if gate.passed {
            incorrectly_passed.push(case);
        } else {
            assert!(gate.relative.is_none(), "{case}: {gate:?}");
            assert!(gate.relative_skipped.is_some(), "{case}: {gate:?}");
            assert!(gate.hard_gates.iter().all(|g| g.pass));
            assert_eq!(json!(gate.relative_skipped), expected["reason"]);
            let wire = serde_json::to_value(&gate).unwrap();
            for (key, value) in contract["unavailable_gate"].as_object().unwrap() {
                assert_eq!(&wire[key], value, "{case}");
            }
        }
    }
    assert!(
        incorrectly_passed.is_empty(),
        "unavailable promotion passed for {incorrectly_passed:?}"
    );
}
