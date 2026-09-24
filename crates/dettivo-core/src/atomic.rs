//! Atomic file replacement: a reader never sees a half-written file, and a
//! crash mid-write leaves the previous contents in place.

use std::fs;
use std::io;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Writes `bytes` to `path` through a temporary sibling and a rename. The
/// parent directory is created with `dir_mode` when missing, and the file
/// is created with `file_mode`.
pub fn write(path: &Path, bytes: &[u8], file_mode: u32, dir_mode: u32) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} has no parent directory", path.display()),
        )
    })?;
    if !parent.exists() {
        fs::create_dir_all(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(dir_mode))?;
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp = parent.join(format!(".{name}.{}.tmp", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(file_mode)
        .open(&tmp)?;
    io::Write::write_all(&mut file, bytes)?;
    file.sync_all()?;
    drop(file);
    fs::set_permissions(&tmp, fs::Permissions::from_mode(file_mode))?;
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_through_a_temp_file_with_the_requested_modes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("file.toml");
        write(&path, b"a = 1\n", 0o600, 0o700).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "a = 1\n");
        let file_mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        let dir_mode = fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600);
        assert_eq!(dir_mode, 0o700);
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }
}
