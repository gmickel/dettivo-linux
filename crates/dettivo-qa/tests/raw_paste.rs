//! The vocabulary bench scores what default raw dictation pastes: its
//! text-only daemon runs a guarded transcript through the raw layer, as
//! a take's text is run after the output guards (ADR 0096). Skips by
//! name when `dettivod` is not built (`just build-rust` first).

use std::path::{Path, PathBuf};

use dettivo_qa::polish_eval::runner::Runner;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

#[test]
fn the_bench_scores_the_text_raw_dictation_pastes() {
    let repo = repo();
    if !repo.join("target/debug/dettivod").is_file() {
        eprintln!("skip: target/debug/dettivod is not built (just build-rust)");
        return;
    }
    let paste = Runner::text_only(&repo, true).expect("text-only daemon");
    // The filed guard-library transcript: a spoken stop after "exam".
    assert_eq!(
        paste
            .raw("The library extended its opening hours so that students could study late during the exam period.")
            .unwrap(),
        "The library extended its opening hours so that students could study late during the exam."
    );
    assert_eq!(
        paste.raw("send me the report comma then go home").unwrap(),
        "Send me the report, then go home"
    );
    assert_eq!(paste.raw("  ").unwrap(), "");
}
