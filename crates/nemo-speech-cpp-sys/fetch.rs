//! Fetching the pinned archives: each is downloaded once into the archive
//! directory, verified against its SHA-256 and unpacked into its place
//! under the source root. A marker naming every commit makes a later build
//! a no-op.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

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
fn fetch(url: &str, path: &Path, sha256: &str) {
    if path.is_file() && sha256_of(path) == sha256 {
        return;
    }
    let part = path.with_extension("part");
    let status = Command::new("curl")
        .args(["-fsSL", "--retry", "3", "-o"])
        .arg(&part)
        .arg(url)
        .status()
        .unwrap_or_else(|e| panic!("curl {url}: {e}"));
    assert!(status.success(), "curl {url} failed with {status}");
    let have = sha256_of(&part);
    assert!(
        have == sha256,
        "{url} hashed {have}, expected {sha256}; left as {}",
        part.display()
    );
    fs::rename(&part, path).expect("move archive into place");
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
