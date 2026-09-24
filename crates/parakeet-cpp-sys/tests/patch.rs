//! The ggml CPU patch rule over a planted source tree: a supplied tree is
//! never written to, an owned tree is patched once.

#[path = "../patch.rs"]
mod patch;

use std::path::Path;

use patch::{Outcome, cpu_patch, is_applied, prepare};

const PATCH: &str = "--- a/src/sgemm.c\n+++ b/src/sgemm.c\n@@ -1 +1 @@\n-broadcast\n+folded\n";

fn plant(root: &Path, line: &str) {
    std::fs::create_dir_all(root.join("third_party/ggml/src")).unwrap();
    std::fs::create_dir_all(root.join("third_party/ggml-patches")).unwrap();
    std::fs::write(
        root.join("third_party/ggml/src/sgemm.c"),
        format!("{line}\n"),
    )
    .unwrap();
    std::fs::write(
        root.join("third_party/ggml-patches/0001-ggml-cpu-fold.patch"),
        PATCH,
    )
    .unwrap();
}

fn have_patch_tool() -> bool {
    std::process::Command::new("patch")
        .arg("--version")
        .output()
        .is_ok()
}

#[test]
fn a_supplied_tree_is_checked_and_never_written() {
    if !have_patch_tool() {
        eprintln!("patch is not installed; skipping");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path();
    plant(source, "broadcast");
    let file = source.join("third_party/ggml/src/sgemm.c");
    let before = std::fs::read(&file).unwrap();
    let why = prepare(source, false).unwrap_err();
    assert!(
        why.contains("never writes into a caller's checkout"),
        "{why}"
    );
    assert!(why.contains("patch -p1 <"), "{why}");
    assert_eq!(
        std::fs::read(&file).unwrap(),
        before,
        "the supplied tree changed"
    );
    assert!(
        !source
            .join("third_party/ggml/.dettivo-cpu-patch-applied")
            .exists()
    );
    // A tree the caller patched, marker or not, is accepted as it is.
    std::fs::write(&file, "folded\n").unwrap();
    assert_eq!(prepare(source, false).unwrap(), Outcome::AlreadyApplied);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "folded\n");
    // Two builds sharing one supplied tree read it and race on nothing.
    assert_eq!(prepare(source, false).unwrap(), Outcome::AlreadyApplied);
}

#[test]
fn an_owned_tree_is_patched_once_and_a_tree_without_a_patch_builds_as_it_is() {
    if !have_patch_tool() {
        eprintln!("patch is not installed; skipping");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path();
    plant(source, "broadcast");
    let file = source.join("third_party/ggml/src/sgemm.c");
    let patch = cpu_patch(source).unwrap();
    assert!(!is_applied(&source.join("third_party/ggml"), &patch).unwrap());
    assert_eq!(prepare(source, true).unwrap(), Outcome::Applied);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "folded\n");
    assert!(is_applied(&source.join("third_party/ggml"), &patch).unwrap());
    assert_eq!(prepare(source, true).unwrap(), Outcome::AlreadyApplied);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "folded\n");
    let bare = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(bare.path().join("third_party/ggml")).unwrap();
    assert_eq!(prepare(bare.path(), true).unwrap(), Outcome::NoPatch);
    assert_eq!(prepare(bare.path(), false).unwrap(), Outcome::NoPatch);
}
