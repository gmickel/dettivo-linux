//! Locate repository and model data for the QA CLI.
use std::path::{Path, PathBuf};

pub(crate) fn repo_root(flag: Option<PathBuf>) -> PathBuf {
    if let Some(r) = flag {
        return r;
    }
    let mut dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    for _ in 0..5 {
        if dir.join("Cargo.toml").exists() && dir.join("crates").is_dir() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub(crate) fn models_dir() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let dir = data.join("dettivo/models");
    dir.is_dir().then_some(dir)
}
