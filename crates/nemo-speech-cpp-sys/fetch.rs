//! Fetching the pinned archives: each is downloaded once into the archive
//! directory, verified against its SHA-256 and unpacked into its place
//! under the source root. A marker naming every commit makes a later build
//! a no-op.

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// One pinned GitHub archive.
pub struct Archive {
    /// The file name it is kept under.
    pub name: &'static str,
    /// Where it comes from.
    pub url: &'static str,
    /// Its checksum.
    pub sha256: &'static str,
    /// Where it unpacks, relative to the source root.
    pub into: &'static str,
}

/// Fetches, verifies and unpacks every archive under `root`; `archives`
/// holds the downloads.
pub fn sources(root: &Path, archives: &Path, pinned: &[Archive]) -> PathBuf {
    let marker = root.join(format!(
        ".unpacked-{}",
        pinned
            .iter()
            .map(|a| &a.sha256[..12])
            .collect::<Vec<_>>()
            .join("-")
    ));
    if marker.is_file() {
        return root.to_path_buf();
    }
    fs::create_dir_all(archives).expect("archive directory");
    if root.exists() {
        fs::remove_dir_all(root).expect("clean source directory");
    }
    for archive in pinned {
        let path = archives.join(archive.name);
        fetch(archive.url, &path, archive.sha256);
        let dir = root.join(archive.into);
        fs::create_dir_all(&dir).expect("unpack directory");
        untar(&path, &dir);
    }
    let urls: Vec<&str> = pinned.iter().map(|a| a.url).collect();
    fs::write(&marker, urls.join("\n")).expect("marker");
    root.to_path_buf()
}

/// Downloads `url` to `path` unless a file with the expected checksum is
/// already there; a checksum mismatch fails the build naming both hashes.
/// `DETTIVO_SOURCE_CACHE` names a directory read before the network and
/// given every verified download, filed under its checksum.
fn fetch(url: &str, path: &Path, sha256: &str) {
    if path.is_file() && sha256_of(path) == sha256 {
        return;
    }
    let cached = env::var_os("DETTIVO_SOURCE_CACHE").map(|dir| PathBuf::from(dir).join(sha256));
    if let Some(cached) = &cached {
        if cached.is_file() && sha256_of(cached) == sha256 {
            fs::copy(cached, path).unwrap_or_else(|e| panic!("{}: {e}", cached.display()));
            return;
        }
    }
    let part = path.with_extension("part");
    curl(url, &part);
    let have = sha256_of(&part);
    assert!(
        have == sha256,
        "{url} hashed {have}, expected {sha256}; left as {}",
        part.display()
    );
    fs::rename(&part, path).expect("move archive into place");
    if let Some(cached) = cached {
        keep(path, &cached);
    }
}

/// Runs curl. A github.com URL carries `GITHUB_TOKEN` (or `GH_TOKEN`) in a
/// header curl reads from stdin, so the token is in no argument list and
/// no message.
fn curl(url: &str, to: &Path) {
    let token = ["GITHUB_TOKEN", "GH_TOKEN"]
        .iter()
        .filter_map(|key| env::var(key).ok())
        .find(|token| !token.is_empty())
        .filter(|_| url.starts_with("https://github.com/"));
    let mut curl = Command::new("curl");
    curl.args(["-fsSL", "--retry", "3", "-o"]).arg(to).arg(url);
    if token.is_some() {
        curl.args(["-H", "@-"]).stdin(Stdio::piped());
    }
    let mut child = curl.spawn().unwrap_or_else(|e| panic!("curl {url}: {e}"));
    if let (Some(token), Some(mut stdin)) = (token, child.stdin.take()) {
        // A curl that exits before reading it fails through its status.
        let _ = writeln!(stdin, "Authorization: Bearer {token}");
    }
    let status = child.wait().unwrap_or_else(|e| panic!("curl {url}: {e}"));
    assert!(status.success(), "curl {url} failed with {status}");
}

/// Files a verified download in the source cache; a cache that cannot be
/// written warns and the build carries on.
fn keep(path: &Path, cached: &Path) {
    let part = cached.with_extension(format!("{}.part", std::process::id()));
    let stored = fs::create_dir_all(cached.parent().expect("cache directory"))
        .and_then(|()| fs::copy(path, &part))
        .and_then(|_| fs::rename(&part, cached));
    if let Err(e) = stored {
        let _ = fs::remove_file(&part);
        println!("cargo:warning=source cache {}: {e}", cached.display());
    }
}

fn sha256_of(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let mut file = fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf).expect("read archive");
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    format!("{:x}", hasher.finalize())
}

/// Unpacks a GitHub archive (one top-level directory) into `dir`.
fn untar(archive: &Path, dir: &Path) {
    let status = Command::new("tar")
        .args(["xzf"])
        .arg(archive)
        .args(["--strip-components=1", "-C"])
        .arg(dir)
        .status()
        .unwrap_or_else(|e| panic!("tar {}: {e}", archive.display()));
    assert!(
        status.success(),
        "tar {} failed with {status}",
        archive.display()
    );
}
