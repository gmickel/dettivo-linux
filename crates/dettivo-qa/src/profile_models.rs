//! A profile's private models directory (fn-41 R3): every drive, pack
//! and bench profile carries its own `models/` tree with only the models
//! it names linked in from the real directory, never a link to the
//! user's directory. A daemon under test may delete or rewrite what it
//! finds under its models directory (`speech.models.delete` does, and
//! the verification rewrites `manifest.json` in place), so the tree
//! shares only what nothing writes: the weight files are hard links
//! (unlinking one leaves the real file where it was), and every file the
//! daemon writes into, the manifest, a partial download and its sidecar,
//! is a copy. The tree is built under the repository's `target/` because
//! the profile root lives on `/tmp` (short socket paths), which is a
//! tmpfs on this desktop where a hard link across filesystems cannot
//! exist and a copy of a large model would fill the quota.

use std::path::{Path, PathBuf};

/// The models every profile carries when the real directory has them:
/// the test model and the fixture clip beside it.
pub const TEST_MODELS: &[&str] = &["whisper/tiny.en", "fixtures"];

/// Where a profile's models come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSource {
    /// The real directory (`$XDG_DATA_HOME/dettivo/models`), read only.
    pub real: PathBuf,
    /// Where the private trees are built: one directory per profile,
    /// on the real directory's filesystem so every link is a hard link.
    pub store: PathBuf,
}

impl ModelSource {
    /// The real directory with the store under the repository's `target/`.
    pub fn new(real: PathBuf, repo_root: &Path) -> Self {
        Self {
            real,
            store: repo_root.join("target/qa-models"),
        }
    }
}

/// One profile's private tree.
#[derive(Debug)]
pub struct ModelTree {
    /// The tree the profile's `data/dettivo/models` points at.
    pub dir: PathBuf,
    source: Option<ModelSource>,
    linked: Vec<String>,
}

impl ModelTree {
    /// Builds the private tree for the profile at `root` and points
    /// `<root>/data/dettivo/models` at it; without a source the tree is
    /// empty and every named model is refused. The test models that
    /// exist are linked in; a missing one is a precondition the scenario
    /// reports, not an error here.
    pub fn create(root: &Path, source: Option<&ModelSource>) -> std::io::Result<Self> {
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "profile".into());
        let dir = match source {
            Some(s) => s.store.join(&name),
            None => root.join("data/dettivo/models-private"),
        };
        if let Some(s) = source {
            guard(&dir, &s.real).map_err(std::io::Error::other)?;
        }
        std::fs::create_dir_all(&dir)?;
        let link = root.join("data/dettivo");
        std::fs::create_dir_all(&link)?;
        std::os::unix::fs::symlink(&dir, link.join("models"))?;
        let mut tree = Self {
            dir,
            source: source.cloned(),
            linked: Vec::new(),
        };
        if let Some(s) = source {
            for name in TEST_MODELS {
                if s.real.join(name).exists() {
                    tree.link(name).map_err(std::io::Error::other)?;
                }
            }
        }
        Ok(tree)
    }

    /// Links `name` (`whisper/tiny.en`, `diarize/diarization`,
    /// `llm/qwen3-1.7b`, `fixtures`) from the real directory into the
    /// tree: every file under it as a hard link, a copy where the two
    /// filesystems differ. A name the real directory does not carry is
    /// refused by name with the command that fetches it.
    pub fn link(&mut self, name: &str) -> Result<PathBuf, String> {
        let target = self.dir.join(name);
        if self.linked.iter().any(|l| l == name) {
            return Ok(target);
        }
        let Some(source) = &self.source else {
            return Err(format!(
                "the profile carries no models directory, so `{name}` cannot be linked ({})",
                hint(name)
            ));
        };
        let from = source.real.join(name);
        if !from.exists() {
            return Err(format!(
                "`{name}` is not under {} ({})",
                source.real.display(),
                hint(name)
            ));
        }
        link_tree(&from, &target).map_err(|e| format!("link `{name}`: {e}"))?;
        self.linked.push(name.to_string());
        Ok(target)
    }

    /// The names linked so far, in order.
    pub fn linked(&self) -> &[String] {
        &self.linked
    }

    /// Removes the tree (its hard links, never the real files).
    pub fn remove(&self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The command that fetches a model by name.
fn hint(name: &str) -> String {
    match name.split('/').collect::<Vec<_>>().as_slice() {
        ["whisper", "tiny.en"] | ["fixtures"] => "scripts/models/fetch-test-model.sh".into(),
        ["diarize", _] => "scripts/models/fetch-diarization-model.sh".into(),
        ["llm", id] => format!("dettivo llm download --model {id}"),
        [provider, id] => format!("dettivo speech download --provider {provider} --model {id}"),
        _ => "scripts/models/fetch-test-model.sh".into(),
    }
}

/// A file the daemon may write into in place (the manifest the
/// verification rewrites, a partial download it resumes, the sidecar
/// beside it): shared as a copy, never as a hard link.
fn written_in_place(name: &str) -> bool {
    name == "manifest.json" || name.ends_with(".part") || name.ends_with(".part.json")
}

fn link_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)?.flatten() {
            link_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let name = to.file_name().map(|n| n.to_string_lossy().into_owned());
    if name.as_deref().is_some_and(written_in_place) || std::fs::hard_link(from, to).is_err() {
        std::fs::copy(from, to)?;
    }
    Ok(())
}

