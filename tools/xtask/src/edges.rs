//! `lint-edges`: every workspace crate may only depend on the workspace
//! crates listed in [`ALLOWED`], the native inference bindings included.
//!
//! Dependency cycles are not checked here: Cargo rejects them at resolve
//! time, so a cyclic edge never gets as far as this lint.

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use crate::{Outcome, ToolError};

/// Allowed workspace edges per workspace crate: `(crate, allowed deps)`.
///
/// Every workspace member except `xtask` must appear here. An empty list
/// means the crate may not depend on any other workspace crate.
pub const ALLOWED: &[(&str, &[&str])] = &[
    ("dettivo-proto", &[]),
    ("dettivo-core", &["dettivo-proto"]),
    ("dettivo-audio", &["dettivo-proto"]),
    ("dettivo-engine-proto", &[]),
    ("dettivo-speech", &["dettivo-proto", "dettivo-engine-proto"]),
    // The session runs the mode step itself (ADR 0023): the raw layer,
    // the deterministic Polish pass and the Enhanced rewrite all live in
    // dettivo-language, and the `[polish]`/`[llm]` sections it freezes
    // come from dettivo-core's schema.
    (
        "dettivo-session",
        &[
            "dettivo-proto",
            "dettivo-audio",
            "dettivo-core",
            "dettivo-language",
            "dettivo-speech",
            "dettivo-engine-proto",
        ],
    ),
    // The text layers read `[polish]` and `[llm]` straight from the
    // daemon's configuration schema (ADR 0023), so they carry the same
    // vocabulary the config file and the `polish.*` methods use.
    (
        "dettivo-language",
        &["dettivo-proto", "dettivo-engine-proto", "dettivo-core"],
    ),
    // The chunked long-audio pipeline (ADR 0022): engines through
    // dettivo-speech, with protocol and engine types shared directly.
    (
        "dettivo-transcribe",
        &["dettivo-proto", "dettivo-engine-proto", "dettivo-speech"],
    ),
    ("dettivo-insert", &["dettivo-proto"]),
    // The daemon runs its backends; the CLI renders its snippets (without
    // the backend feature) and its test checks the chord defaults against
    // dettivo-core's schema.
    ("dettivo-hotkeys", &["dettivo-proto", "dettivo-core"]),
    ("dettivo-storage", &["dettivo-proto"]),
    // The meeting session (ADR 0027, ADR 0030): two-stream capture
    // through dettivo-audio, the meeting row and its directory through
    // dettivo-storage, the live windows and the finalisation through
    // dettivo-transcribe over an engine from dettivo-speech.
    (
        "dettivo-meeting",
        &[
            "dettivo-proto",
            "dettivo-audio",
            "dettivo-storage",
            "dettivo-transcribe",
            "dettivo-speech",
            "dettivo-engine-proto",
        ],
    ),
    ("dettivo-engine-whisper", &["dettivo-engine-proto"]),
    // The native bindings are plain -sys crates; only their engine binary
    // links them (ADR 0003), so no other crate carries inference code.
    ("parakeet-cpp-sys", &[]),
    ("sherpa-onnx-sys", &[]),
    (
        "dettivo-engine-parakeet",
        &["dettivo-engine-proto", "parakeet-cpp-sys"],
    ),
    ("dettivo-engine-llm", &["dettivo-engine-proto"]),
    (
        "dettivo-engine-diarize",
        &["dettivo-engine-proto", "sherpa-onnx-sys"],
    ),
    (
        "dettivod",
        &[
            "dettivo-proto",
            "dettivo-core",
            "dettivo-audio",
            "dettivo-engine-proto",
            "dettivo-speech",
            "dettivo-session",
            "dettivo-language",
            "dettivo-transcribe",
            "dettivo-insert",
            "dettivo-hotkeys",
            "dettivo-storage",
            "dettivo-meeting",
            "dettivo-rest",
        ],
    ),
    // The CLI runs the MCP server in-process (`dettivo mcp serve`) and the
    // QA runner drives a spawned one through the crate's harness; both
    // still reach the daemon through dettivo-proto only (ADR 0019). The
    // CLI hosts the REST shim the same way (`dettivo rest serve`, ADR 0028).
    (
        "dettivo-cli",
        &[
            "dettivo-proto",
            "dettivo-hotkeys",
            "dettivo-mcp",
            "dettivo-rest",
        ],
    ),
    ("dettivo-mcp", &["dettivo-proto"]),
    // The REST shim (ADR 0028) is hosted by the daemon and by the CLI; it
    // reads the shared token and the `[rest]` schema from dettivo-core and
    // reaches the daemon through the contract types only.
    ("dettivo-rest", &["dettivo-proto", "dettivo-core"]),
    // The QA runner also drives the chunker and merger of the long-audio
    // pipeline through the engine CLI modes (ADR 0022) and replays the
    // REST fixtures through dettivo-rest's harness (ADR 0028).
    (
        "dettivo-qa",
        &[
            "dettivo-proto",
            "dettivo-mcp",
            "dettivo-rest",
            "dettivo-transcribe",
        ],
    ),
];

