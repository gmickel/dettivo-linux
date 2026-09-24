//! `dettivo-qa evidence-map` (NFR-10): every R-ID of every spec under
//! `.flow/specs/` has an evidence route that exists. `qa/evidence-map.toml`
//! lists the routes per spec and R-ID; the verb reads every spec's
//! acceptance criteria, checks each R-ID has at least one route and each
//! route's `ref` resolves against the repository (`resolve`), and writes
//! `docs/reports/evidence-map.json` with its Markdown twin (`report`).
//! Coverage under 1.0 or an unresolved reference exits 1 naming it.

pub mod report;
pub mod resolve;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The map, relative to the repository root.
pub const MAP: &str = "qa/evidence-map.toml";
/// The spec directory, relative to the repository root.
pub const SPECS: &str = ".flow/specs";
/// The checked-in report, relative to the repository root (`.md` beside it).
pub const REPORT: &str = "docs/reports/evidence-map.json";
/// The kinds a route may carry, in the order the report lists them.
pub const KINDS: &[&str] = &[
    "unit", "contract", "pipeline", "drive", "visual", "pack", "bench", "docs", "script", "human",
];

/// One route in the map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Route {
    /// The R-IDs the route serves.
    pub rids: Vec<String>,
    /// One of `KINDS`.
    pub kind: String,
    /// What the kind resolves.
    #[serde(rename = "ref")]
    pub reference: String,
    /// What the route proves, one line.
    #[serde(default)]
    pub note: String,
}

/// One spec's block in the map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecMap {
    /// The spec id (the file name under `.flow/specs/` without `.md`).
    pub id: String,
    /// The routes.
    #[serde(default, rename = "route")]
    pub routes: Vec<Route>,
}

/// The whole map file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Map {
    /// One block per spec.
    #[serde(default, rename = "spec")]
    pub specs: Vec<SpecMap>,
}

impl Map {
    /// Parses the map text.
    pub fn parse(text: &str) -> Result<Self, String> {
        let map: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        for spec in &map.specs {
            for route in &spec.routes {
                if !KINDS.contains(&route.kind.as_str()) {
                    return Err(format!(
                        "{}: kind {:?} on ref {:?} is not one of {}",
                        spec.id,
                        route.kind,
                        route.reference,
                        KINDS.join(", ")
                    ));
                }
                if route.rids.is_empty() {
                    return Err(format!(
                        "{}: route {:?} names no R-ID",
                        spec.id, route.reference
                    ));
                }
            }
        }
        Ok(map)
    }

    /// Reads the map from `root`.
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = root.join(MAP);
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// One spec on disk: its id, title and R-IDs in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    /// The file name without `.md`.
    pub id: String,
    /// The H1 line.
    pub title: String,
    /// `R1`, `R2`, ... as the acceptance criteria list them.
    pub rids: Vec<String>,
}

/// The R-IDs a spec's `## Acceptance Criteria` section names, in order:
/// a bullet `- **R<n>:**` or `- **R<n>**:` (both spellings the specs use).
pub fn rids_of(text: &str) -> Vec<String> {
    let mut in_section = false;
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            in_section = heading.trim().eq_ignore_ascii_case("acceptance criteria");
            continue;
        }
        if !in_section {
            continue;
        }
        let Some(rest) = line.trim_start().strip_prefix("- **R") else {
            continue;
        };
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        let after = &rest[digits.len()..];
        if !digits.is_empty() && (after.starts_with(":**") || after.starts_with("**:")) {
            out.push(format!("R{digits}"));
        }
    }
    out
}

/// Every spec under `<root>/.flow/specs/`, sorted by spec number.
pub fn specs(root: &Path) -> Result<Vec<Spec>, String> {
    let dir = root.join(SPECS);
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut out = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(id) = name.strip_suffix(".md") else {
            continue;
        };
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let title = text
            .lines()
            .find_map(|l| l.strip_prefix("# "))
            .unwrap_or("")
            .trim()
            .to_string();
        out.push(Spec {
            id: id.to_string(),
            title,
            rids: rids_of(&text),
        });
    }
    out.sort_by_key(|s| (spec_number(&s.id), s.id.clone()));
    Ok(out)
}

/// The number in `fn-<n>-...`, or `u32::MAX` for anything else.
pub fn spec_number(id: &str) -> u32 {
    id.strip_prefix("fn-")
        .and_then(|r| r.split('-').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(u32::MAX)
}

/// One route's verdict in the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteReport {
    /// The R-IDs.
    pub rids: Vec<String>,
    /// The kind.
    pub kind: String,
    /// The reference.
    #[serde(rename = "ref")]
    pub reference: String,
    /// The note.
    pub note: String,
    /// True when the reference resolved.
    pub resolved: bool,
    /// What resolved it, or why it did not.
    pub detail: String,
}

/// One spec's block in the report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpecReport {
    /// The spec id.
    pub id: String,
    /// The title.
    pub title: String,
    /// The R-IDs the spec names.
    pub rids: Vec<String>,
    /// The R-IDs with at least one route.
    pub mapped: Vec<String>,
    /// The R-IDs without a route.
    pub unmapped: Vec<String>,
    /// R-IDs the map names that the spec does not have.
    pub unknown: Vec<String>,
    /// R-IDs whose only routes are `human`.
    pub human_only: Vec<String>,
    /// mapped over rids (1.0 for a spec without R-IDs).
    pub coverage: f64,
    /// The routes.
    pub routes: Vec<RouteReport>,
}

