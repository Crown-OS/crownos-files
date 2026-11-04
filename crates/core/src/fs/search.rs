//! Finding files and folders under a folder by name, breadth first so the
//! nearest matches come first. Stays on the folder's own filesystem and never
//! follows symlinks.

use std::{
    collections::VecDeque,
    ffi::OsStr,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    time::Duration,
};

use rustix::fs::{AtFlags, CWD, Dir, FileType, Mode, OFlags, StatxFlags, openat, statx};
use tokio_util::sync::CancellationToken;

use crate::{model::filter::Filter, util::throttle::Throttle};

/// Matches past this many stop the walk: enough to scroll, few enough to stay light.
pub const MOST_MATCHES: usize = 2000;
const DELIVERY_INTERVAL: Duration = Duration::from_millis(120);

/// What to look for and where.
#[derive(Debug, Clone)]
pub struct SearchScope<'a> {
    pub root: &'a Path,
    pub filter: &'a Filter,
    pub show_hidden: bool,
}

/// Walks under `scope.root` on the calling (blocking) thread, handing
/// `found` the paths whose names match in batches, until the tree is done,
/// [`MOST_MATCHES`] are found or `cancel` fires.
pub fn search_tree(
    scope: &SearchScope<'_>,
    cancel: &CancellationToken,
    mut found: impl FnMut(Vec<PathBuf>),
) {
    let Some(device) = device_of(scope.root) else {
        return;
    };
    let mut pending = VecDeque::from([scope.root.to_path_buf()]);
    let mut batch = Vec::new();
    let mut remaining = MOST_MATCHES;
    let mut delivery = Throttle::new(DELIVERY_INTERVAL);
    while let Some(dir) = pending.pop_front()
        && remaining > 0
    {
        if cancel.is_cancelled() {
            return;
        }
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let Ok(mut listing) = openat(CWD, &dir, flags, Mode::empty()).and_then(Dir::new) else {
            continue;
        };
        while let Some(Ok(dirent)) = listing.read() {
            let name = dirent.file_name().to_bytes();
            if name == b"." || name == b".." || (!scope.show_hidden && name.starts_with(b".")) {
                continue;
            }
            let path = dir.join(OsStr::from_bytes(name));
            let is_dir = match dirent.file_type() {
                FileType::Directory => true,
                FileType::Unknown => path.symlink_metadata().is_ok_and(|meta| meta.is_dir()),
                _ => false,
            };
            if is_dir && device_of(&path) == Some(device) {
                pending.push_back(path.clone());
            }
            if scope.filter.matches(name) {
                batch.push(path);
                remaining -= 1;
                if remaining == 0 {
                    break;
                }
            }
        }
        if !batch.is_empty() && delivery.ready() {
            found(std::mem::take(&mut batch));
        }
    }
    if !cancel.is_cancelled() {
        found(batch);
    }
}

fn device_of(path: &Path) -> Option<(u32, u32)> {
    let stat = statx(
        CWD,
        path,
        AtFlags::SYMLINK_NOFOLLOW | AtFlags::STATX_DONT_SYNC,
        StatxFlags::TYPE,
    )
    .ok()?;
    Some((stat.stx_dev_major, stat.stx_dev_minor))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn found_under(root: &Path, query: &str, show_hidden: bool) -> Vec<PathBuf> {
        let filter = Filter::new(query).unwrap();
        let scope = SearchScope {
            root,
            filter: &filter,
            show_hidden,
        };
        let mut found = Vec::new();
        search_tree(&scope, &CancellationToken::new(), |batch| {
            found.extend(batch);
        });
        found
    }

    #[test]
    fn matches_files_and_folders_at_every_depth_nearest_first() {
        let root = tempfile::tempdir().unwrap();
        let deep = root.path().join("a/b/report-drafts");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("Report.pdf"), b"").unwrap();
        fs::write(root.path().join("report.txt"), b"").unwrap();
        fs::write(root.path().join("a/notes.md"), b"").unwrap();
        let found = found_under(root.path(), "report", false);
        assert_eq!(
            found,
            [
                root.path().join("report.txt"),
                deep.clone(),
                deep.join("Report.pdf"),
            ]
        );
    }

    #[test]
    fn hidden_entries_are_skipped_unless_shown() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join(".cache/keep")).unwrap();
        fs::write(root.path().join(".cache/keep/keep.txt"), b"").unwrap();
        assert!(found_under(root.path(), "keep", false).is_empty());
        assert_eq!(found_under(root.path(), "keep", true).len(), 2);
    }

    #[test]
    fn a_cancelled_search_finds_nothing() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("match.txt"), b"").unwrap();
        let filter = Filter::new("match").unwrap();
        let scope = SearchScope {
            root: root.path(),
            filter: &filter,
            show_hidden: false,
        };
        let cancel = CancellationToken::new();
        cancel.cancel();
        let mut found = Vec::new();
        search_tree(&scope, &cancel, |batch| found.extend(batch));
        assert!(found.is_empty());
    }
}
