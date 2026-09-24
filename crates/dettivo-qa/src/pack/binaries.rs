//! The binaries a pack drives, named once per run from the build profile
//! and recorded with their hash and, for the CLI, the commit they embed;
//! the release gate refuses a set that is not this commit's release build.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::Digest;

/// One binary the pack drove, with the profile it was built under, the
/// SHA-256 of its bytes and, for the CLI, the commit it embeds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinaryInfo {
    /// The path.
    pub path: String,
    /// `release`, `debug`, or `qt` (the CMake tree, built Release).
    pub profile: String,
    /// The SHA-256 of the file, empty when it is missing.
    #[serde(default)]
    pub sha256: String,
    /// The commit `dettivo --version` prints (twelve hex digits), for
    /// the CLI; `None` for the binaries that embed none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

impl BinaryInfo {
    fn missing(why: String) -> Self {
        Self {
            path: String::new(),
            profile: format!("missing: {why}"),
            sha256: String::new(),
            revision: None,
        }
    }

    fn at(path: &Path, profile: &str) -> Self {
        Self {
            path: path.display().to_string(),
            profile: profile.into(),
            sha256: std::fs::read(path)
                .map(|bytes| format!("{:x}", sha2::Sha256::digest(&bytes)))
                .unwrap_or_default(),
            revision: None,
        }
    }
}

/// The commit `dettivo --version` prints: `dettivo <version> (<sha>)`.
fn cli_revision(path: &Path) -> Option<String> {
    let out = std::process::Command::new(path)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let text = String::from_utf8_lossy(&out.stdout);
    let inner = text.trim().rsplit_once('(')?.1.strip_suffix(')')?;
    Some(inner.trim().to_string())
}

/// `Ok` when the binaries are the release build of this commit: `dettivod`
/// and `dettivo` from the release profile, and the commit the CLI embeds
/// is `git_sha` (its first twelve digits); otherwise why not.
pub fn check_release_binaries(
    binaries: &BTreeMap<String, BinaryInfo>,
    git_sha: &str,
) -> Result<(), String> {
    for name in ["dettivod", "dettivo"] {
        let info = binaries
            .get(name)
            .ok_or_else(|| format!("{name} is not recorded"))?;
        if info.profile != "release" {
            return Err(format!(
                "{name} is {} rather than the release build; run `just build-release`",
                info.profile
            ));
        }
    }
    let cli = &binaries["dettivo"];
    match &cli.revision {
        Some(rev) if !rev.is_empty() && git_sha.starts_with(rev.as_str()) => Ok(()),
        Some(rev) => Err(format!(
            "dettivo at {} is built from {rev}, this checkout is {}; run `just build-release` at this commit",
            cli.path,
            &git_sha[..git_sha.len().min(12)]
        )),
        None => Err(format!(
            "dettivo at {} prints no commit in `--version`",
            cli.path
        )),
    }
}

/// The binaries a pack drives, by name, as `scenarios::binary` resolves
/// them before the first step; a missing one is recorded as such.
pub fn binaries(repo_root: &Path) -> BTreeMap<String, BinaryInfo> {
    [
        "dettivod",
        "dettivo",
        "dettivo-app",
        "dettivo-osd",
        "dettivo-qa",
    ]
    .iter()
    .map(|name| {
        let info = if *name == "dettivo-qa" {
            let profile = if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            };
            std::env::current_exe()
                .map(|p| BinaryInfo::at(&p, profile))
                .unwrap_or_else(|e| BinaryInfo::missing(e.to_string()))
        } else {
            match crate::scenarios::binary(repo_root, name) {
                Ok(path) => {
                    let mut info = BinaryInfo::at(&path, crate::scenarios::binary_profile(&path));
                    if *name == "dettivo" {
                        info.revision = cli_revision(&path);
                    }
                    info
                }
                Err(why) => BinaryInfo::missing(why),
            }
        };
        ((*name).to_string(), info)
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_release_gate_refuses_a_debug_or_stale_binary_set() {
        let sha = "2582ff95409a9eca69389092e5537450440bc61b";
        let info = |profile: &str, revision: Option<&str>| BinaryInfo {
            path: format!("/r/target/{profile}/x"),
            profile: profile.into(),
            sha256: "ab".into(),
            revision: revision.map(str::to_string),
        };
        let mut set: BTreeMap<String, BinaryInfo> = BTreeMap::new();
        set.insert("dettivod".into(), info("release", None));
        set.insert("dettivo".into(), info("release", Some("2582ff95409a")));
        assert_eq!(check_release_binaries(&set, sha), Ok(()));
        set.insert("dettivo".into(), info("release", Some("0b87fa4e6c0f")));
        let why = check_release_binaries(&set, sha).unwrap_err();
        assert!(
            why.contains("built from 0b87fa4e6c0f") && why.contains("2582ff95409a"),
            "{why}"
        );
        set.insert("dettivo".into(), info("release", None));
        assert!(
            check_release_binaries(&set, sha)
                .unwrap_err()
                .contains("prints no commit")
        );
        set.insert("dettivod".into(), info("debug", None));
        let why = check_release_binaries(&set, sha).unwrap_err();
        assert!(why.contains("dettivod is debug"), "{why}");
        set.remove("dettivod");
        assert!(
            check_release_binaries(&set, sha)
                .unwrap_err()
                .contains("not recorded")
        );
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("dettivo");
        std::fs::write(&bin, b"#!/bin/sh\necho 'dettivo 0.1.0 (2582ff95409a)'\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(cli_revision(&bin).as_deref(), Some("2582ff95409a"));
        let hashed = BinaryInfo::at(&bin, "release");
        assert_eq!(hashed.sha256.len(), 64);
    }
}
