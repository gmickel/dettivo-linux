//! `lint-notice`: every external crate in `Cargo.lock` has a row in the
//! generated section of `NOTICE.md`, no row names a crate the lock no
//! longer carries, and every source span reused from another project
//! carries the marker with its row in the reuse table (`notice_markers`).
//! `lint-notice --write` regenerates the crate section from
//! `cargo metadata` (name, version and licence per crate) so the notice
//! stays in step with the dependencies the binaries link.

use std::collections::BTreeMap;
use std::process::Command;

use crate::{Outcome, ToolError};

/// The notice at the repository root; the package installs it under
/// `/usr/share/doc/dettivo/NOTICE.md`.
pub const NOTICE: &str = "NOTICE.md";
const BEGIN: &str = "<!-- crates:begin -->";
const END: &str = "<!-- crates:end -->";

/// Runs the lint, or rewrites the generated section when `write` is set.
pub fn run(write: bool) -> Result<Outcome, ToolError> {
    let root = crate::file_length::repo_root()?;
    let lock = std::fs::read_to_string(root.join("Cargo.lock"))
        .map_err(|e| ToolError(format!("Cargo.lock: {e}")))?;
    let notice_path = root.join(NOTICE);
    let notice =
        std::fs::read_to_string(&notice_path).map_err(|e| ToolError(format!("{NOTICE}: {e}")))?;
    if write {
        let table = render(&metadata(&root)?);
        let updated = replace_section(&notice, &table)?;
        std::fs::write(&notice_path, updated)?;
        println!("{NOTICE}: crate section rewritten");
        return Ok(Outcome::Pass);
    }
    let mut found = violations(&lock, &notice)?;
    found.extend(crate::notice_markers::run(&root, &notice)?);
    for line in &found {
        println!("{line}");
    }
    if found.is_empty() {
        Ok(Outcome::Pass)
    } else {
        Ok(Outcome::Violations)
    }
}

/// Names of every crate in `Cargo.lock` that comes from a registry or a
/// git source; workspace members carry no `source`.
pub fn external_crates(lock: &str) -> Vec<String> {
    let mut crates = Vec::new();
    let mut name: Option<String> = None;
    let mut external = false;
    let flush = |name: &mut Option<String>, external: &mut bool, out: &mut Vec<String>| {
        if let (Some(n), true) = (name.take(), *external) {
            out.push(n);
        }
        *external = false;
    };
    for line in lock.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            flush(&mut name, &mut external, &mut crates);
        } else if let Some(rest) = line.strip_prefix("name = ") {
            name = Some(rest.trim_matches('"').to_string());
        } else if line.starts_with("source = ") {
            external = true;
        }
    }
    flush(&mut name, &mut external, &mut crates);
    crates.sort();
    crates.dedup();
    crates
}

/// The crate names the generated section of the notice lists.
pub fn listed_crates(notice: &str) -> Result<Vec<String>, ToolError> {
    let section = section(notice)?;
    let mut names: Vec<String> = section
        .lines()
        .filter_map(|l| l.strip_prefix("| `"))
        .filter_map(|l| l.split('`').next())
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    Ok(names)
}

/// One line per crate missing from the notice or listed without being in
/// the lock any more.
pub fn violations(lock: &str, notice: &str) -> Result<Vec<String>, ToolError> {
    let listed = listed_crates(notice)?;
    let mut found = Vec::new();
    for name in external_crates(lock) {
        if listed.binary_search(&name).is_err() {
            found.push(format!(
                "{NOTICE}: Cargo.lock carries `{name}` but the crate section does not list it; run `cargo run -p xtask -- lint-notice --write`"
            ));
        }
    }
    let external = external_crates(lock);
    for name in listed {
        if external.binary_search(&name).is_err() {
            found.push(format!(
                "{NOTICE}: the crate section lists `{name}` which Cargo.lock no longer carries; run `cargo run -p xtask -- lint-notice --write`"
            ));
        }
    }
    Ok(found)
}

