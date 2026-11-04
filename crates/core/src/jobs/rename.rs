use std::{
    ffi::{OsStr, OsString},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use rustix::{
    fs::{CWD, RenameFlags, renameat_with},
    io::Errno,
};
use tokio::task::spawn_blocking;

use crate::jobs::error::{AtPath, JobError, NameError};

const NAME_MAX: usize = 255;

/// Checks that `name` can name a single directory entry.
pub fn validate_name(name: &OsStr) -> Result<(), NameError> {
    let bytes = name.as_bytes();
    match bytes {
        [] => Err(NameError::Empty),
        b"." | b".." => Err(NameError::Reserved),
        _ if bytes.contains(&b'/') => Err(NameError::Slash),
        _ if bytes.contains(&0) => Err(NameError::Nul),
        _ if bytes.len() > NAME_MAX => Err(NameError::TooLong),
        _ => Ok(()),
    }
}

/// Renames `path` within its folder, never replacing an existing entry, and
/// returns the new path.
pub async fn rename(path: PathBuf, new_name: OsString) -> Result<PathBuf, JobError> {
    validate_name(&new_name)?;
    let target = path.with_file_name(&new_name);
    spawn_blocking(move || rename_without_replacing(&path, target)).await?
}

pub fn rename_without_replacing(from: &Path, to: PathBuf) -> Result<PathBuf, JobError> {
    match renameat_with(CWD, from, CWD, &to, RenameFlags::NOREPLACE) {
        Ok(()) => Ok(to),
        Err(Errno::EXIST) => Err(JobError::Exists(to)),
        Err(error) => Err(error).at(from),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_validated() {
        assert_eq!(validate_name(OsStr::new("")), Err(NameError::Empty));
        assert_eq!(validate_name(OsStr::new(".")), Err(NameError::Reserved));
        assert_eq!(validate_name(OsStr::new("..")), Err(NameError::Reserved));
        assert_eq!(validate_name(OsStr::new("a/b")), Err(NameError::Slash));
        assert_eq!(validate_name(OsStr::new("a\0b")), Err(NameError::Nul));
        let long = "x".repeat(NAME_MAX + 1);
        assert_eq!(validate_name(OsStr::new(&long)), Err(NameError::TooLong));
        assert_eq!(validate_name(OsStr::new("...hidden")), Ok(()));
    }

    #[tokio::test]
    async fn rename_never_replaces() {
        let temp = tempfile::tempdir().unwrap();
        let from = temp.path().join("a");
        std::fs::write(&from, b"a").unwrap();
        std::fs::write(temp.path().join("b"), b"b").unwrap();
        assert!(matches!(
            rename(from.clone(), "b".into()).await,
            Err(JobError::Exists(_))
        ));
        assert!(matches!(
            rename(from.clone(), "x/y".into()).await,
            Err(JobError::InvalidName(NameError::Slash))
        ));
        let renamed = rename(from, "c".into()).await.unwrap();
        assert_eq!(renamed, temp.path().join("c"));
        assert_eq!(std::fs::read(renamed).unwrap(), b"a");
    }
}
