//! Models on disk (ADR 0005): `<models>/<provider>/<id>/<file>` beside a
//! `manifest.json` that records the checksum the file was verified
//! against, readiness per model, verification with quarantine on a
//! mismatch (FR-P3), and deletion. A model set (the diarization models,
//! ADR 0035) is several files in the one directory; readiness, the
//! verification and the deletion cover every file, and the engine loads
//! the directory.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::catalogue::{Catalogue, ModelEntry, ModelFile};

/// Where a model stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum Readiness {
    /// On disk and verified.
    Ready,
    /// On disk, not verified since the file appeared.
    Unverified,
    /// A download runs.
    Downloading {
        /// Bytes on disk so far.
        bytes_done: u64,
        /// Bytes expected.
        bytes_total: u64,
    },
    /// A partial file waits for a resumed download.
    Partial {
        /// Bytes on disk so far.
        bytes_done: u64,
    },
    /// Not on disk.
    Missing,
    /// The file failed verification and was moved aside.
    Quarantined,
}

/// One final file of a set as it was when it hashed to the catalogue's
/// checksum: the name, that checksum (the extracted member's for an
/// archive) and the size and modification time the file had, so a
/// catalogue that changes a checksum or a file that is replaced in place
/// is no longer verified and is hashed again before an engine loads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedFile {
    /// The file name inside the model directory.
    pub file_name: String,
    /// The checksum the file on disk hashed to.
    pub sha256: String,
    /// The size the file had when it hashed.
    pub size_bytes: u64,
    /// The modification time it had then (unix nanoseconds).
    pub modified_unix_nanos: u64,
}

/// The per-model manifest beside the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// The provider.
    pub provider: String,
    /// The model id.
    pub id: String,
    /// The checksum the file is expected to have.
    pub sha256: String,
    /// The size the catalogue lists.
    pub size_bytes: u64,
    /// When the file was downloaded or first seen (unix seconds).
    pub downloaded_at: u64,
    /// The catalogue version the entry came from.
    pub catalogue_version: u32,
    /// The file hashed to `sha256` after it landed.
    #[serde(default)]
    pub verified: bool,
    /// Every final file of the set as it was when it verified; a manifest
    /// without them (written before this field existed) counts as not
    /// verified and is hashed once more.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<VerifiedFile>,
}

/// The size and modification time of a file, as the manifest records
/// them; `None` when the file cannot be read.
fn identity_of(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some((meta.len(), u64::try_from(modified).unwrap_or(u64::MAX)))
}

/// The model directory with the catalogue that describes it.
#[derive(Debug, Clone)]
pub struct ModelStore {
    root: PathBuf,
    catalogue: Catalogue,
}

impl ModelStore {
    /// A store rooted at `root`.
    pub fn new(root: impl Into<PathBuf>, catalogue: Catalogue) -> Self {
        Self {
            root: root.into(),
            catalogue,
        }
    }

    /// The models directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The catalogue.
    pub fn catalogue(&self) -> &Catalogue {
        &self.catalogue
    }

    /// `<models>/<provider>/<id>/`.
    pub fn dir(&self, entry: &ModelEntry) -> PathBuf {
        self.root.join(&entry.provider).join(&entry.id)
    }

    /// The model file (the first of a model set).
    pub fn path(&self, entry: &ModelEntry) -> PathBuf {
        self.dir(entry).join(&entry.file_name)
    }

    /// What the engine loads: the directory for a model set, else the file.
    pub fn load_path(&self, entry: &ModelEntry) -> PathBuf {
        if entry.loads_directory() {
            self.dir(entry)
        } else {
            self.path(entry)
        }
    }

    /// One file of the set.
    pub fn file_path(&self, entry: &ModelEntry, file: &ModelFile) -> PathBuf {
        self.dir(entry).join(&file.file_name)
    }

    /// The partial download beside one file of the set.
    pub fn file_part_path(&self, entry: &ModelEntry, file: &ModelFile) -> PathBuf {
        self.dir(entry).join(format!("{}.part", file.file_name))
    }