fn section(notice: &str) -> Result<&str, ToolError> {
    let start = notice
        .find(BEGIN)
        .ok_or_else(|| ToolError(format!("{NOTICE}: missing the {BEGIN} marker")))?;
    let end = notice
        .find(END)
        .ok_or_else(|| ToolError(format!("{NOTICE}: missing the {END} marker")))?;
    if end < start {
        return Err(ToolError(format!("{NOTICE}: {END} comes before {BEGIN}")));
    }
    Ok(&notice[start + BEGIN.len()..end])
}

fn replace_section(notice: &str, table: &str) -> Result<String, ToolError> {
    let start = notice
        .find(BEGIN)
        .ok_or_else(|| ToolError(format!("{NOTICE}: missing the {BEGIN} marker")))?;
    let end = notice
        .find(END)
        .ok_or_else(|| ToolError(format!("{NOTICE}: missing the {END} marker")))?;
    Ok(format!(
        "{}{BEGIN}\n{table}{END}{}",
        &notice[..start],
        &notice[end + END.len()..]
    ))
}

/// (name, version) -> licence expression for every external crate.
fn metadata(root: &std::path::Path) -> Result<BTreeMap<(String, String), String>, ToolError> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--locked"])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(ToolError(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let doc: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| ToolError(format!("cargo metadata: {e}")))?;
    let members: Vec<&str> = doc["workspace_members"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let mut out = BTreeMap::new();
    for pkg in doc["packages"].as_array().into_iter().flatten() {
        if members.contains(&pkg["id"].as_str().unwrap_or_default()) {
            continue;
        }
        let name = pkg["name"].as_str().unwrap_or_default().to_string();
        let version = pkg["version"].as_str().unwrap_or_default().to_string();
        let license = pkg["license"]
            .as_str()
            .filter(|l| !l.is_empty())
            .map(|l| l.replace('/', " OR "))
            .unwrap_or_else(|| "see the crate's LICENSE file".to_string());
        out.insert((name, version), license);
    }
    Ok(out)
}

fn render(crates: &BTreeMap<(String, String), String>) -> String {
    let mut table = String::from("| Crate | Version | License |\n|---|---|---|\n");
    for ((name, version), license) in crates {
        table.push_str(&format!("| `{name}` | {version} | {license} |\n"));
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = r#"
[[package]]
name = "dettivo-cli"
version = "0.1.0"
dependencies = ["clap"]

[[package]]
name = "clap"
version = "4.5.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "serde"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;

    fn notice(rows: &str) -> String {
        format!("# Notice\n\n{BEGIN}\n| Crate | Version | License |\n|---|---|---|\n{rows}{END}\n")
    }

    #[test]
    fn workspace_members_are_not_external() {
        assert_eq!(external_crates(LOCK), vec!["clap", "serde"]);
    }

    #[test]
    fn a_missing_and_a_stale_crate_are_named() {
        let n = notice("| `clap` | 4.5.0 | MIT |\n| `old` | 0.1.0 | MIT |\n");
        let found = violations(LOCK, &n).unwrap();
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].contains("`serde`"));
        assert!(found[1].contains("`old`"));
        let complete = notice("| `clap` | 4.5.0 | MIT |\n| `serde` | 1.0.0 | MIT |\n");
        assert!(violations(LOCK, &complete).unwrap().is_empty());
    }

    #[test]
    fn the_section_is_replaced_between_the_markers() {
        let n = notice("| `old` | 0.1.0 | MIT |\n");
        let mut crates = BTreeMap::new();
        crates.insert(("clap".to_string(), "4.5.0".to_string()), "MIT".to_string());
        let updated = replace_section(&n, &render(&crates)).unwrap();
        assert!(updated.contains("| `clap` | 4.5.0 | MIT |"));
        assert!(!updated.contains("`old`"));
        assert!(updated.starts_with("# Notice"));
        assert!(section("no markers").is_err());
    }
}
