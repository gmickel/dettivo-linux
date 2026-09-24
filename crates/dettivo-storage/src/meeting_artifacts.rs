//! The meeting directories beside the database: `<data>/meetings/<id>/`
//! holds the takes, the take sidecars, the journal, the live checkpoint
//! and `metadata.json`; a delete removes the directory, a lighter policy
//! the audio alone.

use std::path::{Path, PathBuf};

use crate::StoreError;

/// The meeting directories under `<data>/meetings/`.
#[derive(Debug, Clone)]
pub struct MeetingArtifacts {
    root: PathBuf,
}

impl MeetingArtifacts {
    /// Meeting directories under `root`.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// The root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// One meeting's directory.
    pub fn dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    /// Removes a meeting's directory; a missing one is fine.
    pub fn remove(&self, id: &str) -> Result<(), StoreError> {
        match std::fs::remove_dir_all(self.dir(id)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Removes the audio files of a meeting's directory and keeps the
    /// rest (journal, sidecars, metadata).
    pub fn remove_audio(&self, id: &str) -> Result<(), StoreError> {
        self.remove_matching(id, |name| name.ends_with(".wav"))
    }

    /// Removes every take and the take sidecars (`takes.json`,
    /// `system-takes.json`) of a meeting's directory and keeps the rest
    /// (journal, notes, analysis, metadata): the `transcript_and_audio`
    /// delete policy.
    pub fn remove_audio_and_sidecars(&self, id: &str) -> Result<(), StoreError> {
        self.remove_matching(id, |name| {
            name.ends_with(".wav") || name == "takes.json" || name.ends_with("-takes.json")
        })
    }

    /// Only a missing directory counts as nothing to remove; a directory
    /// that cannot be read or an entry that cannot be listed is the
    /// caller's error, so the facts it clears afterwards stay true.
    fn remove_matching(&self, id: &str, keep: impl Fn(&str) -> bool) -> Result<(), StoreError> {
        let dir = self.dir(id);
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        for entry in entries {
            let path = entry?.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if path.is_file() && keep(name) {
                std::fs::remove_file(&path)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// meetings/F11 (fn-43): a directory that cannot be read is an error,
    /// not a successful removal; a missing one is nothing to remove.
    #[test]
    fn an_unreadable_directory_is_an_error_and_a_missing_one_is_fine() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let root = tempfile::tempdir().unwrap();
        let artifacts = MeetingArtifacts::new(root.path().to_path_buf());
        assert!(artifacts.remove_audio("missing").is_ok());
        let dir = artifacts.dir("m");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("microphone.wav"), b"wav").unwrap();
        if std::fs::metadata("/proc/self")
            .map(|m| m.uid())
            .unwrap_or(1)
            == 0
        {
            eprintln!("skipped: root reads every directory");
            return;
        }
        std::fs::set_permissions(&dir, PermissionsExt::from_mode(0o300)).unwrap();
        let refused = artifacts.remove_audio("m");
        std::fs::set_permissions(&dir, PermissionsExt::from_mode(0o755)).unwrap();
        assert!(refused.is_err(), "an unreadable directory is not a removal");
        assert!(dir.join("microphone.wav").is_file(), "the take stays");
        artifacts.remove_audio("m").unwrap();
        assert!(!dir.join("microphone.wav").exists());
    }
}
