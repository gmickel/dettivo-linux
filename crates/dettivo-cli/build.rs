//! Records the git commit `dettivo --version` prints beside the package
//! version: `DETTIVO_GIT_SHA` from the environment when the packaging sets
//! it (a release tarball has no `.git`), otherwise `git rev-parse --short
//! HEAD`, otherwise `unknown`.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=DETTIVO_GIT_SHA");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    let sha = std::env::var("DETTIVO_GIT_SHA")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "--short=12", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=DETTIVO_GIT_SHA={sha}");
}
