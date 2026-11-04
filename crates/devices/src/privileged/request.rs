use std::{ffi::OsString, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    jobs::{ConflictPolicy, JobKind},
    util::path_bytes,
};

/// One operation for the privileged helper. Paths are absolute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivilegedRequest {
    Copy {
        #[serde(with = "path_bytes::list")]
        sources: Vec<PathBuf>,
        #[serde(with = "path_bytes")]
        destination: PathBuf,
        on_conflict: ConflictPolicy,
    },
    Move {
        #[serde(with = "path_bytes::list")]
        sources: Vec<PathBuf>,
        #[serde(with = "path_bytes")]
        destination: PathBuf,
        on_conflict: ConflictPolicy,
    },
    Delete {
        #[serde(with = "path_bytes::list")]
        paths: Vec<PathBuf>,
    },
    CreateFolder {
        #[serde(with = "path_bytes")]
        parent: PathBuf,
        #[serde(with = "path_bytes")]
        name: OsString,
    },
    Rename {
        #[serde(with = "path_bytes")]
        path: PathBuf,
        #[serde(with = "path_bytes")]
        new_name: OsString,
    },
    Chmod {
        #[serde(with = "path_bytes::list")]
        paths: Vec<PathBuf>,
        mode: u32,
    },
}

impl PrivilegedRequest {
    /// The kind of background job this is; `None` for instant operations.
    pub fn job_kind(&self) -> Option<JobKind> {
        match self {
            Self::Copy { .. } => Some(JobKind::Copy),
            Self::Move { .. } => Some(JobKind::Move),
            Self::Delete { .. } => Some(JobKind::Delete),
            Self::CreateFolder { .. } | Self::Rename { .. } | Self::Chmod { .. } => None,
        }
    }

    /// The path errors are reported against when no other is known.
    pub fn primary_path(&self) -> Option<&PathBuf> {
        match self {
            Self::Copy { destination, .. } | Self::Move { destination, .. } => Some(destination),
            Self::Delete { paths } | Self::Chmod { paths, .. } => paths.first(),
            Self::CreateFolder { parent, .. } => Some(parent),
            Self::Rename { path, .. } => Some(path),
        }
    }
}