/// Workspace members exempt from the table.
const EXEMPT: &[&str] = &["xtask"];

/// Client crates: they talk to the daemon through `dettivo-proto` only.
const CLIENTS: &[&str] = &["dettivo-cli", "dettivo-mcp", "dettivo-qa"];

/// Engine binaries: they talk to the daemon through `dettivo-engine-proto` only.
const ENGINES: &[&str] = &[
    "dettivo-engine-whisper",
    "dettivo-engine-parakeet",
    "dettivo-engine-llm",
    "dettivo-engine-diarize",
];

/// The native inference bindings: an engine binary links its own, nothing
/// else links either.
const NATIVE_BINDINGS: &[&str] = &["parakeet-cpp-sys", "sherpa-onnx-sys"];

/// Crates that are private to the daemon process.
const DAEMON_INTERNALS: &[&str] = &[
    "dettivod",
    "dettivo-core",
    "dettivo-audio",
    "dettivo-speech",
    "dettivo-language",
    "dettivo-insert",
    "dettivo-storage",
    "dettivo-meeting",
];

/// One workspace member as seen by the lint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// Package name.
    pub name: String,
    /// Manifest path, printed first in every violation.
    pub manifest: String,
    /// Declared workspace dependencies (normal, dev, and build kinds).
    pub deps: Vec<String>,
}

/// Runs the lint against the current workspace.
pub fn run() -> Result<Outcome, ToolError> {
    let members = workspace_members()?;
    let violations = check(&members);
    for line in &violations {
        println!("{line}");
    }
    Ok(if violations.is_empty() {
        Outcome::Pass
    } else {
        Outcome::Violations
    })
}

/// Pure rule logic: returns one message per violation, path first.
pub fn check(members: &[Member]) -> Vec<String> {
    let mut out = Vec::new();
    for member in members {
        if EXEMPT.contains(&member.name.as_str()) {
            continue;
        }
        let Some((_, allowed)) = ALLOWED.iter().find(|(name, _)| *name == member.name) else {
            out.push(format!(
                "{}: workspace member {} is not in the edge table; add it to the edge table in tools/xtask/src/edges.rs",
                member.manifest, member.name
            ));
            continue;
        };
        for dep in &member.deps {
            if allowed.contains(&dep.as_str()) {
                continue;
            }
            out.push(describe(member, dep));
        }
    }
    for (name, _) in ALLOWED {
        if !members.iter().any(|m| m.name == *name) {
            out.push(format!(
                "tools/xtask/src/edges.rs: edge table lists {name} but no workspace crate has that name"
            ));
        }
    }
    out
}

fn describe(member: &Member, dep: &str) -> String {
    let name = member.name.as_str();
    let internal = DAEMON_INTERNALS.contains(&dep);
    let reason = if CLIENTS.contains(&name) && internal {
        "clients depend on dettivo-proto only"
    } else if ENGINES.contains(&name) && internal {
        "engines depend on dettivo-engine-proto only"
    } else if NATIVE_BINDINGS.contains(&dep) {
        "a native inference binding links into its own engine binary only (ADR 0003)"
    } else {
        "not in the edge table in tools/xtask/src/edges.rs"
    };
    format!(
        "{}: forbidden edge: {name} -> {dep} ({reason})",
        member.manifest
    )
}

