use std::cmp::Ordering;

use rayon::slice::ParallelSliceMut;

use crate::{
    model::{
        entries::{Entries, EntryId},
        entry::Entry,
    },
    util::natsort::natural_cmp,
};

const PARALLEL_SORT_THRESHOLD: usize = 50_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SortKey {
    #[default]
    Name,
    Modified,
    Created,
    Size,
    Kind,
}

/// How a folder orders its entries. Every key falls back to the natural name
/// order and then the raw bytes, so the order is total and stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SortOrder {
    pub key: SortKey,
    pub reverse: bool,
    pub dirs_first: bool,
}

impl Default for SortOrder {
    fn default() -> Self {
        Self {
            key: SortKey::Name,
            reverse: false,
            dirs_first: true,
        }
    }
}

impl SortOrder {
    pub fn compare(&self, left: &Entry, right: &Entry) -> Ordering {
        let folders = if self.dirs_first {
            right.is_dir().cmp(&left.is_dir())
        } else {
            Ordering::Equal
        };
        folders.then_with(|| {
            let ordering = self
                .compare_key(left, right)
                .then_with(|| natural_cmp(left.name.as_bytes(), right.name.as_bytes()))
                .then_with(|| left.name.cmp(&right.name));
            if self.reverse {
                ordering.reverse()
            } else {
                ordering
            }
        })
    }

    fn compare_key(&self, left: &Entry, right: &Entry) -> Ordering {
        match self.key {
            SortKey::Name => Ordering::Equal,
            SortKey::Modified => left.stat.modified.cmp(&right.stat.modified),
            SortKey::Created => left.stat.created.cmp(&right.stat.created),
            SortKey::Size => left.stat.len.cmp(&right.stat.len),
            SortKey::Kind => left
                .kind
                .cmp(&right.kind)
                .then_with(|| compare_extensions(left, right)),
        }
    }

    /// Sorts a permutation of `entries`, in parallel once it is large.
    pub fn sort(&self, ids: &mut [EntryId], entries: &Entries) {
        let compare = |a: &EntryId, b: &EntryId| self.compare(entries.entry(*a), entries.entry(*b));
        if ids.len() > PARALLEL_SORT_THRESHOLD {
            ids.par_sort_unstable_by(compare);
        } else {
            ids.sort_unstable_by(compare);
        }
    }

    /// Sorts `incoming` and merges it into the already sorted `order` in one
    /// linear pass.
    pub fn merge_insert(
        &self,
        order: &mut Vec<EntryId>,
        mut incoming: Vec<EntryId>,
        entries: &Entries,
    ) {
        self.sort(&mut incoming, entries);
        if order.is_empty() {
            *order = incoming;
            return;
        }
        let mut merged = Vec::with_capacity(order.len() + incoming.len());
        let mut existing = order.iter().copied().peekable();
        for id in incoming {
            let entry = entries.entry(id);
            while let Some(earlier) =
                existing.next_if(|&other| self.compare(entries.entry(other), entry).is_lt())
            {
                merged.push(earlier);
            }
            merged.push(id);
        }
        merged.extend(existing);
        *order = merged;
    }
}

fn compare_extensions(left: &Entry, right: &Entry) -> Ordering {
    extension_bytes(left)
        .iter()
        .map(u8::to_ascii_lowercase)
        .cmp(extension_bytes(right).iter().map(u8::to_ascii_lowercase))
}

fn extension_bytes(entry: &Entry) -> &[u8] {
    entry.extension().map(str::as_bytes).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{entry::EntryExtra, stat::Stat, timestamp::Timestamp};

    const DIRECTORY_MODE: u16 = 0o040755;

    fn file(name: &str, len: u64, modified: i64) -> Entry {
        Entry::new(
            name.into(),
            Stat {
                len,
                modified: Timestamp::from_parts(modified, 0),
                ..Stat::default()
            },
            EntryExtra::default(),
        )
    }

    fn folder(name: &str) -> Entry {
        Entry::new(
            name.into(),
            Stat {
                mode: DIRECTORY_MODE,
                ..Stat::default()
            },
            EntryExtra::default(),
        )
    }

    fn sample() -> Entries {
        let mut entries = Entries::default();
        for entry in [
            file("b10.txt", 30, 3),
            folder("Zeta"),
            file("B2.md", 10, 1),
            file("a.rs", 20, 2),
            folder("alpha"),
        ] {
            entries.push(entry);
        }
        entries
    }

    fn sorted(entries: &Entries, order: SortOrder) -> Vec<&str> {
        let mut ids: Vec<_> = entries.ids(true).collect();
        order.sort(&mut ids, entries);
        ids.iter()
            .map(|&id| entries.entry(id).name.as_str())
            .collect()
    }

    #[test]
    fn name_order_is_natural_with_folders_first() {
        let entries = sample();
        assert_eq!(
            sorted(&entries, SortOrder::default()),
            ["alpha", "Zeta", "a.rs", "B2.md", "b10.txt"]
        );
    }

    #[test]
    fn reverse_keeps_folders_first() {
        let entries = sample();
        let order = SortOrder {
            reverse: true,
            ..SortOrder::default()
        };
        assert_eq!(
            sorted(&entries, order),
            ["Zeta", "alpha", "b10.txt", "B2.md", "a.rs"]
        );
    }

    #[test]
    fn keys_order_by_their_field() {
        let entries = sample();
        let by = |key| SortOrder {
            key,
            dirs_first: false,
            reverse: false,
        };
        assert_eq!(
            sorted(&entries, by(SortKey::Size)),
            ["alpha", "Zeta", "B2.md", "a.rs", "b10.txt"]
        );
        assert_eq!(
            sorted(&entries, by(SortKey::Modified)),
            ["alpha", "Zeta", "B2.md", "a.rs", "b10.txt"]
        );
        assert_eq!(
            sorted(&entries, by(SortKey::Kind)),
            ["alpha", "Zeta", "B2.md", "b10.txt", "a.rs"]
        );
    }

    #[test]
    fn merge_insert_matches_a_full_sort() {
        let mut entries = Entries::default();
        let order = SortOrder::default();
        let mut merged = Vec::new();
        for chunk in [["f3", "f1", "dir"], ["f10", "f2", "a"], ["f0", "zz", "f11"]] {
            let ids = chunk
                .into_iter()
                .map(|name| {
                    entries.push(if name == "dir" {
                        folder(name)
                    } else {
                        file(name, 0, 0)
                    })
                })
                .collect();
            order.merge_insert(&mut merged, ids, &entries);
        }
        let mut full: Vec<_> = entries.ids(true).collect();
        order.sort(&mut full, &entries);
        assert_eq!(merged, full);
    }

    #[test]
    fn parallel_and_serial_sorts_agree() {
        let mut entries = Entries::default();
        for index in 0..PARALLEL_SORT_THRESHOLD + 1 {
            entries.push(file(&format!("file{}", index * 7919 % 100_003), 0, 0));
        }
        let order = SortOrder::default();
        let mut parallel: Vec<_> = entries.ids(true).collect();
        order.sort(&mut parallel, &entries);
        let mut serial = parallel.clone();
        serial.reverse();
        serial.sort_by(|a, b| order.compare(entries.entry(*a), entries.entry(*b)));
        assert_eq!(parallel, serial);
    }
}
