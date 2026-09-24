//! The release gate's report files: `release-pack.json` and
//! `release-pack.md` under the run directory, and the checked-in copy
//! `docs/reports/release-gate/<version>.json` with its `.md` on `--record`.

use std::path::{Path, PathBuf};

use super::release::{Blocker, RECORD_DIR, Report};

fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\n', '\r'], " ")
}

fn blockers(title: &str, list: &[Blocker], out: &mut String, none: &str) {
    out.push_str(&format!("\n## {title}\n\n"));
    if list.is_empty() {
        out.push_str(none);
        out.push('\n');
        return;
    }
    for b in list {
        out.push_str(&format!("- `{}`: {}", b.step, cell(&b.reason)));
        if let Some(needs) = &b.needs {
            out.push_str(&format!(" Needs {}.", cell(needs)));
        }
        out.push('\n');
    }
}

/// The Markdown rendering.
pub fn markdown(r: &Report) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# Release gate {}: {}\n\n",
        r.version,
        if r.passed { "passed" } else { "FAILED" }
    ));
    out.push_str(&format!(
        "Commit `{}` on `{}` ({}, {} session, gpu {}), {} ms; run directory `{}`. `dettivo-qa pack release` ran every step below in order and embedded each step's report in `{}.json`; the claim that only external blockers remain holds when the gate passed and the unexplained list is empty.\n\n",
        &r.git_sha[..r.git_sha.len().min(12)],
        r.machine.hostname,
        r.machine.compositor.as_deref().unwrap_or("no compositor"),
        r.machine.session,
        r.machine.gpu.as_deref().unwrap_or("none"),
        r.duration_ms,
        r.run_dir,
        r.version
    ));
    if !r.binaries.is_empty() {
        let list: Vec<String> = r
            .binaries
            .iter()
            .map(|(n, b)| format!("`{n}` {}", b.profile))
            .collect();
        out.push_str(&format!("Binaries: {}.\n\n", list.join(", ")));
    }
    if !r.skipped.is_empty() {
        out.push_str(&format!(
            "Left out by `--skip`: {}.\n\n",
            r.skipped.join(", ")
        ));
    }
    out.push_str(
        "## Steps\n\n| Step | Outcome | Duration | Command | Reason |\n|---|---|---|---|---|\n",
    );
    for s in &r.steps {
        out.push_str(&format!(
            "| `{}` | {} | {} ms | `{}` | {} |\n",
            s.id,
            s.outcome.as_str(),
            s.duration_ms,
            cell(&s.command),
            cell(s.reason.as_deref().unwrap_or(""))
        ));
    }
    blockers("External blockers", &r.external_blockers, &mut out, "None.");
    blockers(
        "Unexplained blockers",
        &r.unexplained_blockers,
        &mut out,
        "None: every blocker names a cause outside this machine and this repository.",
    );
    out
}

/// Writes `release-pack.json` and `release-pack.md` into `dir`.
pub fn write(report: &Report, dir: &Path) -> std::io::Result<(PathBuf, PathBuf)> {
    let json = dir.join("release-pack.json");
    let md = dir.join("release-pack.md");
    std::fs::write(
        &json,
        serde_json::to_string_pretty(report).map_err(std::io::Error::other)? + "\n",
    )?;
    std::fs::write(&md, markdown(report))?;
    Ok((json, md))
}

/// Files the run as `docs/reports/release-gate/<version>.json` and `.md`.
pub fn record(report: &Report, root: &Path) -> std::io::Result<PathBuf> {
    let dir = root.join(RECORD_DIR);
    std::fs::create_dir_all(&dir)?;
    let json = dir.join(format!("{}.json", report.version));
    std::fs::write(
        &json,
        serde_json::to_string_pretty(report).map_err(std::io::Error::other)? + "\n",
    )?;
    std::fs::write(dir.join(format!("{}.md", report.version)), markdown(report))?;
    Ok(json)
}
