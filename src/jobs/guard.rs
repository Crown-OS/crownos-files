use std::{fs, path::Path};

use rustix::fs::statvfs;

use crate::jobs::error::{AtPath, JobError};

/// Refuses to put a folder inside itself or one of its descendants.
pub fn ensure_not_into_itself(source: &Path, target: &Path) -> Result<(), JobError> {
    if !fs::symlink_metadata(source).is_ok_and(|meta| meta.is_dir()) {
        return Ok(());
    }
    let (Some(parent), Some(name)) = (target.parent(), target.file_name()) else {
        return Ok(());
    };
    let (Ok(folder), Ok(parent)) = (fs::canonicalize(source), fs::canonicalize(parent)) else {
        return Ok(());
    };
    let target = parent.join(name);
    if target != folder && target.starts_with(&folder) {
        return Err(JobError::IntoItself { folder });
    }
    Ok(())
}

/// Fails early when `dir`'s filesystem cannot hold `needed` more bytes.
pub fn ensure_free_space(dir: &Path, needed: u64) -> Result<(), JobError> {
    let stats = statvfs(dir).at(dir)?;
    let available = stats.f_bavail.saturating_mul(stats.f_frsize);
    if needed > available {
        return Err(JobError::NoSpace {
            dir: dir.to_path_buf(),
            needed,
            available,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_cannot_go_into_itself_but_next_to_itself_is_fine() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("folder");
        fs::create_dir_all(folder.join("child")).unwrap();
        assert!(matches!(
            ensure_not_into_itself(&folder, &folder.join("folder")),
            Err(JobError::IntoItself { .. })
        ));
        assert!(ensure_not_into_itself(&folder, &folder.join("child/folder")).is_err());
        assert!(ensure_not_into_itself(&folder, &folder).is_ok());
        assert!(ensure_not_into_itself(&folder, &temp.path().join("folder2")).is_ok());
        let file = temp.path().join("file");
        fs::write(&file, b"x").unwrap();
        assert!(ensure_not_into_itself(&file, &file.join("x")).is_ok());
    }

    #[test]
    fn free_space_is_checked_against_the_filesystem() {
        let temp = tempfile::tempdir().unwrap();
        assert!(ensure_free_space(temp.path(), 1).is_ok());
        assert!(matches!(
            ensure_free_space(temp.path(), u64::MAX),
            Err(JobError::NoSpace { .. })
        ));
    }
}