/// The preflight: the tree a profile's `models` link points at must
/// never be the real directory or anything under it, whichever way the
/// paths are spelled. A tree that does not exist yet resolves through
/// its deepest existing ancestor, so a parent that is a symlink into the
/// real directory is caught before anything is created there. Refused
/// by path; a path that cannot be resolved is refused too.
pub fn guard(tree: &Path, real: &Path) -> Result<(), String> {
    let tree_resolved = resolve_new(tree)?;
    let real_resolved = resolve_new(real)?;
    if tree.starts_with(real) || tree_resolved.starts_with(&real_resolved) {
        return Err(format!(
            "the profile's models directory {} is the real models directory {} or under it; a drive never links a real user directory",
            tree.display(),
            real.display()
        ));
    }
    Ok(())
}

/// `path` with every symlink resolved: the deepest ancestor that exists
/// is canonicalized and the components below it, which need not exist
/// yet, are appended as spelled.
fn resolve_new(path: &Path) -> Result<PathBuf, String> {
    let mut missing = Vec::new();
    let mut cursor = path.to_path_buf();
    loop {
        match std::fs::canonicalize(&cursor) {
            Ok(base) => {
                let mut out = base;
                for component in missing.iter().rev() {
                    out.push(component);
                }
                return Ok(out);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let Some(name) = cursor.file_name().map(|n| n.to_os_string()) else {
                    return Err(format!("{} cannot be resolved", path.display()));
                };
                missing.push(name);
                if !cursor.pop() {
                    return Err(format!("{} cannot be resolved", path.display()));
                }
            }
            Err(e) => return Err(format!("{} cannot be resolved: {e}", path.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real_dir() -> tempfile::TempDir {
        let real = tempfile::Builder::new()
            .prefix("dq-models-real-")
            .tempdir_in("/tmp")
            .unwrap();
        for file in [
            "whisper/tiny.en/ggml-tiny.en.bin",
            "whisper/tiny.en/manifest.json",
            "whisper/large-v3-turbo/ggml-large-v3-turbo.bin",
            "fixtures/jfk.wav",
            "diarize/diarization/segmentation.onnx",
        ] {
            let path = real.path().join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, file.as_bytes()).unwrap();
        }
        real
    }

    #[test]
    fn the_tree_carries_only_the_named_models_as_hard_links_and_never_the_real_directory() {
        let real = real_dir();
        let store = tempfile::Builder::new()
            .prefix("dq-models-store-")
            .tempdir_in("/tmp")
            .unwrap();
        let root = tempfile::Builder::new()
            .prefix("dqtest-")
            .tempdir_in("/tmp")
            .unwrap();
        let source = ModelSource {
            real: real.path().to_path_buf(),
            store: store.path().to_path_buf(),
        };
        let mut tree = ModelTree::create(root.path(), Some(&source)).unwrap();
        let link = root.path().join("data/dettivo/models");
        assert_eq!(std::fs::read_link(&link).unwrap(), tree.dir);
        assert_ne!(
            std::fs::canonicalize(&link).unwrap(),
            real.path().canonicalize().unwrap()
        );
        assert!(link.join("whisper/tiny.en/ggml-tiny.en.bin").is_file());
        assert!(link.join("fixtures/jfk.wav").is_file());
        assert!(
            !link.join("whisper/large-v3-turbo").exists(),
            "a model nobody named stays out"
        );
        let real_meta =
            std::fs::metadata(real.path().join("whisper/tiny.en/ggml-tiny.en.bin")).unwrap();
        assert_eq!(
            std::os::unix::fs::MetadataExt::nlink(&real_meta),
            2,
            "hard-linked"
        );

        let named = tree.link("whisper/large-v3-turbo").unwrap();
        assert!(named.join("ggml-large-v3-turbo.bin").is_file());
        assert_eq!(
            tree.link("whisper/large-v3-turbo").unwrap(),
            named,
            "idempotent"
        );
        assert_eq!(
            tree.linked(),
            ["whisper/tiny.en", "fixtures", "whisper/large-v3-turbo"]
        );
        let refused = tree.link("llm/qwen3-1.7b").unwrap_err();
        assert!(
            refused.contains("`llm/qwen3-1.7b` is not under")
                && refused.contains("dettivo llm download --model qwen3-1.7b"),
            "{refused}"
        );

        // What the daemon writes in place stays inside the profile: the
        // manifest is a copy, rewritten and truncated here without the
        // real one moving.
        let real_manifest = real.path().join("whisper/tiny.en/manifest.json");
        let tree_manifest = link.join("whisper/tiny.en/manifest.json");
        assert_eq!(
            std::os::unix::fs::MetadataExt::nlink(&std::fs::metadata(&real_manifest).unwrap()),
            1,
            "the manifest is copied, not linked"
        );
        std::fs::write(&tree_manifest, b"{\"verified\":false}").unwrap();
        std::fs::write(&tree_manifest, b"").unwrap();
        assert_eq!(
            std::fs::read_to_string(&real_manifest).unwrap(),
            "whisper/tiny.en/manifest.json",
            "the real manifest keeps its bytes"
        );
        assert_eq!(
            std::os::unix::fs::MetadataExt::nlink(&real_meta),
            2,
            "the weights stay shared"
        );

        // What the daemon deletes under the profile stays real elsewhere.
        std::fs::remove_dir_all(link.join("whisper/tiny.en")).unwrap();
        assert!(
            real.path()
                .join("whisper/tiny.en/ggml-tiny.en.bin")
                .is_file()
        );
        tree.remove();
        assert!(!tree.dir.exists());
        assert!(real.path().join("fixtures/jfk.wav").is_file());
    }

    #[test]
    fn a_tree_that_is_the_real_directory_or_under_it_is_refused_by_path() {
        let real = real_dir();
        let refused = guard(real.path(), real.path()).unwrap_err();
        assert!(
            refused.contains(&real.path().display().to_string()),
            "{refused}"
        );
        assert!(guard(&real.path().join("whisper"), real.path()).is_err());
        // The same directory reached through a symlink is still refused.
        let alias = tempfile::Builder::new()
            .prefix("dq-models-alias-")
            .tempdir_in("/tmp")
            .unwrap();
        std::os::unix::fs::symlink(real.path(), alias.path().join("models")).unwrap();
        assert!(guard(&alias.path().join("models"), real.path()).is_err());
        // A tree that does not exist yet under a symlinked parent is
        // refused before it is created inside the real directory.
        assert!(
            guard(&alias.path().join("models/qa/dqtest-new"), real.path()).is_err(),
            "a new child of a symlink into the real directory"
        );
        // A symlinked store elsewhere stays usable.
        let elsewhere = tempfile::Builder::new()
            .prefix("dq-store-real-")
            .tempdir_in("/tmp")
            .unwrap();
        std::os::unix::fs::symlink(elsewhere.path(), alias.path().join("store")).unwrap();
        assert!(guard(&alias.path().join("store/dqtest-new"), real.path()).is_ok());
        assert!(guard(Path::new("/tmp/dq-store/dqtest-abc"), real.path()).is_ok());
        // A store inside the real directory is refused before anything is built.
        let source = ModelSource {
            real: real.path().to_path_buf(),
            store: real.path().join("qa"),
        };
        let root = tempfile::Builder::new()
            .prefix("dqtest-")
            .tempdir_in("/tmp")
            .unwrap();
        let err = ModelTree::create(root.path(), Some(&source)).unwrap_err();
        assert!(err.to_string().contains("real models directory"), "{err}");
        assert!(!root.path().join("data/dettivo/models").exists());
    }

    #[test]
    fn without_a_source_the_tree_is_empty_and_every_name_is_refused() {
        let root = tempfile::Builder::new()
            .prefix("dqtest-")
            .tempdir_in("/tmp")
            .unwrap();
        let mut tree = ModelTree::create(root.path(), None).unwrap();
        assert!(root.path().join("data/dettivo/models").is_dir());
        assert!(tree.linked().is_empty());
        let refused = tree.link("diarize/diarization").unwrap_err();
        assert!(refused.contains("fetch-diarization-model.sh"), "{refused}");
    }
}