/// The totals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Totals {
    /// Specs read.
    pub specs: usize,
    /// R-IDs over every spec.
    pub rids: usize,
    /// R-IDs with a route.
    pub mapped: usize,
    /// mapped over rids.
    pub coverage: f64,
    /// Routes in the map.
    pub routes: usize,
    /// Routes of kind `human`.
    pub human_routes: usize,
    /// Routes per kind.
    pub by_kind: BTreeMap<String, usize>,
    /// Every finding that fails the map: unmapped R-IDs, unknown R-IDs,
    /// specs the map names that do not exist, unresolved references.
    pub findings: Vec<String>,
}

/// The report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// 1.
    pub schema_version: u32,
    /// The commit the report was built at.
    pub git_sha: String,
    /// Unix seconds.
    pub generated_unix: u64,
    /// One block per spec.
    pub specs: Vec<SpecReport>,
    /// The totals.
    pub totals: Totals,
}

impl Report {
    /// True when every R-ID is mapped and every reference resolved.
    pub fn ok(&self) -> bool {
        self.totals.findings.is_empty()
    }
}

fn git_sha(root: &Path) -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Builds the report for `root`: the specs, the map and the resolver.
pub fn build(root: &Path) -> Result<Report, String> {
    let specs = specs(root)?;
    let map = Map::load(root)?;
    let resolver = resolve::Resolver::new(root)?;
    let mut findings = Vec::new();
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    let mut human_routes = 0;
    let mut routes_total = 0;
    let mut blocks = Vec::new();
    for spec_map in &map.specs {
        if !specs.iter().any(|s| s.id == spec_map.id) {
            findings.push(format!(
                "{}: the map names a spec that is not under {SPECS}",
                spec_map.id
            ));
        }
    }
    for spec in &specs {
        let mapped_routes: Vec<&Route> = map
            .specs
            .iter()
            .filter(|m| m.id == spec.id)
            .flat_map(|m| m.routes.iter())
            .collect();
        let mut routes = Vec::new();
        let mut unknown = Vec::new();
        for route in &mapped_routes {
            routes_total += 1;
            *by_kind.entry(route.kind.clone()).or_default() += 1;
            if route.kind == "human" {
                human_routes += 1;
            }
            for rid in &route.rids {
                if !spec.rids.contains(rid) && !unknown.contains(rid) {
                    unknown.push(rid.clone());
                }
            }
            let (resolved, detail) = match resolver.resolve(&route.kind, &route.reference) {
                Ok(d) => (true, d),
                Err(why) => {
                    findings.push(format!(
                        "{}: {} ref {:?} does not resolve: {why}",
                        spec.id, route.kind, route.reference
                    ));
                    (false, why)
                }
            };
            routes.push(RouteReport {
                rids: route.rids.clone(),
                kind: route.kind.clone(),
                reference: route.reference.clone(),
                note: route.note.clone(),
                resolved,
                detail,
            });
        }
        let mapped: Vec<String> = spec
            .rids
            .iter()
            .filter(|r| mapped_routes.iter().any(|m| m.rids.contains(r)))
            .cloned()
            .collect();
        let unmapped: Vec<String> = spec
            .rids
            .iter()
            .filter(|r| !mapped.contains(r))
            .cloned()
            .collect();
        let human_only: Vec<String> = mapped
            .iter()
            .filter(|r| {
                mapped_routes
                    .iter()
                    .filter(|m| m.rids.contains(r))
                    .all(|m| m.kind == "human")
            })
            .cloned()
            .collect();
        for rid in &unmapped {
            findings.push(format!("{}: {rid} has no evidence route", spec.id));
        }
        for rid in &unknown {
            findings.push(format!(
                "{}: the map names {rid} but the spec's acceptance criteria do not",
                spec.id
            ));
        }
        let coverage = if spec.rids.is_empty() {
            1.0
        } else {
            mapped.len() as f64 / spec.rids.len() as f64
        };
        blocks.push(SpecReport {
            id: spec.id.clone(),
            title: spec.title.clone(),
            rids: spec.rids.clone(),
            mapped,
            unmapped,
            unknown,
            human_only,
            coverage,
            routes,
        });
    }
    let rids: usize = blocks.iter().map(|b| b.rids.len()).sum();
    let mapped: usize = blocks.iter().map(|b| b.mapped.len()).sum();
    Ok(Report {
        schema_version: 1,
        git_sha: git_sha(root),
        generated_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        specs: blocks,
        totals: Totals {
            specs: specs.len(),
            rids,
            mapped,
            coverage: if rids == 0 {
                1.0
            } else {
                mapped as f64 / rids as f64
            },
            routes: routes_total,
            human_routes,
            by_kind,
            findings,
        },
    })
}

/// `dettivo-qa evidence-map`: builds the report, prints it (JSON or the
/// Markdown rendering), writes the checked-in copy under `--write`, and
/// exits 0 when every R-ID is mapped and every reference resolved, 1
/// otherwise, 2 when the map or the specs cannot be read.
pub fn command(root: &Path, json: bool, write: bool) -> u8 {
    let report = match build(root) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("evidence-map: {e}");
            return 2;
        }
    };
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_default()
        );
    } else {
        print!("{}", report::markdown(&report));
    }
    if write {
        let json_path: PathBuf = root.join(REPORT);
        match report::write(&report, &json_path) {
            Ok(md) => {
                if !json {
                    println!("written {} and {}", json_path.display(), md.display());
                }
            }
            Err(e) => {
                eprintln!("evidence-map: {}: {e}", json_path.display());
                return 2;
            }
        }
    }
    for f in &report.totals.findings {
        eprintln!("evidence-map: {f}");
    }
    u8::from(!report.ok())
}

#[cfg(test)]
mod tests;
