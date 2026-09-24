//! Upstream's ggml CPU patch (folded broadcast sgemm; a speed-up, not a
//! correctness fix) over a parakeet.cpp source tree. A tree this build
//! fetched and owns is patched in place once; a tree a caller supplied
//! through `PARAKEET_CPP_SOURCE_DIR` is never written to: it is checked
//! for the patch and the build stops naming the command when it lacks
//! it. `build.rs` and the crate's tests include this file by path, so the
//! rule is tested without a compiler in the loop.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What `prepare` found or did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The tree carries no `0001-ggml-cpu*` patch; it builds unpatched.
    NoPatch,
    /// `patch` is not installed; an owned tree builds unpatched.
    NoPatchTool,
    /// The patch was already applied (a supplied tree, or an owned one on
    /// a later build).
    AlreadyApplied,
    /// The patch was applied to the owned tree now.
    Applied,
}

/// The `0001-ggml-cpu*` patch under `third_party/ggml-patches`, when the
/// tree carries one.
pub fn cpu_patch(source: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(source.join("third_party/ggml-patches"))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("0001-ggml-cpu"))
        })
        .collect();
    found.sort();
    found.into_iter().next()
}

fn run_patch(ggml: &Path, patch: &Path, args: &[&str]) -> Result<bool, String> {
    let file = fs::File::open(patch).map_err(|e| format!("{}: {e}", patch.display()))?;
    match Command::new("patch")
        .args(args)
        .current_dir(ggml)
        .stdin(file)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) => Ok(status.success()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err("patch is not installed".into()),
        Err(e) => Err(format!("patch: {e}")),
    }
}

/// True when `patch` is already applied to `ggml`: reversing it dry-run
/// succeeds. Reads only.
pub fn is_applied(ggml: &Path, patch: &Path) -> Result<bool, String> {
    run_patch(ggml, patch, &["-p1", "-R", "--dry-run", "-s", "-f"])
}

/// Readies `source`'s ggml tree for the build. `owned` is true for the
/// tree this build fetched under `OUT_DIR`, which is patched in place;
/// a supplied tree is only checked, and `Err` names what to run when it
/// is not patched yet.
pub fn prepare(source: &Path, owned: bool) -> Result<Outcome, String> {
    let ggml = source.join("third_party/ggml");
    let Some(patch) = cpu_patch(source) else {
        return Ok(Outcome::NoPatch);
    };
    let applied = match is_applied(&ggml, &patch) {
        Ok(applied) => applied,
        Err(e) if owned && e == "patch is not installed" => return Ok(Outcome::NoPatchTool),
        Err(e) => {
            return Err(format!(
                "{e}; the supplied tree {} cannot be checked for the ggml CPU patch",
                source.display()
            ));
        }
    };
    if applied {
        return Ok(Outcome::AlreadyApplied);
    }
    if !owned {
        return Err(format!(
            "the supplied parakeet.cpp tree {} does not carry the ggml CPU patch and this build never writes into a caller's checkout; apply it first: cd {} && patch -p1 < {}",
            source.display(),
            ggml.display(),
            patch.display()
        ));
    }
    if run_patch(&ggml, &patch, &["-p1", "-N", "-s", "-r", "-"])? {
        Ok(Outcome::Applied)
    } else {
        Err(format!(
            "patch {} failed to apply under {}",
            patch.display(),
            ggml.display()
        ))
    }
}
