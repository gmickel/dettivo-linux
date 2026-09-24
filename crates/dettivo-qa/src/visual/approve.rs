//! `dettivo-qa visual approve` (fn-20 R2): renders the selected entries
//! fresh, refuses a render the style check named, copies each into
//! `baselines/<surface>/<state>.<theme>.<scale>x.png`, and records who
//! approved what, when, and how the render scored against the artboard
//! in `docs/design/baselines.md`, so every baseline beyond the artboards
//! is a reviewed commit with its provenance in one table.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::manifest::Manifest;
use super::matrix::{Baseline, Entry, Filter, approved_path, expand};
use super::render::{compare, diff_tool, render};

#[path = "approve_publish.rs"]
mod publish;

/// The approval log, relative to the repository root.
pub const LOG_PATH: &str = "docs/design/baselines.md";

/// One approved baseline.
#[derive(Debug, Clone, PartialEq)]
pub struct Approved {
    /// The entry.
    pub entry: Entry,
    /// Where the baseline now lives.
    pub path: PathBuf,
    /// The render's score against the artboard crop, when there is one.
    pub artboard_score: Option<f64>,
    /// Whether a baseline already existed and was replaced.
    pub replaced: bool,
}

/// Renders and approves every entry the filter keeps; `by` names the
/// approver in the log, the git user otherwise.
pub fn approve(
    repo: &Path,
    manifest: &Manifest,
    filter: &Filter,
    work_dir: &Path,
    by: Option<&str>,
) -> Result<Vec<Approved>, String> {
    let entries = expand(manifest, repo, filter);
    if entries.is_empty() {
        return Err(format!("no manifest entry matches {filter:?}"));
    }
    let tool = diff_tool(repo)?;
    let who = by.map(str::to_string).unwrap_or_else(approver);
    let today = today_utc();
    let mut approved = Vec::new();
    std::fs::create_dir_all(work_dir).map_err(|e| e.to_string())?;
    let staging = tempfile::tempdir_in(work_dir).map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for entry in entries {
        let dir = staging.path().join(&entry.surface);
        let png = dir.join(format!("{}.png", entry.stem()));
        let log = dir.join(format!("{}.log", entry.stem()));
        render(repo, &entry, &png, &log, None)
            .map_err(|e| format!("{}/{}: {e}; nothing approved", entry.surface, entry.stem()))?;
        let artboard_score = match &entry.artboard_crop {
            Some(crop) if crop.is_file() => {
                Some(compare(&tool, crop, &png, entry.threshold(), None)?.score)
            }
            _ => None,
        };
        let path = approved_path(
            repo,
            &entry.surface,
            &entry.state,
            &entry.theme,
            entry.scale,
        );
        let replaced = matches!(&entry.baseline, Baseline::Approved(_));
        files.push((path.clone(), png));
        approved.push(Approved {
            entry,
            path,
            artboard_score,
            replaced,
        });
    }
    let mut text = log_text(repo)?;
    for row in &approved {
        append_row(
            &mut text,
            repo,
            &row.entry,
            &row.path,
            row.artboard_score,
            &who,
            &today,
        );
    }
    let staged_log = staging.path().join("provenance.md");
    std::fs::write(&staged_log, text).map_err(|e| e.to_string())?;
    files.push((repo.join(LOG_PATH), staged_log));
    publish::publish(files)?;
    Ok(approved)
}

/// The header the log starts with when it does not exist yet.
const LOG_HEADER: &str = "# Approved visual baselines\n\n\
Every baseline beyond the Black Gold artboards is a render someone accepted through `dettivo-qa visual approve` \
(ADR 0021); the table is its provenance. A row's artboard score is the render's structure correlation against the \
approved artboard crop where the surface has one, evidence for the beauty pass, not a gate.\n\n\
| Date | Surface | State | Theme | Scale | Baseline | Approved by | Artboard score |\n\
|---|---|---|---|---|---|---|---|\n";

/// Appends the row for one approval, creating the log with its header.
pub fn record(
    repo: &Path,
    entry: &Entry,
    path: &Path,
    artboard_score: Option<f64>,
    who: &str,
    date: &str,
) -> Result<(), String> {
    let log = repo.join(LOG_PATH);
    let mut text = log_text(repo)?;
    append_row(&mut text, repo, entry, path, artboard_score, who, date);
    std::fs::write(&log, text).map_err(|e| format!("{}: {e}", log.display()))
}

fn log_text(repo: &Path) -> Result<String, String> {
    let log = repo.join(LOG_PATH);
    let mut text = match std::fs::read_to_string(&log) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}; nothing approved", log.display())),
    };
    if text.trim().is_empty() {
        text = LOG_HEADER.to_string();
    } else if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

fn append_row(
    text: &mut String,
    repo: &Path,
    entry: &Entry,
    path: &Path,
    artboard_score: Option<f64>,
    who: &str,
    date: &str,
) {
    let relative = path
        .strip_prefix(repo)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned();
    text.push_str(&format!(
        "| {date} | {} | {} | {} | {}x | `{relative}` | {who} | {} |\n",
        entry.surface,
        entry.state,
        entry.theme,
        entry.scale,
        artboard_score
            .map(|s| format!("{s:.3}"))
            .unwrap_or_else(|| "no artboard".into()),
    ));
}

/// `git config user.name`, else `$USER`, else `unknown`.
pub fn approver() -> String {
    let git = Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());
    git.or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "unknown".into())
}

/// Today as `YYYY-MM-DD` in UTC, from the system clock alone.
pub fn today_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    civil_from_days(i64::try_from(secs / 86_400).unwrap_or(0))
}

/// Howard Hinnant's days-to-civil algorithm.
fn civil_from_days(days: i64) -> String {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
#[path = "approve_transaction_tests.rs"]
mod transaction_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn the_log_gets_a_header_once_and_one_row_per_approval() {
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join("docs/design")).unwrap();
        let entry = Entry {
            surface: "osd".into(),
            state: "listening".into(),
            theme: "tokyo-night".into(),
            scale: 2,
            binary: "dettivo-osd".into(),
            env: BTreeMap::new(),
            args: Vec::new(),
            threshold_milli: 550,
            colour_tolerance_milli: 60,
            baseline: Baseline::Missing(PathBuf::from("/x")),
            artboard_crop: None,
            crop: None,
        };
        let path = repo
            .path()
            .join("docs/design/studio/baselines/osd/listening.tokyo-night.2x.png");
        record(
            repo.path(),
            &entry,
            &path,
            Some(0.6031),
            "Gordon",
            "2026-09-04",
        )
        .unwrap();
        record(repo.path(), &entry, &path, None, "Gordon", "2026-09-04").unwrap();
        let text = std::fs::read_to_string(repo.path().join(LOG_PATH)).unwrap();
        assert_eq!(text.matches("# Approved visual baselines").count(), 1);
        assert!(text.contains(
            "| 2026-09-04 | osd | listening | tokyo-night | 2x | `docs/design/studio/baselines/osd/listening.tokyo-night.2x.png` | Gordon | 0.603 |"
        ));
        assert!(text.contains("| Gordon | no artboard |"));
        assert_eq!(text.lines().filter(|l| l.starts_with("| 2026")).count(), 2);
    }

    #[test]
    fn days_convert_to_civil_dates() {
        assert_eq!(civil_from_days(0), "1970-01-01");
        assert_eq!(civil_from_days(20_700), "2026-09-04");
        assert_eq!(civil_from_days(19_723), "2024-01-01");
    }
}
