use std::{path::Path, sync::Arc};

use compact_str::CompactString;
use rustix::io::Errno;

use crate::{
    model::{dir_stamp::DirStamp, entry::Entry, ticket::Ticket},
    util::hash::FastIndexMap,
};

/// A change to one directory's listing, as the engine reports it.
#[derive(Debug, Clone)]
pub enum FilesOp {
    /// The next chunk of a listing.
    Part {
        dir: Arc<Path>,
        ticket: Ticket,
        entries: Vec<Entry>,
    },
    /// The listing is complete; `stamp` is the directory as it was opened.
    Done {
        dir: Arc<Path>,
        ticket: Ticket,
        stamp: DirStamp,
    },
    Fail {
        dir: Arc<Path>,
        ticket: Ticket,
        error: Errno,
    },
    /// Entries created or changed since the listing.
    Upsert { dir: Arc<Path>, entries: Vec<Entry> },
    Delete {
        dir: Arc<Path>,
        names: Vec<CompactString>,
    },
    /// Measured directory sizes, by child name.
    Size {
        dir: Arc<Path>,
        sizes: Vec<(CompactString, u64)>,
    },
}

#[derive(Default)]
struct Mutations {
    changes: FastIndexMap<CompactString, Option<Entry>>,
    sizes: FastIndexMap<CompactString, u64>,
}

impl FilesOp {
    pub fn dir(&self) -> &Arc<Path> {
        match self {
            Self::Part { dir, .. }
            | Self::Done { dir, .. }
            | Self::Fail { dir, .. }
            | Self::Upsert { dir, .. }
            | Self::Delete { dir, .. }
            | Self::Size { dir, .. } => dir,
        }
    }

    /// Coalesces a batch: per directory, the last upsert or delete of each
    /// name wins and becomes at most one `Delete`, one `Upsert` and one
    /// `Size`. Listing ops pass through first, in their original order.
    pub fn merge(ops: impl IntoIterator<Item = Self>) -> Vec<Self> {
        let mut listing = Vec::new();
        let mut per_dir: FastIndexMap<Arc<Path>, Mutations> = FastIndexMap::default();
        for op in ops {
            match op {
                Self::Upsert { dir, entries } => per_dir.entry(dir).or_default().changes.extend(
                    entries
                        .into_iter()
                        .map(|entry| (entry.name.clone(), Some(entry))),
                ),
                Self::Delete { dir, names } => per_dir
                    .entry(dir)
                    .or_default()
                    .changes
                    .extend(names.into_iter().map(|name| (name, None))),
                Self::Size { dir, sizes } => per_dir.entry(dir).or_default().sizes.extend(sizes),
                listed => listing.push(listed),
            }
        }
        for (dir, mutations) in per_dir {
            let (upserts, deletes): (Vec<_>, Vec<_>) = mutations
                .changes
                .into_iter()
                .partition(|(_, change)| change.is_some());
            if !deletes.is_empty() {
                let names = deletes.into_iter().map(|(name, _)| name).collect();
                listing.push(Self::Delete {
                    dir: dir.clone(),
                    names,
                });
            }
            if !upserts.is_empty() {
                let entries = upserts.into_iter().filter_map(|(_, entry)| entry).collect();
                listing.push(Self::Upsert {
                    dir: dir.clone(),
                    entries,
                });
            }
            if !mutations.sizes.is_empty() {
                let sizes = mutations.sizes.into_iter().collect();
                listing.push(Self::Size { dir, sizes });
            }
        }
        listing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{entry::EntryExtra, stat::Stat};

    fn entry(name: &str, len: u64) -> Entry {
        Entry::new(
            name.into(),
            Stat {
                len,
                ..Stat::default()
            },
            EntryExtra::default(),
        )
    }

    #[test]
    fn the_last_change_per_name_wins_per_directory() {
        let here: Arc<Path> = Path::new("/here").into();
        let there: Arc<Path> = Path::new("/there").into();
        let merged = FilesOp::merge([
            FilesOp::Upsert {
                dir: here.clone(),
                entries: vec![entry("a", 1), entry("b", 1)],
            },
            FilesOp::Delete {
                dir: here.clone(),
                names: vec!["a".into()],
            },
            FilesOp::Upsert {
                dir: there.clone(),
                entries: vec![entry("c", 1)],
            },
            FilesOp::Upsert {
                dir: here.clone(),
                entries: vec![entry("b", 2)],
            },
            FilesOp::Size {
                dir: here.clone(),
                sizes: vec![("d".into(), 1), ("d".into(), 5)],
            },
        ]);
        let [
            FilesOp::Delete {
                dir: deleted_in,
                names,
            },
            FilesOp::Upsert {
                dir: upserted_in,
                entries,
            },
            FilesOp::Size { sizes, .. },
            FilesOp::Upsert {
                dir: other_dir,
                entries: other,
            },
        ] = merged.as_slice()
        else {
            panic!("unexpected merge: {merged:?}");
        };
        assert_eq!(deleted_in, &here);
        assert_eq!(names.as_slice(), ["a"]);
        assert_eq!(upserted_in, &here);
        assert_eq!(entries.as_slice(), [entry("b", 2)]);
        assert_eq!(sizes.as_slice(), [("d".into(), 5)]);
        assert_eq!((other_dir, other.len()), (&there, 1));
    }

    #[test]
    fn a_delete_after_an_upsert_cancels_it() {
        let dir: Arc<Path> = Path::new("/d").into();
        let merged = FilesOp::merge([
            FilesOp::Upsert {
                dir: dir.clone(),
                entries: vec![entry("x", 0)],
            },
            FilesOp::Delete {
                dir: dir.clone(),
                names: vec!["x".into()],
            },
            FilesOp::Upsert {
                dir,
                entries: vec![entry("x", 3)],
            },
        ]);
        assert!(
            matches!(merged.as_slice(), [FilesOp::Upsert { entries, .. }] if entries[0].stat.len == 3)
        );
    }
}
