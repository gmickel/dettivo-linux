//! The evidence map's report files: `docs/reports/evidence-map.json` and
//! its Markdown twin, one table per spec with every route and its
//! resolution, the totals first.

use std::path::{Path, PathBuf};

use super::Report;

fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\n', '\r'], " ")
}

/// The Markdown rendering.
pub fn markdown(r: &Report) -> String {
    let t = &r.totals;
    let mut out = String::new();
    out.push_str("# Evidence map\n\n");
    out.push_str(&format!(
        "Every requirement of every spec under `.flow/specs/` mapped to a verification route that exists (NFR-10): {} of {} R-IDs over {} specs, coverage {:.3}, {} routes ({} of them walked by a person), at commit `{}`. This is the inventory of routes, not their results: a route that resolves exists, the release gate is what runs it, and a `human` route is the receipt a person files rather than proof that they walked it. `dettivo-qa evidence-map --write` regenerates this file from `qa/evidence-map.toml`.\n\n",
        t.mapped,
        t.rids,
        t.specs,
        t.coverage,
        t.routes,
        t.human_routes,
        &r.git_sha[..r.git_sha.len().min(12)]
    ));
    out.push_str("| Kind | Routes |\n|---|---|\n");
    for (kind, n) in &t.by_kind {
        out.push_str(&format!("| `{kind}` | {n} |\n"));
    }
    out.push_str("\n## Findings\n\n");
    if t.findings.is_empty() {
        out.push_str("None: every R-ID has a route and every reference resolves.\n");
    } else {
        for f in &t.findings {
            out.push_str(&format!("- {}\n", cell(f)));
        }
    }
    out.push_str(
        "\n## Specs\n\n| Spec | R-IDs | Mapped | Coverage | Human only |\n|---|---|---|---|---|\n",
    );
    for s in &r.specs {
        out.push_str(&format!(
            "| `{}` | {} | {} | {:.3} | {} |\n",
            s.id,
            s.rids.len(),
            s.mapped.len(),
            s.coverage,
            if s.human_only.is_empty() {
                "-".to_string()
            } else {
                s.human_only.join(", ")
            }
        ));
    }
    for s in &r.specs {
        out.push_str(&format!("\n### `{}`\n\n{}\n\n", s.id, cell(&s.title)));
        if !s.unmapped.is_empty() {
            out.push_str(&format!("Unmapped: {}.\n\n", s.unmapped.join(", ")));
        }
        out.push_str("| R-IDs | Kind | Ref | Resolves | Proves |\n|---|---|---|---|---|\n");
        for route in &s.routes {
            out.push_str(&format!(
                "| {} | `{}` | `{}` | {} | {} |\n",
                route.rids.join(", "),
                route.kind,
                cell(&route.reference),
                if route.resolved {
                    cell(&route.detail)
                } else {
                    format!("**no**: {}", cell(&route.detail))
                },
                cell(&route.note)
            ));
        }
    }
    out
}

/// Writes the JSON at `json_path` and the Markdown beside it; returns
/// the Markdown path.
pub fn write(report: &Report, json_path: &Path) -> std::io::Result<PathBuf> {
    if let Some(parent) = json_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let md_path = json_path.with_extension("md");
    std::fs::write(
        json_path,
        serde_json::to_string_pretty(report).map_err(std::io::Error::other)? + "\n",
    )?;
    std::fs::write(&md_path, markdown(report))?;
    Ok(md_path)
}
