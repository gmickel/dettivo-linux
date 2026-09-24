use std::path::PathBuf;
use tempfile::NamedTempFile;

struct Pending {
    target: PathBuf,
    staged: NamedTempFile,
    original: Option<NamedTempFile>,
}

pub(super) fn publish(files: Vec<(PathBuf, PathBuf)>) -> Result<(), String> {
    let mut pending = Vec::new();
    for (target, source) in files {
        let parent = target.parent().ok_or("approval target has no parent")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let original = match std::fs::symlink_metadata(&target) {
            Ok(meta) if meta.file_type().is_file() => {
                let backup = NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
                std::fs::copy(&target, backup.path()).map_err(|e| e.to_string())?;
                Some(backup)
            }
            Ok(_) => {
                return Err(format!(
                    "{} is not a regular file; nothing approved",
                    target.display()
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(format!("{}: {e}; nothing approved", target.display())),
        };
        let staged = NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        std::fs::copy(source, staged.path()).map_err(|e| e.to_string())?;
        pending.push(Pending {
            target,
            staged,
            original,
        });
    }
    commit(pending)
}

fn commit(pending: Vec<Pending>) -> Result<(), String> {
    let mut changed: Vec<(PathBuf, Option<NamedTempFile>)> = Vec::new();
    for Pending {
        target,
        staged,
        original,
    } in pending
    {
        if let Err(e) = staged.persist(&target) {
            let mut errors = vec![format!("publish {}: {e}", target.display())];
            for (path, backup) in changed.into_iter().rev() {
                match backup {
                    Some(backup) => {
                        if let Err(e) = backup.persist(&path) {
                            let reason = e.error.to_string();
                            let recovery = e
                                .file
                                .keep()
                                .map(|(_, p)| p.display().to_string())
                                .unwrap_or_else(|e| format!("could not retain backup: {e}"));
                            errors.push(format!(
                                "rollback failed for {}: {reason}; original: {recovery}",
                                path.display()
                            ));
                        }
                    }
                    None => {
                        if let Err(e) = std::fs::remove_file(&path) {
                            errors.push(format!("rollback failed for {}: {e}", path.display()));
                        }
                    }
                }
            }
            if errors.len() == 1 {
                errors.push("all changes rolled back".into());
            }
            return Err(errors.join("; "));
        }
        changed.push((target, original));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_failure_restores_replaced_and_removes_created_files() {
        for existed in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let target = dir.path().join("first");
            let original = existed.then(|| {
                std::fs::write(&target, "old").unwrap();
                let backup = NamedTempFile::new_in(dir.path()).unwrap();
                std::fs::copy(&target, backup.path()).unwrap();
                backup
            });
            let staged = NamedTempFile::new_in(dir.path()).unwrap();
            std::fs::write(staged.path(), "new").unwrap();
            let blocked = dir.path().join("blocked");
            std::fs::create_dir(&blocked).unwrap();
            let error = commit(vec![
                Pending {
                    target: target.clone(),
                    staged,
                    original,
                },
                Pending {
                    target: blocked,
                    staged: NamedTempFile::new_in(dir.path()).unwrap(),
                    original: None,
                },
            ])
            .unwrap_err();
            assert!(error.contains("all changes rolled back"), "{error}");
            if existed {
                assert_eq!(std::fs::read_to_string(target).unwrap(), "old");
            } else {
                assert!(!target.exists());
            }
        }
    }
}
