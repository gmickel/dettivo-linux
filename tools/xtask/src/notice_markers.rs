//! The reuse rule of `lint-notice` (ADR 0041, the Voxtype reuse review):
//! every source span taken from another project carries the marker
//! `Reused from Voxtype (MIT), see NOTICE.md` in a comment, and the
//! reuse table of `NOTICE.md` (between its `reuse` markers) has one row
//! per marked file with the upstream file, commit and licence. A marker
//! without a row and a row without a marker both fail, by path.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use crate::ToolError;
use crate::notice::NOTICE;

const BEGIN: &str = "<!-- reuse:begin -->";
const END: &str = "<!-- reuse:end -->";

/// The marker a reused span carries (assembled so this file never
/// matches its own rule).
pub fn marker() -> String {
    ["Reused from", "Voxtype (MIT), see", "NOTICE.md"].join(" ")
}

/// The files the marker rule scans: every tracked file except the
/// notice itself, the review that quotes the marker, and this lint.
pub fn tracked_sources(root: &Path) -> Result<Vec<String>, ToolError> {
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(ToolError("git ls-files failed".into()));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter(|p| !p.is_empty())
        .filter(|p| !is_exempt(p))
        .map(str::to_string)
        .collect())
}

fn is_exempt(path: &str) -> bool {
    path == NOTICE
        || path.starts_with("docs/reports/")
        || path.starts_with("docs/adr/")
        || path.starts_with(".flow/")
        || path == "tools/xtask/src/notice_markers.rs"
}

/// The files that carry the marker.
pub fn marked_files(root: &Path, files: &[String]) -> BTreeSet<String> {
    let needle = marker();
    files
        .iter()
        .filter(|f| {
            std::fs::read_to_string(root.join(f))
                .map(|t| t.contains(&needle))
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

/// The file paths the notice's reuse table lists (the first column).
pub fn listed_files(notice: &str) -> Result<BTreeSet<String>, ToolError> {
    let start = notice
        .find(BEGIN)
        .ok_or_else(|| ToolError(format!("{NOTICE}: missing the {BEGIN} marker")))?;
    let end = notice
        .find(END)
        .ok_or_else(|| ToolError(format!("{NOTICE}: missing the {END} marker")))?;
    if end < start {
        return Err(ToolError(format!("{NOTICE}: {END} comes before {BEGIN}")));
    }
    Ok(notice[start + BEGIN.len()..end]
        .lines()
        .filter_map(|l| l.trim().strip_prefix("| `"))
        .filter_map(|l| l.split('`').next())
        .map(str::to_string)
        .collect())
}

/// One line per marked file without a row and per row without a marker.
pub fn violations(marked: &BTreeSet<String>, listed: &BTreeSet<String>) -> Vec<String> {
    let mut found = Vec::new();
    for file in marked.difference(listed) {
        found.push(format!(
            "{file}: carries the reuse marker but {NOTICE} has no row for it in the reuse table"
        ));
    }
    for file in listed.difference(marked) {
        found.push(format!(
            "{NOTICE}: the reuse table lists `{file}` but the file carries no `{}` marker (or does not exist)",
            marker()
        ));
    }
    found
}

/// Runs the rule against `root`.
pub fn run(root: &Path, notice: &str) -> Result<Vec<String>, ToolError> {
    let files = tracked_sources(root)?;
    let marked = marked_files(root, &files);
    let listed = listed_files(notice)?;
    Ok(violations(&marked, &listed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notice(rows: &str) -> String {
        format!(
            "# Notice\n\n{BEGIN}\n| File | Upstream | Commit | License |\n|---|---|---|---|\n{rows}{END}\n"
        )
    }

    #[test]
    fn a_marker_without_a_row_and_a_row_without_a_marker_fail_by_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("crates/x/src")).unwrap();
        std::fs::write(
            root.join("crates/x/src/planted.rs"),
            format!("// {}\nfn f() {{}}\n", marker()),
        )
        .unwrap();
        std::fs::write(root.join("crates/x/src/clean.rs"), "fn g() {}\n").unwrap();
        let files = vec![
            "crates/x/src/planted.rs".to_string(),
            "crates/x/src/clean.rs".to_string(),
        ];
        let marked = marked_files(root, &files);
        assert_eq!(
            marked.iter().collect::<Vec<_>>(),
            ["crates/x/src/planted.rs"]
        );

        let listed = listed_files(&notice("")).unwrap();
        let found = violations(&marked, &listed);
        assert_eq!(found.len(), 1);
        assert!(
            found[0].starts_with("crates/x/src/planted.rs: carries the reuse marker"),
            "{}",
            found[0]
        );

        let listed = listed_files(&notice(
            "| `crates/x/src/planted.rs` | voxtype/src/a.rs | abc123 | MIT |\n| `crates/x/src/clean.rs` | voxtype/src/b.rs | abc123 | MIT |\n",
        ))
        .unwrap();
        let found = violations(&marked, &listed);
        assert_eq!(found.len(), 1);
        assert!(
            found[0].contains("lists `crates/x/src/clean.rs` but the file carries no"),
            "{}",
            found[0]
        );

        let listed = listed_files(&notice(
            "| `crates/x/src/planted.rs` | voxtype/src/a.rs | abc123 | MIT |\n",
        ))
        .unwrap();
        assert!(violations(&marked, &listed).is_empty());
        assert!(listed_files("no markers").is_err());
        assert!(is_exempt("NOTICE.md") && !is_exempt("crates/x/src/a.rs"));
    }
}