fn workspace_members() -> Result<Vec<Member>, ToolError> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()?;
    if !output.status.success() {
        return Err(ToolError(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let meta: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| ToolError(format!("cargo metadata output: {e}")))?;
    let root = meta["workspace_root"].as_str().unwrap_or("");
    let packages = meta["packages"]
        .as_array()
        .ok_or_else(|| ToolError("cargo metadata: missing packages".into()))?;
    // `--no-deps` lists the workspace members and nothing else, so their
    // names are the set a dependency is checked against.
    let workspace: HashSet<&str> = packages
        .iter()
        .filter_map(|pkg| pkg["name"].as_str())
        .collect();
    Ok(packages
        .iter()
        .map(|pkg| member_from(pkg, root, &workspace))
        .collect())
}

/// The lint's view of one package: its manifest and the dependencies that
/// are workspace members. Membership, not the `dettivo-` prefix, decides
/// what counts, so `parakeet-cpp-sys` and `sherpa-onnx-sys` are edges.
fn member_from(pkg: &serde_json::Value, root: &str, workspace: &HashSet<&str>) -> Member {
    let manifest = pkg["manifest_path"].as_str().unwrap_or("");
    let manifest = Path::new(manifest)
        .strip_prefix(root)
        .map_or_else(|_| manifest.to_string(), |p| p.display().to_string());
    let deps = pkg["dependencies"]
        .as_array()
        .map(|deps| {
            deps.iter()
                .filter_map(|d| d["name"].as_str())
                .filter(|n| workspace.contains(n))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Member {
        name: pkg["name"].as_str().unwrap_or("").to_string(),
        manifest,
        deps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(name: &str, deps: &[&str]) -> Member {
        Member {
            name: name.into(),
            manifest: format!("crates/{name}/Cargo.toml"),
            deps: deps.iter().map(|d| (*d).to_string()).collect(),
        }
    }

    /// Every table crate with exactly its allowed edges.
    fn full_workspace() -> Vec<Member> {
        let mut members: Vec<Member> = ALLOWED
            .iter()
            .map(|(name, deps)| member(name, deps))
            .collect();
        members.push(member("xtask", &[]));
        members
    }

    #[test]
    fn full_workspace_passes() {
        assert_eq!(check(&full_workspace()), Vec::<String>::new());
    }

    #[test]
    fn violations_table() {
        let cases: &[(&str, &[&str], &str)] = &[
            (
                "dettivo-cli",
                &["dettivo-proto", "dettivo-core"],
                "crates/dettivo-cli/Cargo.toml: forbidden edge: dettivo-cli -> dettivo-core (clients depend on dettivo-proto only)",
            ),
            (
                "dettivo-qa",
                &["dettivod"],
                "crates/dettivo-qa/Cargo.toml: forbidden edge: dettivo-qa -> dettivod (clients depend on dettivo-proto only)",
            ),
            (
                "dettivo-engine-llm",
                &["dettivo-engine-proto", "dettivo-speech"],
                "crates/dettivo-engine-llm/Cargo.toml: forbidden edge: dettivo-engine-llm -> dettivo-speech (engines depend on dettivo-engine-proto only)",
            ),
            (
                "dettivo-mcp",
                &["dettivo-engine-proto"],
                "crates/dettivo-mcp/Cargo.toml: forbidden edge: dettivo-mcp -> dettivo-engine-proto (not in the edge table in tools/xtask/src/edges.rs)",
            ),
            (
                "dettivo-proto",
                &["dettivo-core"],
                "crates/dettivo-proto/Cargo.toml: forbidden edge: dettivo-proto -> dettivo-core (not in the edge table in tools/xtask/src/edges.rs)",
            ),
        ];
        for (name, deps, expected) in cases {
            let mut members = full_workspace();
            members.retain(|m| m.name != *name);
            members.push(member(name, deps));
            let got = check(&members);
            assert_eq!(got, vec![(*expected).to_string()], "case {name}");
        }
    }

    #[test]
    fn unknown_member_is_reported() {
        let mut members = full_workspace();
        members.push(member("dettivo-new", &[]));
        let got = check(&members);
        assert_eq!(got.len(), 1);
        assert!(got[0].starts_with(
            "crates/dettivo-new/Cargo.toml: workspace member dettivo-new is not in the edge table"
        ));
        assert!(got[0].ends_with("add it to the edge table in tools/xtask/src/edges.rs"));
    }

    #[test]
    fn missing_crate_is_reported() {
        let mut members = full_workspace();
        members.retain(|m| m.name != "dettivo-storage");
        let got = check(&members);
        assert_eq!(
            got,
            vec![
                "tools/xtask/src/edges.rs: edge table lists dettivo-storage but no workspace crate has that name".to_string()
            ]
        );
    }

    /// The metadata reader keeps every workspace dependency, native
    /// bindings included, and drops the crates outside the workspace.
    #[test]
    fn metadata_keeps_native_workspace_edges() {
        let pkg = serde_json::json!({
            "name": "dettivo-cli",
            "manifest_path": "/ws/crates/dettivo-cli/Cargo.toml",
            "dependencies": [
                {"name": "dettivo-proto"},
                {"name": "parakeet-cpp-sys"},
                {"name": "serde"}
            ]
        });
        let workspace: HashSet<&str> = ["dettivo-cli", "dettivo-proto", "parakeet-cpp-sys"]
            .into_iter()
            .collect();
        let member = member_from(&pkg, "/ws", &workspace);
        assert_eq!(member.manifest, "crates/dettivo-cli/Cargo.toml");
        assert_eq!(member.deps, vec!["dettivo-proto", "parakeet-cpp-sys"]);
        let mut members = full_workspace();
        members.retain(|m| m.name != "dettivo-cli");
        members.push(member);
        assert_eq!(
            check(&members),
            vec![
                "crates/dettivo-cli/Cargo.toml: forbidden edge: dettivo-cli -> parakeet-cpp-sys (a native inference binding links into its own engine binary only (ADR 0003))"
            ]
        );
    }

    #[test]
    fn xtask_is_exempt_from_table() {
        assert!(!ALLOWED.iter().any(|(n, _)| *n == "xtask"));
        assert!(EXEMPT.contains(&"xtask"));
    }
}
