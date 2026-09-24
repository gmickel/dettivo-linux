//! The gate every catalogue-backed load passes: a model that verified
//! loads, one that has not been hashed since its file appeared (or since
//! the catalogue or the file changed) is hashed now, and a quarantined or
//! missing one is refused by name with the download command. A path the
//! catalogue does not know (a private sideload, ADR 0032) passes as it
//! is; that policy stays with the sideload itself.

use std::path::Path;
use std::sync::{Arc, Mutex};

use dettivo_speech::catalogue::ModelEntry;
use dettivo_speech::models::{ModelStore, Readiness};

/// The gate as the engine services hold it, over the store `store_of`
/// answers now (the configuration can move the directory or the
/// catalogue). `ensure` is what a load calls: it hashes an unverified
/// model first, one hash at a time, so a preload and a session start
/// that meet on the same model hash it once. `check` is what a status
/// query calls: the same verdict without a hash, so `system.capabilities`
/// and a probe never wait behind gigabytes of hashing.
#[derive(Clone)]
pub struct Verifier {
    store_of: Arc<dyn Fn() -> ModelStore + Send + Sync>,
    hashing: Arc<Mutex<()>>,
}

impl Verifier {
    /// `Ok` when the file at `path` may load now, hashing it first when it
    /// has not verified since it appeared or changed; else why not.
    pub fn ensure(&self, path: &Path) -> Result<(), String> {
        ensure_verified(&(self.store_of)(), &self.hashing, path)
    }

    /// `Ok` when the file at `path` is on disk and not quarantined; an
    /// unverified model passes here and is hashed by the load that opens
    /// it. Never hashes.
    pub fn check(&self, path: &Path) -> Result<(), String> {
        check_readiness(&(self.store_of)(), path)
    }
}

/// The gate over `store_of`.
pub fn verifier(store_of: impl Fn() -> ModelStore + Send + Sync + 'static) -> Verifier {
    Verifier {
        store_of: Arc::new(store_of),
        hashing: Arc::new(Mutex::new(())),
    }
}

/// The catalogue entry whose file or directory is `path`, when there is one.
fn entry_at<'a>(store: &'a ModelStore, path: &Path) -> Option<&'a ModelEntry> {
    store
        .catalogue()
        .models
        .iter()
        .find(|e| store.load_path(e) == path || store.path(e) == path)
}

/// The verdict on a readiness the caller already has; `Unverified` is
/// the caller's to hash or to pass.
fn verdict(entry: &ModelEntry, readiness: Readiness) -> Result<(), String> {
    let name = format!("{}/{}", entry.provider, entry.id);
    let fetch = crate::engines::download_command(&entry.provider, &entry.id);
    match readiness {
        Readiness::Ready | Readiness::Unverified => Ok(()),
        Readiness::Quarantined => Err(format!(
            "model {name} failed verification and is quarantined; run `{fetch}`"
        )),
        _ => Err(format!("model {name} is not downloaded; run `{fetch}`")),
    }
}

/// The readiness verdict without a hash: an unverified model passes.
pub fn check_readiness(store: &ModelStore, path: &Path) -> Result<(), String> {
    match entry_at(store, path) {
        Some(entry) => verdict(entry, store.readiness(entry)),
        None => Ok(()),
    }
}

/// Verifies the catalogue model whose file or directory is `path`, when
/// there is one and it is not verified yet.
pub fn ensure_verified(store: &ModelStore, hashing: &Mutex<()>, path: &Path) -> Result<(), String> {
    let Some(entry) = entry_at(store, path) else {
        return Ok(());
    };
    let name = format!("{}/{}", entry.provider, entry.id);
    let readiness = match store.readiness(entry) {
        Readiness::Unverified => {
            let _one_at_a_time = hashing.lock().unwrap_or_else(|p| p.into_inner());
            if store.readiness(entry) == Readiness::Ready {
                return Ok(());
            }
            tracing::info!(model = %name, "hashing the model before it loads");
            let hashed = store
                .verify(entry)
                .map_err(|e| format!("model {name} cannot be verified: {e}"))?;
            if hashed == Readiness::Unverified {
                // A hash that left it unverified is a refusal, never a load.
                return Err(format!("model {name} did not verify"));
            }
            hashed
        }
        other => other,
    };
    verdict(entry, readiness)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dettivo_speech::catalogue::Catalogue;
    use sha2::Digest;

    fn store(dir: &Path) -> ModelStore {
        let sha = format!("{:x}", sha2::Sha256::digest(b"weights"));
        let catalogue = Catalogue::parse(&format!(
            r#"version = 1

[[models]]
provider = "whisper"
id = "tiny"
display_name = "Tiny"
kind = "stt"
file_name = "ggml-tiny.bin"
size_bytes = 7
url = "http://127.0.0.1:1/ggml-tiny.bin"
sha256 = "{sha}"
license = "MIT"
redistribution = "test fixture"
languages = ["en"]
default = true
"#
        ))
        .unwrap();
        ModelStore::new(dir, catalogue)
    }

    #[test]
    fn an_unverified_model_is_hashed_before_it_loads_and_a_bad_one_is_refused_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        let hashing = Mutex::new(());
        let entry = store.catalogue().find("whisper", "tiny").unwrap().clone();
        let path = store.path(&entry);
        // Not on disk: refused with the download command.
        let err = ensure_verified(&store, &hashing, &path).unwrap_err();
        assert!(err.contains("whisper/tiny is not downloaded"), "{err}");
        assert!(err.contains("dettivo speech download"), "{err}");
        assert!(
            check_readiness(&store, &path).is_err(),
            "check refuses a missing model too"
        );
        // On disk without a manifest: a status check passes without a
        // hash and leaves it unverified; the load hashes it, then it is
        // ready.
        std::fs::create_dir_all(store.dir(&entry)).unwrap();
        std::fs::write(&path, b"weights").unwrap();
        assert_eq!(store.readiness(&entry), Readiness::Unverified);
        check_readiness(&store, &path).unwrap();
        assert_eq!(
            store.readiness(&entry),
            Readiness::Unverified,
            "a check never hashes"
        );
        ensure_verified(&store, &hashing, &path).unwrap();
        assert_eq!(store.readiness(&entry), Readiness::Ready);
        // Replaced with something else: hashed again and quarantined,
        // so the engine never opens it.
        std::thread::sleep(std::time::Duration::from_millis(5));
        std::fs::write(&path, b"corrupt").unwrap();
        let err = ensure_verified(&store, &hashing, &path).unwrap_err();
        assert!(err.contains("whisper/tiny failed verification"), "{err}");
        assert!(!path.exists(), "the corrupt file is moved aside");
        let err = check_readiness(&store, &path).unwrap_err();
        assert!(err.contains("quarantined"), "{err}");
        // A path the catalogue does not know passes as it is.
        ensure_verified(&store, &hashing, Path::new("/nowhere/private.gguf")).unwrap();
    }
}
