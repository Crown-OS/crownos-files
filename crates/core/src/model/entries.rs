use compact_str::CompactString;

use crate::{model::entry::Entry, util::hash::FastHashMap};

const HIDDEN_BIT: u32 = 1 << 31;
const REMOVED: u32 = u32::MAX;

/// A stable handle to an entry in [`Entries`]: valid until the entry is
/// removed, unaffected by sorting, filtering or toggling hidden files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntryId(u32);

impl EntryId {
    fn new(index: usize, hidden: bool) -> Self {
        let index = index as u32;
        Self(if hidden { index | HIDDEN_BIT } else { index })
    }

    pub fn is_hidden(self) -> bool {
        self.0 & HIDDEN_BIT != 0
    }

    fn index(self) -> usize {
        (self.0 & !HIDDEN_BIT) as usize
    }
}

/// A folder's children, with hidden ones kept apart so showing or hiding
/// them never touches the visible ones.
#[derive(Debug, Clone, Default)]
pub struct Entries {
    shown: Vec<Entry>,
    hidden: Vec<Entry>,
}

impl Entries {
    pub fn len(&self) -> usize {
        self.shown.len() + self.hidden.len()
    }

    pub fn is_empty(&self) -> bool {
        self.shown.is_empty() && self.hidden.is_empty()
    }

    pub fn hidden_len(&self) -> usize {
        self.hidden.len()
    }

    pub fn get(&self, id: EntryId) -> Option<&Entry> {
        self.vec(id.is_hidden()).get(id.index())
    }

    /// The entry behind an id this container handed out.
    pub(crate) fn entry(&self, id: EntryId) -> &Entry {
        &self.vec(id.is_hidden())[id.index()]
    }

    fn vec(&self, hidden: bool) -> &[Entry] {
        if hidden { &self.hidden } else { &self.shown }
    }

    pub fn push(&mut self, entry: Entry) -> EntryId {
        let hidden = entry.is_hidden();
        let vec = if hidden {
            &mut self.hidden
        } else {
            &mut self.shown
        };
        vec.push(entry);
        EntryId::new(vec.len() - 1, hidden)
    }