    /// The sidecar that records what one file's partial download fetched.
    pub fn file_part_meta_path(&self, entry: &ModelEntry, file: &ModelFile) -> PathBuf {
        self.dir(entry)
            .join(format!("{}.part.json", file.file_name))
    }

    /// The partial download beside the model file.
    pub fn part_path(&self, entry: &ModelEntry) -> PathBuf {
        self.dir(entry).join(format!("{}.part", entry.file_name))
    }

    /// The manifest beside the model file.
    pub fn manifest_path(&self, entry: &ModelEntry) -> PathBuf {
        self.dir(entry).join("manifest.json")
    }

    /// Reads the manifest, if any.
    pub fn manifest(&self, entry: &ModelEntry) -> Option<Manifest> {
        let text = std::fs::read_to_string(self.manifest_path(entry)).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Writes the manifest for `entry`; a verified one records every
    /// final file as it is on disk right now.
    pub fn write_manifest(&self, entry: &ModelEntry, verified: bool) -> std::io::Result<()> {
        let files = if verified {
            entry
                .files()
                .iter()
                .filter_map(|f| {
                    let (size_bytes, modified_unix_nanos) = identity_of(&self.file_path(entry, f))?;
                    Some(VerifiedFile {
                        file_name: f.file_name.clone(),
                        sha256: f.disk_sha256().to_string(),
                        size_bytes,
                        modified_unix_nanos,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        let manifest = Manifest {
            provider: entry.provider.clone(),
            id: entry.id.clone(),
            sha256: entry.sha256.clone(),
            size_bytes: entry.size_bytes,
            downloaded_at: now(),
            catalogue_version: self.catalogue.version,
            verified,
            files,
        };
        std::fs::create_dir_all(self.dir(entry))?;
        let text = serde_json::to_string_pretty(&manifest).unwrap_or_default();
        std::fs::write(self.manifest_path(entry), text + "\n")
    }

    /// Readiness from the disk alone (a running download is layered on by
    /// the caller that owns it). A model set is ready when every file is
    /// on disk and verified, which holds only while the manifest carries
    /// every file's identity under the catalogue's checksums and the
    /// files still have the size and time they verified with; with some
    /// files landed or a partial download waiting it is `Partial` with
    /// the bytes that are there.
    pub fn readiness(&self, entry: &ModelEntry) -> Readiness {
        let files = entry.files();
        let present = files
            .iter()
            .filter(|f| self.file_path(entry, f).is_file())
            .count();
        if present == files.len() {
            let verified = self.manifest(entry).is_some_and(|m| {
                m.verified && m.sha256 == entry.sha256 && self.identities_hold(entry, &m)
            });
            return if verified {
                Readiness::Ready
            } else {
                Readiness::Unverified
            };
        }
        let mut bytes_done = 0;
        let mut waiting = false;
        for f in &files {
            if self.file_path(entry, f).is_file() {
                bytes_done += f.size_bytes;
                waiting = true;
            } else if let Ok(meta) = std::fs::metadata(self.file_part_path(entry, f)) {
                bytes_done += meta.len();
                waiting = true;
            }
        }
        if waiting {
            Readiness::Partial { bytes_done }
        } else if self.quarantine_dir(entry).is_dir() {
            Readiness::Quarantined
        } else {
            Readiness::Missing
        }
    }

    /// Every final file of `entry` verified under the checksum the
    /// catalogue names now, and still has the size and time it had then.
    fn identities_hold(&self, entry: &ModelEntry, manifest: &Manifest) -> bool {
        entry.files().iter().all(|f| {
            manifest
                .files
                .iter()
                .find(|v| v.file_name == f.file_name)
                .is_some_and(|v| {
                    v.sha256 == f.disk_sha256()
                        && identity_of(&self.file_path(entry, f))
                            == Some((v.size_bytes, v.modified_unix_nanos))
                })
        })
    }

    /// Hashes every file on disk and records the result: `Ready` when all
    /// match, or the set is quarantined and `Quarantined` returned. A set
    /// with a file missing answers its readiness.
    pub fn verify(&self, entry: &ModelEntry) -> std::io::Result<Readiness> {
        let files = entry.files();
        if files.iter().any(|f| !self.file_path(entry, f).is_file()) {
            return Ok(self.readiness(entry));
        }
        for f in &files {
            let have = sha256_of(&self.file_path(entry, f))?;
            if have != f.disk_sha256() {
                tracing::warn!(
                    provider = %entry.provider,
                    model = %entry.id,
                    file = %f.file_name,
                    "model checksum mismatch; quarantined"
                );
                self.quarantine_all(entry)?;
                return Ok(Readiness::Quarantined);
            }
        }
        self.write_manifest(entry, true)?;
        Ok(Readiness::Ready)
    }

    /// Moves `file` (and the manifest) into the quarantine directory and
    /// returns where it went.
    pub fn quarantine(&self, entry: &ModelEntry, file: &Path) -> std::io::Result<PathBuf> {
        let dir = self.quarantine_dir(entry).join(now().to_string());
        std::fs::create_dir_all(&dir)?;
        if let Some(name) = file.file_name() {
            std::fs::rename(file, dir.join(name))?;
        }
        let manifest = self.manifest_path(entry);
        if manifest.is_file() {
            let _ = std::fs::rename(&manifest, dir.join("manifest.json"));
        }
        Ok(dir)
    }

    /// Moves every file of the set that is on disk (and the manifest)
    /// into one quarantine directory.
    pub fn quarantine_all(&self, entry: &ModelEntry) -> std::io::Result<PathBuf> {
        let dir = self.quarantine_dir(entry).join(now().to_string());
        std::fs::create_dir_all(&dir)?;
        for f in entry.files() {
            let path = self.file_path(entry, &f);
            if path.is_file() {
                std::fs::rename(&path, dir.join(&f.file_name))?;
            }
        }
        let manifest = self.manifest_path(entry);
        if manifest.is_file() {
            let _ = std::fs::rename(&manifest, dir.join("manifest.json"));
        }
        Ok(dir)
    }

    /// `<models>/quarantine/<provider>/<id>/`.
    pub fn quarantine_dir(&self, entry: &ModelEntry) -> PathBuf {
        self.root
            .join("quarantine")
            .join(&entry.provider)
            .join(&entry.id)
    }

    /// How many quarantined copies exist across every model.
    pub fn quarantined_count(&self) -> usize {
        self.catalogue
            .models
            .iter()
            .filter(|m| self.quarantine_dir(m).is_dir())
            .count()
    }

    /// Removes every file of the set, their partial downloads and the
    /// manifest.
    pub fn delete(&self, entry: &ModelEntry) -> std::io::Result<bool> {
        let mut removed = false;
        let mut paths = vec![self.manifest_path(entry)];
        for f in entry.files() {
            paths.push(self.file_path(entry, &f));
            paths.push(self.file_part_path(entry, &f));
            paths.push(self.file_part_meta_path(entry, &f));
        }
        for p in paths {
            if p.exists() {
                std::fs::remove_file(&p)?;
                removed = true;
            }
        }
        let _ = std::fs::remove_dir(self.dir(entry));
        Ok(removed)
    }

    /// The sidecar that records what a partial download was fetching.
    pub fn part_meta_path(&self, entry: &ModelEntry) -> PathBuf {
        self.dir(entry)
            .join(format!("{}.part.json", entry.file_name))
    }

    /// Verifies every model on disk that is not verified yet; returns the
    /// quarantined ones.
    pub fn verify_all(&self) -> Vec<String> {
        let mut quarantined = Vec::new();
        for entry in &self.catalogue.models {
            if self.readiness(entry) == Readiness::Unverified {
                match self.verify(entry) {
                    Ok(Readiness::Quarantined) => {
                        quarantined.push(format!("{}/{}", entry.provider, entry.id))
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!(model = %entry.id, error = %e, "verification failed"),
                }
            }
        }
        quarantined
    }
}

/// The SHA-256 of a file, streamed.
pub fn sha256_of(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "models_tests.rs"]
mod tests;
