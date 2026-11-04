use std::{
    io,
    path::{Path, PathBuf},
};

use rustix::io::Errno;

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("{path}: {source}", path = path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot put the folder {folder} inside itself", folder = folder.display())]
    IntoItself { folder: PathBuf },
    #[error(
        "not enough free space in {dir}: {needed} bytes needed, {available} available",
        dir = dir.display()
    )]
    NoSpace {
        dir: PathBuf,
        needed: u64,
        available: u64,
    },
    #[error("{} already exists", .0.display())]
    Exists(PathBuf),
    #[error("{} cannot be moved to the trash", .0.display())]
    NoTrash(PathBuf),
    #[error("invalid name: {0}")]
    InvalidName(#[from] NameError),
    #[error("the background task stopped: {0}")]
    Task(#[from] tokio::task::JoinError),
    #[error("cancelled")]
    Cancelled,
}

impl JobError {
    /// Permission errors the UI can retry through the privileged helper.
    pub fn needs_privilege(&self) -> bool {
        let Self::Io { source, .. } = self else {
            return false;
        };
        matches!(
            source.raw_os_error().map(Errno::from_raw_os_error),
            Some(Errno::ACCESS | Errno::PERM)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    #[error("the name is empty")]
    Empty,
    #[error("\".\" and \"..\" are reserved")]
    Reserved,
    #[error("names cannot contain \"/\"")]
    Slash,
    #[error("names cannot contain a NUL byte")]
    Nul,
    #[error("the name is longer than 255 bytes")]
    TooLong,
}

/// Attaches the path an I/O error happened at.
pub(crate) trait AtPath<T> {
    fn at(self, path: &Path) -> Result<T, JobError>;
}

impl<T, E: Into<io::Error>> AtPath<T> for Result<T, E> {
    fn at(self, path: &Path) -> Result<T, JobError> {
        self.map_err(|source| JobError::Io {
            path: path.to_path_buf(),
            source: source.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_permission_errors_need_privilege() {
        let denied: Result<(), Errno> = Err(Errno::ACCESS);
        assert!(
            denied
                .at(Path::new("/root/x"))
                .unwrap_err()
                .needs_privilege()
        );
        let not_permitted: Result<(), Errno> = Err(Errno::PERM);
        assert!(
            not_permitted
                .at(Path::new("/x"))
                .unwrap_err()
                .needs_privilege()
        );
        let missing: Result<(), Errno> = Err(Errno::NOENT);
        assert!(!missing.at(Path::new("/x")).unwrap_err().needs_privilege());
        assert!(!JobError::Cancelled.needs_privilege());
    }
}
