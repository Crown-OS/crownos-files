//! Recents, Trash and Search: folders the engine does not list, filled from
//! the recently-used list, the trash cans and a search's matches. Their
//! entries carry absolute paths as on-disk names, so `Entry::path_in`
//! reaches the real file.

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::Arc,
};

use compact_str::CompactString;
use crownui::{kit::icons, prelude::Runtime};
use tokio::task::spawn_blocking;

use crate::{
    app::{listing::apply_ops, search::search_again, tasks::Task},
    fs::stat_at::stat_path,
    jobs::list_trash,
    model::{
        dir_stamp::DirStamp,
        entry::{Entry, EntryExtra},
        files_op::FilesOp,
        kind::FileKind,
        ticket::Ticket,
        timestamp::Timestamp,
        trash::TrashedItem,
    },
    open::RecentFiles,
    state::Store,
    util::xdg_dirs::XdgDirs,
};

const RECENTS: &str = "recent:///";
const TRASH: &str = "trash:///";
const SEARCH: &str = "search:///";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualFolder {
    Recents,
    Trash,
    Search,
}

/// A virtual folder's listing, read off the UI thread.
#[derive(Debug)]
pub struct VirtualListing {
    pub folder: VirtualFolder,
    pub entries: Vec<Entry>,
    pub trashed: Vec<TrashedItem>,
}

impl VirtualFolder {
    pub fn of(dir: &Path) -> Option<Self> {
        match dir.as_os_str().to_str()? {
            RECENTS => Some(Self::Recents),
            TRASH => Some(Self::Trash),
            SEARCH => Some(Self::Search),
            _ => None,
        }
    }

    pub fn path(self) -> Arc<Path> {
        Path::new(match self {
            Self::Recents => RECENTS,
            Self::Trash => TRASH,
            Self::Search => SEARCH,
        })
        .into()
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Recents => "Recents",
            Self::Trash => "Trash",
            Self::Search => "Search",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Recents => icons::CLOCK,
            Self::Trash => icons::TRASH_2,
            Self::Search => icons::SEARCH,
        }
    }

    pub async fn list(self) -> VirtualListing {
        let (entries, trashed) = match self {
            Self::Recents => (recent_entries().await, Vec::new()),
            Self::Search => (Vec::new(), Vec::new()),
            Self::Trash => {
                let trashed = list_trash().await.unwrap_or_default();
                let listed = trashed.clone();
                let entries = spawn_blocking(move || trash_entries(&listed))
                    .await
                    .unwrap_or_default();
                (entries, trashed)
            }
        };
        VirtualListing {
            folder: self,
            entries,
            trashed,
        }
    }
}

async fn recent_entries() -> Vec<Entry> {
    spawn_blocking(|| {
        let Some(xdg) = XdgDirs::from_env() else {
            return Vec::new();
        };
        RecentFiles::from_xdg(&xdg)
            .read()
            .unwrap_or_default()
            .iter()
            .filter_map(|path| entry_at(path, None))
            .collect()
    })
    .await
    .unwrap_or_default()
}

fn trash_entries(trashed: &[TrashedItem]) -> Vec<Entry> {
    trashed
        .iter()
        .filter_map(|item| entry_at(&item.file_path(), item.info.original.file_name()))
        .collect()
}

/// The entry of `path`, shown as `shown_name` when given.
pub(crate) fn entry_at(path: &Path, shown_name: Option<&OsStr>) -> Option<Entry> {
    let mut entry = stat_path(path).ok()?;
    if let Some(name) = shown_name {
        entry.name = CompactString::from(name.to_string_lossy());
        entry.kind = FileKind::classify(&entry.name, entry.is_dir());
    }
    Some(absolute_entry(entry, path))
}

/// `entry` reaching `path` from any folder.
pub(crate) fn absolute_entry(mut entry: Entry, path: &Path) -> Entry {
    entry.extra = EntryExtra::new(
        entry.link_target().map(Path::to_path_buf),
        Some(path.as_os_str().into()),
    );
    entry
}

/// Lists the virtual folder being shown again, as after a job changed it.
pub fn reload_virtual(runtime: &mut Runtime, store: Store) {
    match VirtualFolder::of(&store.dir(runtime)) {
        Some(VirtualFolder::Search) => search_again(runtime, store),
        Some(folder) => store.run(runtime, Task::ListVirtual(folder)),
        None => {}
    }
}

/// Shows `listing` if its folder is still the one open.
pub fn apply_listing(runtime: &mut Runtime, store: Store, listing: VirtualListing) {
    let dir = listing.folder.path();
    if store.dir(runtime) != dir {
        return;
    }
    store
        .trash_items
        .update(runtime, |items| *items = listing.trashed);
    let ticket = Ticket::next();
    let stamp = DirStamp {
        modified: Timestamp::UNKNOWN,
        changed: Timestamp::UNKNOWN,
        len: 0,
        inode: 0,
        device: 0,
    };
    apply_ops(
        runtime,
        store,
        vec![
            FilesOp::Part {
                dir: dir.clone(),
                ticket,
                entries: listing.entries,
            },
            FilesOp::Done { dir, ticket, stamp },
        ],
    );
}

/// The trashed items behind `paths`, the files under a trash can's `files/`.
pub fn trashed_at(runtime: &Runtime, store: Store, paths: &[PathBuf]) -> Vec<TrashedItem> {
    store.trash_items.with(runtime, |items| {
        items
            .iter()
            .filter(|item| paths.contains(&item.file_path()))
            .cloned()
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_folders_round_trip_through_their_paths() {
        for folder in [
            VirtualFolder::Recents,
            VirtualFolder::Trash,
            VirtualFolder::Search,
        ] {
            assert_eq!(VirtualFolder::of(&folder.path()), Some(folder));
        }
        assert_eq!(VirtualFolder::of(Path::new("/home")), None);
    }

    #[test]
    fn an_entry_found_elsewhere_keeps_its_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("report.pdf");
        std::fs::write(&file, b"%PDF").unwrap();
        let entry = entry_at(&file, Some(OsStr::new("old name.pdf"))).unwrap();
        assert_eq!(entry.name, "old name.pdf");
        assert_eq!(entry.kind, FileKind::Pdf);
        assert_eq!(entry.path_in(&VirtualFolder::Trash.path()), file);
    }
}