    /// Every id, hidden ones last and only when `include_hidden`.
    pub fn ids(&self, include_hidden: bool) -> impl Iterator<Item = EntryId> {
        let hidden = if include_hidden { self.hidden.len() } else { 0 };
        (0..self.shown.len())
            .map(|index| EntryId::new(index, false))
            .chain((0..hidden).map(|index| EntryId::new(index, true)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Entry> {
        self.shown.iter_mut().chain(self.hidden.iter_mut())
    }

    pub fn find(&self, name: &str) -> Option<EntryId> {
        self.ids(true).find(|&id| self.entry(id).name == name)
    }

    pub fn reserve(&mut self, additional: usize) {
        self.shown.reserve(additional);
    }

    /// Replaces the entries whose names match and appends the rest, returning
    /// the id of every entry written.
    pub fn upsert(&mut self, incoming: Vec<Entry>) -> Vec<EntryId> {
        let mut pending: FastHashMap<CompactString, Entry> = incoming
            .into_iter()
            .map(|entry| (entry.name.clone(), entry))
            .collect();
        let mut written = Vec::with_capacity(pending.len());
        for (hidden, vec) in [(false, &mut self.shown), (true, &mut self.hidden)] {
            for (index, slot) in vec.iter_mut().enumerate() {
                if pending.is_empty() {
                    break;
                }
                if let Some(entry) = pending.remove(&slot.name) {
                    *slot = entry;
                    written.push(EntryId::new(index, hidden));
                }
            }
        }
        written.extend(pending.into_values().map(|entry| self.push(entry)));
        written
    }

    /// Drops the entries `keep` rejects; `None` when nothing was dropped.
    pub fn retain(&mut self, mut keep: impl FnMut(&Entry) -> bool) -> Option<Remap> {
        let shown = retain_mapped(&mut self.shown, &mut keep);
        let hidden = retain_mapped(&mut self.hidden, &mut keep);
        (shown.is_some() || hidden.is_some()).then(|| Remap {
            shown: shown.unwrap_or_default(),
            hidden: hidden.unwrap_or_default(),
        })
    }
}

fn retain_mapped(vec: &mut Vec<Entry>, keep: &mut impl FnMut(&Entry) -> bool) -> Option<Vec<u32>> {
    let doomed = vec.iter().position(|entry| !keep(entry))?;
    let mut next = doomed as u32;
    let mut remap: Vec<u32> = (0..doomed as u32).collect();
    remap.reserve(vec.len() - doomed);
    let mut index = 0;
    vec.retain(|entry| {
        let kept = index < doomed || (index > doomed && keep(entry));
        if index >= doomed {
            remap.push(if kept { next } else { REMOVED });
            next += u32::from(kept);
        }
        index += 1;
        kept
    });
    Some(remap)
}

/// Where each surviving entry moved after [`Entries::retain`].
#[derive(Debug)]
pub struct Remap {
    shown: Vec<u32>,
    hidden: Vec<u32>,
}

impl Remap {
    pub fn apply(&self, id: EntryId) -> Option<EntryId> {
        let table = if id.is_hidden() {
            &self.hidden
        } else {
            &self.shown
        };
        match table.get(id.index()) {
            None => Some(id),
            Some(&REMOVED) => None,
            Some(&index) => Some(EntryId::new(index as usize, id.is_hidden())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        entry::EntryExtra,
        stat::{Stat, StatFlags},
    };

    fn entry(name: &str, len: u64) -> Entry {
        let flags = if name.starts_with('.') {
            StatFlags::HIDDEN
        } else {
            StatFlags::empty()
        };
        Entry::new(
            name.into(),
            Stat {
                len,
                flags,
                ..Stat::default()
            },
            EntryExtra::default(),
        )
    }

    fn names(entries: &Entries) -> Vec<&str> {
        entries
            .ids(true)
            .map(|id| entries.entry(id).name.as_str())
            .collect()
    }

    #[test]
    fn hidden_entries_live_apart_and_ids_stay_stable() {
        let mut entries = Entries::default();
        let visible = entries.push(entry("a", 0));
        let hidden = entries.push(entry(".b", 0));
        entries.push(entry("c", 0));
        assert!(hidden.is_hidden());
        assert_eq!(entries.entry(visible).name, "a");
        assert_eq!(entries.entry(hidden).name, ".b");
        assert_eq!(entries.ids(false).count(), 2);
        assert_eq!(names(&entries), ["a", "c", ".b"]);
    }

    #[test]
    fn upsert_replaces_in_place_and_appends_new_names() {
        let mut entries = Entries::default();
        entries.push(entry("a", 1));
        entries.push(entry("b", 1));
        let written = entries.upsert(vec![entry("b", 2), entry("z", 3)]);
        assert_eq!(written.len(), 2);
        assert_eq!(names(&entries), ["a", "b", "z"]);
        assert_eq!(
            entries.find("b").map(|id| entries.entry(id).stat.len),
            Some(2)
        );
    }

    #[test]
    fn retain_reports_where_survivors_moved() {
        let mut entries = Entries::default();
        let ids: Vec<_> = ["a", "b", "c", "d", ".e"]
            .into_iter()
            .map(|name| entries.push(entry(name, 0)))
            .collect();
        let remap = entries
            .retain(|entry| entry.name != "b" && entry.name != "d")
            .expect("two entries removed");
        assert_eq!(names(&entries), ["a", "c", ".e"]);
        let moved: Vec<_> = ids.iter().map(|&id| remap.apply(id)).collect();
        assert_eq!(moved[0], Some(ids[0]));
        assert_eq!(moved[1], None);
        assert_eq!(moved[2], Some(ids[1]));
        assert_eq!(moved[3], None);
        assert_eq!(moved[4], Some(ids[4]));
        assert!(entries.retain(|_| true).is_none());
    }
}
