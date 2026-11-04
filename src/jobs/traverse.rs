use std::{
    borrow::Cow,
    collections::VecDeque,
    fs::{self, FileType},
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
};

use tokio_util::sync::CancellationToken;

use crate::jobs::{
    context::check,
    error::{AtPath, JobError},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Dir,
    File,
    Symlink,
    Fifo,
    /// Sockets and devices, which are not copied.
    Other,
}

impl From<FileType> for NodeKind {
    fn from(file_type: FileType) -> Self {
        if file_type.is_dir() {
            Self::Dir
        } else if file_type.is_file() {
            Self::File
        } else if file_type.is_symlink() {
            Self::Symlink
        } else if file_type.is_fifo() {
            Self::Fifo
        } else {
            Self::Other
        }
    }
}

/// One entry of a tree, relative to its root (the root itself is empty).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub relative: PathBuf,
    pub kind: NodeKind,
    /// The size of a regular file; 0 for everything else.
    pub len: u64,
}

impl Node {
    pub fn is_root(&self) -> bool {
        self.relative.as_os_str().is_empty()
    }

    pub fn under<'a>(&self, root: &'a Path) -> Cow<'a, Path> {
        if self.is_root() {
            Cow::Borrowed(root)
        } else {
            Cow::Owned(root.join(&self.relative))
        }
    }
}

/// Visits `root` and everything below it breadth-first, parents before their
/// children, without following symlinks.
pub fn walk(
    root: &Path,
    cancel: &CancellationToken,
    mut visit: impl FnMut(Node),
) -> Result<(), JobError> {
    let meta = fs::symlink_metadata(root).at(root)?;
    let mut pending = VecDeque::new();
    if meta.is_dir() {
        pending.push_back(PathBuf::new());
    }
    visit(node(PathBuf::new(), &meta));
    while let Some(relative) = pending.pop_front() {
        check(cancel)?;
        let dir = root.join(&relative);
        for entry in fs::read_dir(&dir).at(&dir)? {
            let entry = entry.at(&dir)?;
            let meta = entry.metadata().at(&entry.path())?;
            let child = relative.join(entry.file_name());
            if meta.is_dir() {
                pending.push_back(child.clone());
            }
            visit(node(child, &meta));
        }
    }
    Ok(())
}

fn node(relative: PathBuf, meta: &fs::Metadata) -> Node {
    let kind = NodeKind::from(meta.file_type());
    let len = if kind == NodeKind::File {
        meta.len()
    } else {
        0
    };
    Node {
        relative,
        kind,
        len,
    }
}
