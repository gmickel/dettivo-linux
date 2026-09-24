//! The evidence map's tests: a fixture repository with one spec, and
//! the checked-in map against this repository.

use super::resolve::Resolver;
use super::*;

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".flow/specs")).unwrap();
    std::fs::create_dir_all(root.join("qa")).unwrap();
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::create_dir_all(root.join("crates/thing/src")).unwrap();
    std::fs::write(
        root.join(".flow/specs/fn-1-thing.md"),
        "# Thing\n\n## Goal\n\n- **R9:** not a criterion\n\n## Acceptance Criteria\n\n- **R1:** one. [user]\n- **R2:** two. [inferred]\n\n## Boundaries\n",
    )
    .unwrap();
    std::fs::write(root.join("docs/thing.md"), "# Thing\n").unwrap();
    std::fs::write(
        root.join("crates/thing/src/lib.rs"),
        "#[test]\nfn it_works() {}\n",
    )
    .unwrap();
    dir
}

#[test]
fn the_rids_come_from_the_acceptance_criteria_only() {
    let text = std::fs::read_to_string(repo().path().join(".flow/specs/fn-1-thing.md")).unwrap();
    assert_eq!(rids_of(&text), ["R1", "R2"]);
    assert_eq!(
        rids_of(
            "## Acceptance criteria\n- **R1**: one\n- **R12**: twelve\n## Next\n- **R3**: no\n"
        ),
        ["R1", "R12"]
    );
    assert_eq!(spec_number("fn-12-x"), 12);
    assert_eq!(spec_number("other"), u32::MAX);
}

#[test]
fn full_coverage_passes_and_an_unmapped_rid_fails_by_name() {
    let dir = repo();
    let root = dir.path();
    std::fs::write(
        root.join(MAP),
        "[[spec]]\nid = \"fn-1-thing\"\n[[spec.route]]\nrids = [\"R1\", \"R2\"]\nkind = \"unit\"\nref = \"thing::it_works\"\nnote = \"n\"\n",
    )
    .unwrap();
    let report = build(root).unwrap();
    assert!(report.ok(), "{:?}", report.totals.findings);
    assert_eq!(report.totals.coverage, 1.0);
    assert_eq!(report.specs[0].mapped, ["R1", "R2"]);

    std::fs::write(
        root.join(MAP),
        "[[spec]]\nid = \"fn-1-thing\"\n[[spec.route]]\nrids = [\"R1\", \"R3\"]\nkind = \"docs\"\nref = \"docs/thing.md\"\n",
    )
    .unwrap();
    let report = build(root).unwrap();
    assert!(!report.ok());
    assert_eq!(report.specs[0].unmapped, ["R2"]);
    assert_eq!(report.specs[0].unknown, ["R3"]);
    assert_eq!(report.totals.coverage, 0.5);
    let text = report.totals.findings.join("\n");
    assert!(
        text.contains("fn-1-thing: R2 has no evidence route"),
        "{text}"
    );
    assert!(text.contains("the map names R3"), "{text}");
    assert_eq!(command(root, true, false), 1);
}

#[test]
fn an_unresolvable_ref_names_the_kind_and_the_reference() {
    let dir = repo();
    let root = dir.path();
    std::fs::write(
        root.join(MAP),
        "[[spec]]\nid = \"fn-1-thing\"\n[[spec.route]]\nrids = [\"R1\", \"R2\"]\nkind = \"unit\"\nref = \"thing::nope\"\n[[spec.route]]\nrids = [\"R2\"]\nkind = \"human\"\nref = \"docs/receipt.md\"\n",
    )
    .unwrap();
    let report = build(root).unwrap();
    let text = report.totals.findings.join("\n");
    assert!(
        text.contains("fn-1-thing: unit ref \"thing::nope\" does not resolve"),
        "{text}"
    );
    assert!(text.contains("human ref \"docs/receipt.md\""), "{text}");
    assert_eq!(report.totals.human_routes, 1);
}

#[test]
fn a_unit_ref_needs_a_test_attribute_and_a_visual_ref_needs_a_real_state() {
    let dir = repo();
    let root = dir.path();
    std::fs::write(
        root.join("crates/thing/src/lib.rs"),
        "pub fn helper() {}\n// fn commented_out() {}\n#[cfg(test)]\nmod tests {\n    #[tokio::test]\n    async fn it_waits() {}\n\n    /// Documented.\n    #[test]\n    fn it_works() {}\n}\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("qa/visual")).unwrap();
    std::fs::write(
        root.join("qa/visual/manifest.toml"),
        "[[surface]]\nname = \"osd\"\n[[surface.state]]\nname = \"listening\"\n[[surface.state]]\nname = \"error\"\n[[surface]]\nname = \"home\"\n",
    )
    .unwrap();
    let r = Resolver::new(root).unwrap();
    assert!(r.resolve("unit", "thing::it_works").is_ok());
    assert!(r.resolve("unit", "thing::it_waits").is_ok());
    let why = r.resolve("unit", "thing::helper").unwrap_err();
    assert!(why.contains("not a test"), "{why}");
    let why = r.resolve("unit", "thing::commented_out").unwrap_err();
    assert!(why.starts_with("no `fn commented_out`"), "{why}");
    assert!(r.resolve("visual", "osd").is_ok());
    assert!(r.resolve("visual", "osd/error").is_ok());
    let why = r.resolve("visual", "osd/asleep").unwrap_err();
    assert!(
        why.contains("no state \"asleep\"") && why.contains("listening, error"),
        "{why}"
    );
    assert!(r.resolve("visual", "home/any").is_err());
    std::fs::write(root.join("docs/receipt.md"), "walked").unwrap();
    let found = r.resolve("human", "docs/receipt.md").unwrap();
    assert!(found.contains("not proof"), "{found}");
    assert!(r.resolve("bench", "startup").is_ok());
    assert!(r.resolve("bench", "meeting_throughput").is_ok());
    assert!(
        r.resolve("bench", "diarization").is_err(),
        "the placeholder is gone"
    );
    assert!(r.resolve("pipeline", "diarization").is_ok());
    assert!(
        Map::parse(
            "[[spec]]\nid = \"x\"\n[[spec.route]]\nrids = [\"R1\"]\nkind = \"magic\"\nref = \"y\"\n"
        )
        .is_err()
    );
}

#[test]
fn the_checked_in_map_covers_every_rid_of_every_spec() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let report = build(&root).unwrap();
    assert!(
        report.ok(),
        "qa/evidence-map.toml:\n{}",
        report.totals.findings.join("\n")
    );
    assert_eq!(report.totals.coverage, 1.0);
}
