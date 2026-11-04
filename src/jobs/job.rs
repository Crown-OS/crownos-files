use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::model::trash::TrashedItem;

static NEXT_JOB_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JobId(u64);

impl JobId {
    pub(crate) fn next() -> Self {
        Self(NEXT_JOB_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobKind {
    Copy,
    Move,
    Delete,
    Trash,
    Restore,
    DeleteFromTrash,
    EmptyTrash,
}

/// One source and the exact path it should end up at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transfer {
    pub from: PathBuf,
    pub to: PathBuf,
}

impl Transfer {
    /// Each source keeps its name inside `dir`.
    pub fn into_dir(sources: impl IntoIterator<Item = PathBuf>, dir: &Path) -> Vec<Self> {
        sources
            .into_iter()
            .filter_map(|from| {
                let to = dir.join(from.file_name()?);
                Some(Self { from, to })
            })
            .collect()
    }

    pub fn reversed(self) -> Self {
        Self {
            from: self.to,
            to: self.from,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    Copy(Vec<Transfer>),
    Move(Vec<Transfer>),
    Delete(Vec<PathBuf>),
    Trash(Vec<PathBuf>),
    Restore(Vec<TrashedItem>),
    DeleteFromTrash(Vec<TrashedItem>),
    EmptyTrash,
}

impl Job {
    pub fn copy_into(sources: impl IntoIterator<Item = PathBuf>, dir: &Path) -> Self {
        Self::Copy(Transfer::into_dir(sources, dir))
    }

    pub fn move_into(sources: impl IntoIterator<Item = PathBuf>, dir: &Path) -> Self {
        Self::Move(Transfer::into_dir(sources, dir))
    }

    /// Copies next to themselves; the copies get " (2)"-style names.
    pub fn duplicate(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        Self::Copy(
            paths
                .into_iter()
                .map(|path| Transfer {
                    to: path.clone(),
                    from: path,
                })
                .collect(),
        )
    }

    pub fn kind(&self) -> JobKind {
        match self {
            Self::Copy(_) => JobKind::Copy,
            Self::Move(_) => JobKind::Move,
            Self::Delete(_) => JobKind::Delete,
            Self::Trash(_) => JobKind::Trash,
            Self::Restore(_) => JobKind::Restore,
            Self::DeleteFromTrash(_) => JobKind::DeleteFromTrash,
            Self::EmptyTrash => JobKind::EmptyTrash,
        }
    }
}
