use std::{io, path::PathBuf};

use crate::{
    jobs::{JobError, NameError},
    privileged::frame::HelperFailure,
};

/// Why the helper refuses a request before touching anything.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("the request names no files")]
    Empty,
    #[error("{} is not an absolute path", .0.display())]
    NotAbsolute(PathBuf),
    #[error("{} goes through \"..\"", .0.display())]
    ParentComponent(PathBuf),
    #[error("{} is a protected system location", .0.display())]
    Forbidden(PathBuf),
    #[error("{} is not a folder", .0.display())]
    NotADirectory(PathBuf),
    #[error("{path}: {source}", path = path.display())]
    Unreachable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid name: {0}")]
    InvalidName(#[from] NameError),
}

#[derive(Debug, thiserror::Error)]
pub enum PrivilegeError {
    #[error("authentication was cancelled")]
    Cancelled,
    #[error("not authorized to change these files")]
    NotAuthorized,
    #[error("{0}")]
    Failed(HelperFailure),
    #[error("cannot run the helper: {0}")]
    Spawn(#[source] io::Error),
    #[error("the helper stopped unexpectedly ({0})")]
    Exited(String),
    #[error("lost the helper: {0}")]
    Pipe(#[source] io::Error),
}

impl PrivilegeError {
    /// What pkexec's exit status means when the helper reported nothing.
    pub fn from_exit_code(code: Option<i32>) -> Self {
        match code {
            Some(PKEXEC_DISMISSED) => Self::Cancelled,
            Some(PKEXEC_NOT_AUTHORIZED) => Self::NotAuthorized,
            Some(code) => Self::Exited(format!("exit code {code}")),
            None => Self::Exited("killed by a signal".to_owned()),
        }
    }

    /// The error a job reports for this, against `path` when nothing better
    /// is known.
    pub fn into_job_error(self, path: Option<&PathBuf>) -> JobError {
        match self {
            Self::Failed(failure) => failure.into_job_error(),
            Self::Cancelled => JobError::Cancelled,
            other => JobError::Io {
                path: path.cloned().unwrap_or_default(),
                source: io::Error::other(other.to_string()),
            },
        }
    }
}

pub const PKEXEC_DISMISSED: i32 = 126;
pub const PKEXEC_NOT_AUTHORIZED: i32 = 127;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_pkexec_exit_codes() {
        assert!(matches!(
            PrivilegeError::from_exit_code(Some(126)),
            PrivilegeError::Cancelled
        ));
        assert!(matches!(
            PrivilegeError::from_exit_code(Some(127)),
            PrivilegeError::NotAuthorized
        ));
        assert!(matches!(
            PrivilegeError::from_exit_code(Some(3)),
            PrivilegeError::Exited(_)
        ));
        assert!(matches!(
            PrivilegeError::from_exit_code(None),
            PrivilegeError::Exited(_)
        ));
    }
}
