use std::{ffi::OsString, path::PathBuf};

use tokio::task::spawn_blocking;

use crate::jobs::{
    conflict::{Claim, unique_name},
    error::{AtPath, JobError},
    rename::validate_name,
};

/// Creates an empty folder named `name` in `dir`, or "name (2)"... when it is
/// taken, and returns its path.
pub async fn create_folder(dir: PathBuf, name: OsString) -> Result<PathBuf, JobError> {
    create(dir, name, Claim::Dir).await
}

/// Creates an empty file like [`create_folder`] creates folders.
pub async fn create_file(dir: PathBuf, name: OsString) -> Result<PathBuf, JobError> {
    create(dir, name, Claim::File).await
}

async fn create(dir: PathBuf, name: OsString, claim: Claim) -> Result<PathBuf, JobError> {
    validate_name(&name)?;
    spawn_blocking(move || unique_name(&dir, &name, claim).at(&dir)).await?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn creations_take_free_names() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().to_path_buf();
        let first = create_folder(dir.clone(), "New Folder".into())
            .await
            .unwrap();
        let second = create_folder(dir.clone(), "New Folder".into())
            .await
            .unwrap();
        let file = create_file(dir.clone(), "notes.txt".into()).await.unwrap();
        assert_eq!(second, dir.join("New Folder (2)"));
        assert!(first.is_dir() && second.is_dir() && file.is_file());
        assert!(create_file(dir, "..".into()).await.is_err());
    }
}
