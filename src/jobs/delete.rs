use std::{
    fs,
    path::{Path, PathBuf},
};

use rustix::{
    fd::BorrowedFd,
    fs::{AtFlags, CWD, Dir, FileType, Mode, OFlags, openat, unlinkat},
    io::Errno,
};
use tokio_util::sync::CancellationToken;

use crate::jobs::{
    context::{JobContext, check},
    error::{AtPath, JobError},
    traverse::walk,
};

const DIR_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

pub(crate) fn run_delete(paths: &[PathBuf], ctx: &mut JobContext) -> Result<(), JobError> {
    let items = paths
        .iter()
        .map(|path| count_items(path, &ctx.cancel))
        .sum::<Result<u64, _>>()?;
    ctx.start(0, items);
    for path in paths {
        remove_tree(path, &ctx.cancel, || ctx.progress.add_item(path))?;
    }
    Ok(())
}

pub(crate) fn count_items(path: &Path, cancel: &CancellationToken) -> Result<u64, JobError> {
    let mut items = 0;
    walk(path, cancel, |_| items += 1)?;
    Ok(items)
}

/// Permanently removes `path` and, for a folder, everything below it, with
/// an `openat`/`unlinkat` walk that never follows symlinks. `on_removed`
/// hears every removed entry.
pub fn remove_tree(
    path: &Path,
    cancel: &CancellationToken,
    mut on_removed: impl FnMut(),
) -> Result<(), JobError> {
    let root = match open_dir(CWD, path) {
        Ok(dir) => dir,
        Err(Errno::NOTDIR | Errno::LOOP) => {
            fs::remove_file(path).at(path)?;
            on_removed();
            return Ok(());
        }
        Err(error) => return Err(error).at(path),
    };
    let mut stack = vec![(root, None)];
    while let Some((dir, _)) = stack.last_mut() {
        check(cancel)?;
        let Some(entry) = dir.read() else {
            let Some((_, name)) = stack.pop() else { break };
            match (stack.last(), name) {
                (Some((parent, _)), Some(name)) => {
                    let parent = parent.fd().at(path)?;
                    ignore_missing(unlinkat(parent, &name, AtFlags::REMOVEDIR)).at(path)?;
                }
                _ => fs::remove_dir(path).at(path)?,
            }
            on_removed();
            continue;
        };
        let entry = entry.at(path)?;
        let name = entry.file_name();
        if matches!(name.to_bytes(), b"." | b"..") {
            continue;
        }
        let parent = dir.fd().at(path)?;
        if matches!(entry.file_type(), FileType::Directory | FileType::Unknown) {
            match open_dir(parent, name) {
                Ok(child) => {
                    let name = name.to_owned();
                    stack.push((child, Some(name)));
                    continue;
                }
                Err(Errno::NOENT) => continue,
                Err(Errno::NOTDIR | Errno::LOOP) => {}
                Err(error) => return Err(error).at(path),
            }
        }
        ignore_missing(unlinkat(parent, name, AtFlags::empty())).at(path)?;
        on_removed();
    }
    Ok(())
}

fn open_dir<P: rustix::path::Arg>(parent: BorrowedFd<'_>, path: P) -> rustix::io::Result<Dir> {
    openat(parent, path, DIR_FLAGS, Mode::empty()).and_then(Dir::new)
}

fn ignore_missing(result: rustix::io::Result<()>) -> rustix::io::Result<()> {
    match result {
        Err(Errno::NOENT) => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_files_symlinks_and_trees_without_following_links() {
        let temp = tempfile::tempdir().unwrap();
        let keep = temp.path().join("keep");
        fs::create_dir(&keep).unwrap();
        fs::write(keep.join("precious"), b"x").unwrap();
        let tree = temp.path().join("tree");
        fs::create_dir_all(tree.join("a/b")).unwrap();
        fs::write(tree.join("a/b/f"), b"x").unwrap();
        std::os::unix::fs::symlink(&keep, tree.join("a/link")).unwrap();
        let mut removed = 0;
        remove_tree(&tree, &CancellationToken::new(), || removed += 1).unwrap();
        assert_eq!(removed, 5);
        assert!(!tree.exists() && keep.join("precious").exists());
        let file = temp.path().join("file");
        fs::write(&file, b"x").unwrap();
        remove_tree(&file, &CancellationToken::new(), || {}).unwrap();
        assert!(!file.exists());
    }
}
