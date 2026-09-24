use super::*;
use std::os::unix::fs::PermissionsExt;

fn fixture(fail_second: bool) -> (tempfile::TempDir, Manifest) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    let binary = repo.join("build/qt/apps/fake/fake");
    std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
    std::fs::write(
        &binary,
        "#!/bin/sh\nif test \"$FAIL_RENDER\" = 1; then exit 3; fi\nprintf new > \"$2\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    let tool = repo.join("build/qt/tools/visual-diff/dettivo-visual-diff");
    std::fs::create_dir_all(tool.parent().unwrap()).unwrap();
    std::fs::write(tool, "unused").unwrap();
    let manifest = Manifest::parse(&format!(
        r#"
themes = ["builtin-dark"]
scales = [1]
[[surface]]
name = "test"
binary = "fake"
[[surface.state]]
name = "first"
[[surface.state]]
name = "second"
[surface.state.env]
FAIL_RENDER = "{}"
"#,
        u8::from(fail_second)
    ))
    .unwrap();
    let baseline = approved_path(repo, "test", "first", "builtin-dark", 1);
    std::fs::create_dir_all(baseline.parent().unwrap()).unwrap();
    std::fs::write(baseline, "original").unwrap();
    std::fs::write(repo.join(LOG_PATH), "original provenance\n").unwrap();
    (dir, manifest)
}

#[test]
fn failed_approval_preserves_all_baselines_and_provenance() {
    for failure in ["second-render", "provenance"] {
        let (dir, manifest) = fixture(failure == "second-render");
        let repo = dir.path();
        if failure == "provenance" {
            std::fs::remove_file(repo.join(LOG_PATH)).unwrap();
            std::fs::create_dir(repo.join(LOG_PATH)).unwrap();
        }
        let err = approve(
            repo,
            &manifest,
            &Filter::default(),
            &repo.join("work"),
            Some("test fixture"),
        )
        .unwrap_err();
        assert!(!err.is_empty());
        let original = approved_path(repo, "test", "first", "builtin-dark", 1);
        assert_eq!(
            std::fs::read_to_string(original).unwrap(),
            "original",
            "{failure}: {err}"
        );
        assert!(!approved_path(repo, "test", "second", "builtin-dark", 1).exists());
        if failure == "second-render" {
            assert_eq!(
                std::fs::read_to_string(repo.join(LOG_PATH)).unwrap(),
                "original provenance\n"
            );
        } else {
            assert!(repo.join(LOG_PATH).is_dir());
        }
    }
}

#[test]
fn successful_approval_publishes_the_whole_selection_with_provenance() {
    let (dir, manifest) = fixture(false);
    let repo = dir.path();
    let result = approve(
        repo,
        &manifest,
        &Filter::default(),
        &repo.join("work"),
        Some("test fixture"),
    )
    .unwrap();
    assert_eq!(result.len(), 2);
    assert!(result[0].replaced);
    assert!(!result[1].replaced);
    let log = std::fs::read_to_string(repo.join(LOG_PATH)).unwrap();
    assert!(log.starts_with("original provenance\n"));
    for row in result {
        assert_eq!(std::fs::read_to_string(&row.path).unwrap(), "new");
        assert!(log.contains(row.path.strip_prefix(repo).unwrap().to_str().unwrap()));
    }
}
